//! Exact audio association on metadata publication generations (#649, ADR 0054 D06) over real
//! `server.db` files: exact offers at publication, explicit display-only association, refusal of
//! incompatible or non-active releases, the live Media gate after withdrawal, supersession and
//! revocation, persistence across reopen and the Studio route. Release chains are seeded through
//! the shared world fixture; D06c through the real activation path is covered by the Studio
//! HTTP journey in `tests/support/metadata_audio_http_turso.rs`.
use super::{SeededWork, ServerFixture, WorkSeed};
use crate::contexts::catalog::{
    activate_audio,
    domain::{
        metadata_publication::{AudioRef, PublicationError, ReleasePin},
        ContentUnitId, OwnerId, Revision, WorkId,
    },
    public_units, ActivationAssociation, CatalogCommand, CatalogStoreError,
};
use crate::contexts::media::{playable_releases, ActiveAudio};
use crate::contract::studio::{
    ApiError, ErrorCode, MetadataAudioBasis as Basis, MetadataPublicationRequest as Request,
    MetadataPublicationState as State, MetadataPublicationUnit,
};
use axum::{
    body::Body,
    http::{header, Request as HttpRequest, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

async fn world() -> ServerFixture {
    let fixture = ServerFixture::new().await;
    fixture.studio_caller("alice", ALICE).await;
    fixture
        .exec(
            "INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','rights','synthetic-rights','synthetic')",
            &[],
        )
        .await
        .unwrap();
    fixture
}

async fn seed(fixture: &ServerFixture, tag: u32, units: &[&str]) -> SeededWork {
    fixture
        .seed_work(None, &WorkSeed::podcast(tag, "Đêm phát thanh", units))
        .await
}

fn release(seeded: &SeededWork, index: usize) -> String {
    seeded.units[index].chain.release_id.clone().unwrap()
}

async fn revisions(fixture: &ServerFixture, seeded: &SeededWork) -> (u64, Vec<(String, u64)>) {
    let store = fixture.store();
    let work = store
        .catalog_work(ALICE, &WorkId::parse(&seeded.work_id).unwrap())
        .await
        .unwrap()
        .revision();
    let mut units = Vec::new();
    for unit in &seeded.units {
        let id = ContentUnitId::parse(&unit.unit_id).unwrap();
        let revision = store.catalog_unit(ALICE, &id).await.unwrap().revision();
        units.push((unit.unit_id.clone(), revision));
    }
    (work, units)
}

fn publish(expected: u64, work_revision: u64, units: &[(String, u64)]) -> Request {
    Request::Publish {
        operation_id: Uuid::new_v4().to_string(),
        expected_version: expected,
        work_revision,
        units: units
            .iter()
            .map(|(unit_id, revision)| MetadataPublicationUnit {
                unit_id: unit_id.clone(),
                revision: *revision,
            })
            .collect(),
        confirm_metadata_rights: true,
    }
}

fn associate(expected: u64, unit: &str, release: &str, confirm: bool) -> Request {
    Request::AssociateAudio {
        operation_id: Uuid::new_v4().to_string(),
        expected_version: expected,
        unit_id: unit.into(),
        release_id: release.into(),
        confirm_display_only_drift: confirm,
    }
}

async fn execute(
    fixture: &ServerFixture,
    seeded: &SeededWork,
    request: Request,
) -> Result<State, CatalogStoreError> {
    fixture
        .store()
        .metadata_publication_execute(ALICE, &seeded.work_id, request, &ActiveAudio)
        .await
}

async fn state(fixture: &ServerFixture, seeded: &SeededWork) -> State {
    fixture
        .store()
        .metadata_publication_state(ALICE, &seeded.work_id)
        .await
        .unwrap()
}

/// Each entry's association as `(release, basis)`, in published order.
fn audio(state: &State) -> Vec<Option<(String, Basis)>> {
    state
        .generation
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .map(|entry| {
            entry
                .audio
                .as_ref()
                .map(|audio| (audio.release_id.clone(), audio.basis))
        })
        .collect()
}

fn refused(result: Result<State, CatalogStoreError>, expected: PublicationError) {
    match result {
        Err(CatalogStoreError::Publication(actual)) => assert_eq!(actual, expected),
        other => panic!("{other:?}"),
    }
}

/// What a public read of the Work plays now: per visible Unit, the playable release or `None`.
async fn playable(fixture: &ServerFixture, seeded: &SeededWork) -> Vec<Option<String>> {
    let connection = fixture.store().connection().await.unwrap();
    let tx = connection.read().await.unwrap();
    let page = public_units(&tx, &WorkId::parse(&seeded.work_id).unwrap(), None, 50)
        .await
        .unwrap()
        .unwrap();
    let asked: Vec<_> = page
        .items
        .iter()
        .map(|item| (item.unit.id(), item.audio.as_ref()))
        .collect();
    let releases = playable_releases(&tx, &asked).await.unwrap();
    tx.commit().await.unwrap();
    releases
        .into_iter()
        .map(|release| release.map(|r| r.release().document().id.clone()))
        .collect()
}

async fn count(fixture: &ServerFixture, table: &str) -> i64 {
    fixture
        .count(&format!("SELECT count(*) FROM {table}"), &[])
        .await
}

#[tokio::test]
async fn exact_releases_associate_at_publication_and_the_live_gate_stops_stale_audio() {
    let mut fixture = world().await;
    let seeded = seed(&fixture, 0x6400_0001, &["Tập 1", "Tập 2", "Tập 3"]).await;
    let (w, units) = revisions(&fixture, &seeded).await;
    let releases: Vec<_> = (0..3).map(|i| release(&seeded, i)).collect();
    let manifests = fixture
        .rows(
            "SELECT id,digest,snapshot FROM audio_releases ORDER BY id",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| -> (String, String, Vec<u8>) { (row.get(0), row.get(1), row.get(2)) })
        .collect::<Vec<_>>();

    // Audio already released for exactly these revisions: publication associates each one.
    let published = execute(&fixture, &seeded, publish(0, w, &units))
        .await
        .unwrap();
    assert_eq!(published.version, 1);
    assert_eq!(
        audio(&published),
        releases
            .iter()
            .map(|r| Some((r.clone(), Basis::Exact)))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        playable(&fixture, &seeded).await,
        releases.iter().cloned().map(Some).collect::<Vec<_>>()
    );

    // Withdrawal, supersession and revocation stop playback at once, while the generation
    // still records the exact release it associated.
    fixture.withdraw_release("alice", &seeded.units[1]).await;
    let replacement = fixture
        .publish_replacement(None, &seeded, &seeded.units[0])
        .await
        .release_id
        .unwrap();
    fixture.revoke_submission("alice", &seeded.units[2]).await;
    assert_eq!(playable(&fixture, &seeded).await, [None, None, None]);
    assert_eq!(audio(&state(&fixture, &seeded).await), audio(&published));

    // Only the release the pointer names now may be associated: no substitute, no revival.
    for (unit, stale) in [(0, &releases[0]), (1, &releases[1])] {
        refused(
            execute(
                &fixture,
                &seeded,
                associate(1, &seeded.units[unit].unit_id, stale, true),
            )
            .await,
            PublicationError::ReleaseNotActive,
        );
    }
    let associated = execute(
        &fixture,
        &seeded,
        associate(1, &seeded.units[0].unit_id, &replacement, false),
    )
    .await
    .unwrap();
    assert_eq!(associated.version, 2);
    assert_eq!(associated.generation.as_ref().unwrap().number, 2);
    assert_eq!(
        associated.first_visible_at_ms,
        published.first_visible_at_ms
    );
    assert_eq!(
        audio(&associated),
        [
            Some((replacement.clone(), Basis::Exact)),
            Some((releases[1].clone(), Basis::Exact)),
            Some((releases[2].clone(), Basis::Exact)),
        ]
    );
    assert_eq!(
        playable(&fixture, &seeded).await,
        [Some(replacement.clone()), None, None]
    );

    // Association never rewrote a manifest, a digest or an approval.
    let after = fixture
        .rows(
            "SELECT id,digest,snapshot FROM audio_releases WHERE id<>?1 ORDER BY id",
            &[&replacement.as_str()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| -> (String, String, Vec<u8>) { (row.get(0), row.get(1), row.get(2)) })
        .collect::<Vec<_>>();
    assert_eq!(after, manifests);

    fixture.restart().await;
    assert_eq!(state(&fixture, &seeded).await, associated);
    assert_eq!(
        playable(&fixture, &seeded).await,
        [Some(replacement), None, None]
    );
}

#[tokio::test]
async fn display_only_drift_needs_the_owner_and_language_drift_stays_metadata_only() {
    let fixture = world().await;
    let seeded = seed(&fixture, 0x6400_0002, &["Tập 1"]).await;
    let unit = seeded.units[0].unit_id.clone();
    let unit_id = ContentUnitId::parse(&unit).unwrap();
    let work_id = WorkId::parse(&seeded.work_id).unwrap();
    let release = release(&seeded, 0);
    let current = fixture.store().catalog_unit(ALICE, &unit_id).await.unwrap();
    let mut details = current.details().clone();
    details.title = "Tập 1 (bản biên tập)".into();
    details.description = "Lời giới thiệu mới".into();
    let (w, _) = revisions(&fixture, &seeded).await;
    fixture
        .store()
        .catalog_execute(
            ALICE,
            &Uuid::new_v4().to_string(),
            CatalogCommand::EditUnit {
                id: unit_id.clone(),
                expected_revision: current.revision(),
                expected_work_revision: w,
                details,
            },
        )
        .await
        .unwrap();
    let (w, units) = revisions(&fixture, &seeded).await;

    // The release pins the earlier title: publication leaves the new revision metadata-only.
    let published = execute(&fixture, &seeded, publish(0, w, &units))
        .await
        .unwrap();
    assert_eq!(audio(&published), [None]);
    assert_eq!(playable(&fixture, &seeded).await, [None]);

    refused(
        execute(
            &fixture,
            &seeded,
            associate(1, &unit, &Uuid::new_v4().to_string(), true),
        )
        .await,
        PublicationError::ReleaseNotActive,
    );
    refused(
        execute(&fixture, &seeded, associate(1, &unit, &release, false)).await,
        PublicationError::AudioConfirmationRequired,
    );
    let request = associate(1, &unit, &release, true);
    let associated = execute(&fixture, &seeded, request.clone()).await.unwrap();
    assert_eq!(
        audio(&associated),
        [Some((release.clone(), Basis::DisplayOnly))]
    );
    assert_eq!(playable(&fixture, &seeded).await, [Some(release.clone())]);
    // A lost response replays the settled receipt; a changed payload under the same ID refuses.
    assert_eq!(
        execute(&fixture, &seeded, request.clone()).await.unwrap(),
        associated
    );
    let Request::AssociateAudio { operation_id, .. } = &request else {
        unreachable!()
    };
    let changed = Request::AssociateAudio {
        operation_id: operation_id.clone(),
        expected_version: 2,
        unit_id: unit.clone(),
        release_id: release.clone(),
        confirm_display_only_drift: true,
    };
    assert!(matches!(
        execute(&fixture, &seeded, changed).await,
        Err(CatalogStoreError::OperationReused)
    ));
    // Associating the same release again records nothing.
    refused(
        execute(&fixture, &seeded, associate(2, &unit, &release, true)).await,
        PublicationError::Unchanged,
    );
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        2
    );

    // A new Work revision in another language carries no association and cannot take one.
    let work = fixture.store().catalog_work(ALICE, &work_id).await.unwrap();
    let mut details = work.details().clone();
    details.language = "en".into();
    fixture
        .store()
        .catalog_execute(
            ALICE,
            &Uuid::new_v4().to_string(),
            CatalogCommand::EditWork {
                id: work_id,
                expected_revision: work.revision(),
                details,
            },
        )
        .await
        .unwrap();
    let (w, units) = revisions(&fixture, &seeded).await;
    let republished = execute(&fixture, &seeded, publish(2, w, &units))
        .await
        .unwrap();
    assert_eq!(audio(&republished), [None]);
    refused(
        execute(&fixture, &seeded, associate(3, &unit, &release, true)).await,
        PublicationError::AudioIncompatible,
    );
    assert_eq!(playable(&fixture, &seeded).await, [None]);
}

async fn release_digest(fixture: &ServerFixture, release: &str) -> String {
    fixture
        .rows("SELECT digest FROM audio_releases WHERE id=?1", &[&release])
        .await
        .unwrap()[0]
        .get(0)
}

/// Run D06c in its own write unit, as the audio activation runs it after moving the pointer.
async fn activate(
    fixture: &ServerFixture,
    unit: &str,
    release: &str,
    work_revision: u64,
    unit_revision: u64,
) -> ActivationAssociation {
    let digest = release_digest(fixture, release).await;
    let mut connection = fixture.store().connection().await.unwrap();
    let tx = connection.transaction().await.unwrap();
    let outcome = activate_audio(
        &tx,
        &OwnerId::parse("alice").unwrap(),
        &ContentUnitId::parse(unit).unwrap(),
        ReleasePin {
            release: AudioRef::new(release, &digest).unwrap(),
            work_revision: Revision::new(work_revision).unwrap(),
            unit_revision: Revision::new(unit_revision).unwrap(),
        },
        1_900_000_000_000,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    outcome
}

/// D06c: exact pins associate as generation N+1; other pins, already associated entries and
/// unpublished Units write nothing.
#[tokio::test]
async fn activation_associates_only_an_exact_active_entry() {
    let fixture = world().await;
    let seeded = seed(&fixture, 0x6400_0003, &["Tập 1", "Tập 2"]).await;
    let (w, units) = revisions(&fixture, &seeded).await;
    let (first, second) = (release(&seeded, 0), release(&seeded, 1));
    let (u0, r0) = (units[0].0.as_str(), units[0].1);
    let (u1, r1) = (units[1].0.as_str(), units[1].1);
    // Nothing is published yet: the release waits for metadata publication to offer it.
    assert_eq!(
        activate(&fixture, u0, &first, w, r0).await,
        ActivationAssociation::NotPublished
    );
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        0
    );
    // With the first release withdrawn, its Unit publishes metadata-only.
    fixture.withdraw_release("alice", &seeded.units[0]).await;
    let only_second = execute(&fixture, &seeded, publish(0, w, &units[1..]))
        .await
        .unwrap();
    assert_eq!(audio(&only_second), [Some((second.clone(), Basis::Exact))]);
    assert_eq!(
        activate(&fixture, u0, &first, w, r0).await,
        ActivationAssociation::NotPublished
    );
    assert_eq!(
        activate(&fixture, u1, &second, w, r1).await,
        ActivationAssociation::AlreadyAssociated
    );
    let both = execute(&fixture, &seeded, publish(1, w, &units))
        .await
        .unwrap();
    assert_eq!(audio(&both), [None, Some((second.clone(), Basis::Exact))]);
    // A release pinned to a later Unit revision than the entry is "ready, not associated".
    let current = fixture
        .store()
        .catalog_unit(ALICE, &ContentUnitId::parse(u0).unwrap())
        .await
        .unwrap();
    let mut details = current.details().clone();
    details.title = "Tập 1 (mới)".into();
    fixture
        .store()
        .catalog_execute(
            ALICE,
            &Uuid::new_v4().to_string(),
            CatalogCommand::EditUnit {
                id: current.id().clone(),
                expected_revision: current.revision(),
                expected_work_revision: w,
                details,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        activate(&fixture, u0, &first, w, r0 + 1).await,
        ActivationAssociation::NotAssociated
    );
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        2
    );
    // The exact pin associates as generation N+1, copying the other entry and the anchor.
    assert_eq!(
        activate(&fixture, u0, &first, w, r0).await,
        ActivationAssociation::Associated
    );
    let settled = state(&fixture, &seeded).await;
    assert_eq!(
        (settled.version, settled.generation.as_ref().unwrap().number),
        (3, 3)
    );
    assert_eq!(settled.first_visible_at_ms, both.first_visible_at_ms);
    assert_eq!(
        audio(&settled),
        [Some((first, Basis::Exact)), Some((second, Basis::Exact))]
    );
    let action: String = fixture
        .rows(
            "SELECT action FROM catalog_metadata_publication_events WHERE version=3",
            &[],
        )
        .await
        .unwrap()[0]
        .get(0);
    assert_eq!(action, "activate_audio");
}

#[tokio::test]
async fn generation_audio_rows_are_append_only_and_bound_to_a_release_of_the_same_unit() {
    let fixture = world().await;
    let seeded = seed(&fixture, 0x6400_0004, &["Tập 1", "Tập 2"]).await;
    let (w, units) = revisions(&fixture, &seeded).await;
    execute(&fixture, &seeded, publish(0, w, &units))
        .await
        .unwrap();
    for sql in [
        "UPDATE catalog_metadata_generation_audio SET basis='display_only'",
        "DELETE FROM catalog_metadata_generation_audio",
    ] {
        assert!(fixture.try_exec(sql, &[]).await.is_err(), "{sql}");
    }
    // A sealed generation takes no new pin; an open one only a release of the same Unit.
    let other = release(&seeded, 1);
    let digest = release_digest(&fixture, &other).await;
    let insert = "INSERT INTO catalog_metadata_generation_audio(work_id,generation,unit_id,release_id,release_digest,basis) VALUES(?1,1,?2,?3,?4,'exact')";
    assert!(fixture
        .try_exec(
            insert,
            &[
                &seeded.work_id.as_str(),
                &units[0].0.as_str(),
                &other.as_str(),
                &digest.as_str()
            ],
        )
        .await
        .is_err());
    assert_eq!(
        count(&fixture, "catalog_metadata_generation_audio").await,
        2
    );
}

#[tokio::test]
async fn studio_route_associates_audio_and_answers_the_contract_shape() {
    let fixture = world().await;
    let seeded = seed(&fixture, 0x6400_0005, &["Tập 1"]).await;
    let (w, units) = revisions(&fixture, &seeded).await;
    let app =
        crate::adapters::inbound::http::api_router(crate::adapters::inbound::http::AppState {
            store: fixture.store().clone(),
            web_origin: "https://studio.example.test".into(),
        });
    let path = format!(
        "/api/v1/studio/works/{}/metadata-publication",
        seeded.work_id
    );
    let send = |body: String| {
        HttpRequest::post(&path)
            .header(header::ORIGIN, "https://studio.example.test")
            .header(header::COOKIE, format!("cantos_session={ALICE}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap()
    };
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../contracts/studio/metadata-publication/v1/metadata-publication.schema.json"
    ))
    .unwrap();
    let valid = |name: &str, value: &serde_json::Value| {
        jsonschema::validator_for(&serde_json::json!({
            "$schema": schema["$schema"], "$defs": schema["$defs"],
            "$ref": format!("#/$defs/{name}")
        }))
        .unwrap()
        .is_valid(value)
    };
    async fn json(response: axum::response::Response) -> serde_json::Value {
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
    }
    let request = serde_json::to_value(publish(0, w, &units)).unwrap();
    let response = app
        .clone()
        .oneshot(send(request.to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let active = json(response).await;
    assert!(valid("MetadataPublicationState", &active));
    assert_eq!(
        active["generation"]["entries"][0]["audio"]["basis"],
        "exact"
    );

    let stale =
        serde_json::to_value(associate(1, &units[0].0, &Uuid::new_v4().to_string(), true)).unwrap();
    assert!(valid("MetadataPublicationRequest", &stale));
    let response = app.clone().oneshot(send(stale.to_string())).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error: ApiError = serde_json::from_value(json(response).await).unwrap();
    assert_eq!(error.code, ErrorCode::CatalogConflict);
    let unchanged =
        serde_json::to_value(associate(1, &units[0].0, &release(&seeded, 0), false)).unwrap();
    let response = app
        .clone()
        .oneshot(send(unchanged.to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let mut unknown = unchanged.clone();
    unknown["latest"] = serde_json::json!(true);
    let response = app
        .clone()
        .oneshot(send(unknown.to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        1
    );
}

/// Two owners' tabs associate different Units from the same version: the single writer settles
/// one, the other is a typed stale conflict carrying the new version, and a retry on that
/// version settles. A duplicated request settles once and replays its receipt.
#[tokio::test]
async fn concurrent_associations_settle_once_and_the_loser_retries_on_the_new_version() {
    let fixture = world().await;
    let seeded = seed(&fixture, 0x6400_0006, &["Tập 1", "Tập 2"]).await;
    let (w, units) = revisions(&fixture, &seeded).await;
    execute(&fixture, &seeded, publish(0, w, &units))
        .await
        .unwrap();
    let mut replacements = Vec::new();
    // Distinct replacement generations keep every fixture identity disjoint.
    for (unit, generation) in seeded.units.iter().zip(1..) {
        replacements.push(
            fixture
                .publish_generation(None, &seeded, unit, generation)
                .await
                .release_id
                .unwrap(),
        );
    }
    let first = associate(1, &units[0].0, &replacements[0], false);
    let second = associate(1, &units[1].0, &replacements[1], false);
    let (a, b) = tokio::join!(
        execute(&fixture, &seeded, first.clone()),
        execute(&fixture, &seeded, second.clone())
    );
    let (won, lost) = match (a, b) {
        (Ok(state), Err(error)) => (state, (Err(error), second)),
        (Err(error), Ok(state)) => (state, (Err(error), first)),
        other => panic!("{other:?}"),
    };
    assert_eq!(won.version, 2);
    refused(lost.0, PublicationError::StaleVersion { current: 2 });
    let Request::AssociateAudio {
        unit_id,
        release_id,
        ..
    } = lost.1
    else {
        unreachable!()
    };
    let retry = associate(2, &unit_id, &release_id, false);
    let (x, y) = tokio::join!(
        execute(&fixture, &seeded, retry.clone()),
        execute(&fixture, &seeded, retry)
    );
    assert_eq!(x.unwrap(), y.unwrap());
    let settled = state(&fixture, &seeded).await;
    assert_eq!(settled.version, 3);
    assert_eq!(
        audio(&settled),
        replacements
            .iter()
            .map(|r| Some((r.clone(), Basis::Exact)))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        3
    );
}

#[tokio::test]
async fn direct_manifest_and_discovery_require_the_exact_current_generation_association() {
    use crate::contexts::media::{ListenerError, ListenerService};
    let fixture = world().await;
    let seeded = seed(&fixture, 0x6510_0001, &["Bản nghe chính xác"]).await;
    let unit_id = seeded.units[0].unit_id.clone();
    let release_id = release(&seeded, 0);
    let service = ListenerService::new(fixture.store().clone());
    assert_eq!(
        service.manifest(&release_id).await,
        Err(ListenerError::NotFound)
    );
    let (w, units) = revisions(&fixture, &seeded).await;
    execute(&fixture, &seeded, publish(0, w, &units))
        .await
        .unwrap();
    let manifest = service.manifest(&release_id).await.unwrap();
    let page = service
        .metadata_units(&seeded.work_id, 20, None)
        .await
        .unwrap();
    assert_eq!(
        page.items[0].playback.as_ref().unwrap().publication_id,
        release_id
    );
    // A display-only revision becomes metadata-only until the owner explicitly associates it.
    let current = fixture
        .store()
        .catalog_unit(ALICE, &ContentUnitId::parse(&unit_id).unwrap())
        .await
        .unwrap();
    let mut details = current.details().clone();
    details.title = "Tên biên tập mới".into();
    fixture
        .store()
        .catalog_execute(
            ALICE,
            &Uuid::new_v4().to_string(),
            CatalogCommand::EditUnit {
                id: current.id().clone(),
                expected_revision: current.revision(),
                expected_work_revision: w,
                details,
            },
        )
        .await
        .unwrap();
    let (w, units) = revisions(&fixture, &seeded).await;
    execute(&fixture, &seeded, publish(1, w, &units))
        .await
        .unwrap();
    assert_eq!(
        service
            .metadata_units(&seeded.work_id, 20, None)
            .await
            .unwrap()
            .items[0]
            .playback,
        None
    );
    assert_eq!(
        service.manifest(&release_id).await,
        Err(ListenerError::NotFound)
    );
    execute(&fixture, &seeded, associate(2, &unit_id, &release_id, true))
        .await
        .unwrap();
    assert_eq!(service.manifest(&release_id).await.unwrap(), manifest);
    assert_eq!(
        service
            .metadata_units(&seeded.work_id, 20, None)
            .await
            .unwrap()
            .items[0]
            .title,
        "Tên biên tập mới"
    );
    fixture.revoke_submission("alice", &seeded.units[0]).await;
    assert_eq!(
        service.manifest(&release_id).await,
        Err(ListenerError::NotFound)
    );
    assert_eq!(
        service
            .metadata_units(&seeded.work_id, 20, None)
            .await
            .unwrap()
            .items[0]
            .playback,
        None
    );
}
