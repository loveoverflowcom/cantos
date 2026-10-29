//! #171 acceptance over a real file-backed `server.db` and a real private filesystem object root.
//! Releases are seeded with production domain values (`test_support::world`); the cover image is
//! admitted, selected and disclosed through the production Studio services, and read through the
//! production `ListenerService` and HTTP router. Nothing here is deployed TLS/CDN, browser or
//! device acceptance, and no creator publication journey ran (see `world`).
use std::sync::Arc;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use cantos_store_turso::testing::FixedClock;
use http_body_util::BodyExt;
use tower::ServiceExt;

use crate::{
    adapters::inbound::http::{api_router, router_with_public_media, AppState},
    application::Store,
    contexts::media::{
        domain::playback_grant::GrantKey, ArtworkService, CoverDisclosureApplicationError,
        CoverDisclosureService, FilesystemPublicationStorage, ListenerError, ListenerService,
        OriginPlayback, PublicMediaOrigin, PublicationService,
    },
    contract::{studio as studio_dto, theatre::discovery as dto},
    test_support::{SeededUnit, SeededWork, ServerFixture, WorkSeed},
};

/// 2030-03-17 in microseconds; far from any real clock so a leaked wall-clock read shows.
const T0: i64 = 1_900_000_000_000_000;
const HOUR: i64 = 3_600_000_000;
const TOKEN: &str = "c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0";
const ORIGIN: &str = "https://listen.example.test";
const STUDIO: &str = "https://studio.example.test";
const PNG: &[u8] = include_bytes!("../contexts/media/image_intake/fixtures/python-rgb-2x3.png");
const PNG_SHA256: &str = "7b222c7332a2d0a325b482be5ff001918ff8470f52405b99d4c8351856bed276";
const JPEG: &[u8] = include_bytes!("../contexts/media/image_intake/fixtures/pillow-rgb-2x3.jpg");
const JPEG_SHA256: &str = "a259cc25075434554da937e24a8056b5f9db0608a448c06effac4ba9c950eb46";
const AVIF: &[u8] =
    include_bytes!("../contexts/media/image_intake/fixtures/ffmpeg-yuv420p-2x3.avif");
const AVIF_SHA256: &str = "7c2306b2967d74b97c08809e1aa73b4923ab830a71cdecdf7acf2f0ba6873257";
const PROVENANCE: &str = "private-provenance-sentinel";
const RIGHTS: &str = "synthetic-rights";

struct World {
    fixture: ServerFixture,
    clock: Arc<FixedClock>,
    store: Store,
    work: SeededWork,
    unit: SeededUnit,
    generation: u32,
}

struct Image {
    id: String,
    digest: String,
}

impl World {
    async fn new() -> Self {
        let clock = FixedClock::new(T0);
        let fixture = ServerFixture::with_clock(clock.clone()).await;
        let store = fixture
            .store()
            .clone()
            .with_audio_storage(&fixture.dir().join("private"))
            .expect("a private object root");
        let work = fixture
            .seed_work(
                None,
                &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Một lời hẹn"]),
            )
            .await;
        fixture.seed_development_session("alice", TOKEN, 48).await;
        fixture
            .exec(
                "INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','rights',?1,'Synthetic fixture declaration; no rights approval')",
                &[&RIGHTS],
            )
            .await
            .expect("rights evidence");
        let unit = work.units[0].clone();
        Self {
            fixture,
            clock,
            store,
            work,
            unit,
            generation: 0,
        }
    }

    fn now_ms(&self) -> i64 {
        self.fixture.clock_reading().expect("the clock reads") / 1000
    }

    /// Admit one image through the production Studio intake, into the private object root.
    async fn admit(&self, bytes: &[u8], sha256: &str, mime: studio_dto::ImageMime) -> Image {
        let service = ArtworkService::new(self.store.clone());
        let id = uuid::Uuid::new_v4().to_string();
        service
            .prepare(
                TOKEN,
                &self.work.work_id,
                studio_dto::CreateImageIntentRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    id: id.clone(),
                    expected_sha256: sha256.into(),
                    expected_byte_length: bytes.len() as u64,
                    content_type: mime,
                    declarations: studio_dto::ImageDeclarations {
                        provenance: PROVENANCE.into(),
                        rights_refs: vec![RIGHTS.into()],
                    },
                },
            )
            .await
            .expect("the intent is prepared");
        // Admission lanes are process-wide and other tests deliberately exhaust them: retry the
        // production refusal (`AdmissionBusy`) briefly instead of bypassing admission.
        let mut ticket = None;
        for _ in 0..400 {
            match service
                .begin_upload(TOKEN, &self.work.work_id, &id, mime)
                .await
            {
                Ok(t) => {
                    ticket = Some(t);
                    break;
                }
                Err(crate::contexts::media::ArtworkApplicationError::Intake(
                    crate::contexts::media::image_intake::ImageIntakeError::AdmissionBusy,
                )) => tokio::time::sleep(std::time::Duration::from_millis(25)).await,
                Err(e) => panic!("the upload begins: {e:?}"),
            }
        }
        let asset = ticket
            .expect("an admission lane frees within ten seconds")
            .admit(bytes.to_vec())
            .await
            .expect("the bytes are admitted")
            .into_parts()
            .0;
        Image {
            id: asset.id,
            digest: asset.digest,
        }
    }

    async fn head_revision(&self) -> u64 {
        u64::try_from(
            self.fixture
                .count(
                    "SELECT head_revision FROM catalog_works WHERE id=?1",
                    &[&self.work.work_id.as_str()],
                )
                .await,
        )
        .unwrap()
    }

    /// The owner's private current cover: a new Work revision, nothing public.
    async fn select(&self, image: Option<&Image>, alt: &str) {
        ArtworkService::new(self.store.clone())
            .set_artwork(
                TOKEN,
                &self.work.work_id,
                studio_dto::SetWorkArtworkRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    expected_work_revision: self.head_revision().await,
                    artwork: image.map(|image| studio_dto::WorkArtworkSelection {
                        image_asset_id: image.id.clone(),
                        image_asset_digest: image.digest.clone(),
                        alt_text: alt.into(),
                    }),
                },
            )
            .await
            .expect("the selection is recorded");
    }

    /// A newer release of the unit, pinned to the current Work head (and so to its cover).
    async fn republish(&mut self) {
        // The replacement chain is dated an hour before "now"; keep it after the Work edit.
        self.clock.advance(2 * HOUR);
        self.generation += 1;
        let chain = self
            .fixture
            .publish_generation(None, &self.work, &self.unit, self.generation)
            .await;
        self.unit.chain = chain;
    }

    async fn disclose(
        &self,
        image: &Image,
        cleared_for_ms: Option<i64>,
    ) -> Result<studio_dto::CoverDisclosureResponse, CoverDisclosureApplicationError> {
        CoverDisclosureService::new(self.store.clone())
            .record(
                TOKEN,
                &self.work.work_id,
                studio_dto::RecordCoverDisclosureRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    image_asset_digest: image.digest.clone(),
                    decision: if cleared_for_ms.is_some() {
                        studio_dto::CoverDisclosureKind::Cleared
                    } else {
                        studio_dto::CoverDisclosureKind::Revoked
                    },
                    valid_until_ms: cleared_for_ms
                        .map(|ms| u64::try_from(self.now_ms() + ms).unwrap()),
                },
            )
            .await
    }

    fn covered(&self) -> ListenerService {
        ListenerService::new(self.store.clone())
            .with_cover_delivery(PublicMediaOrigin::parse(ORIGIN).unwrap(), &self.store)
    }

    async fn cover(&self) -> Option<dto::CoverImage> {
        let service = self.covered();
        let detail = service.covered_work(&self.work.work_id).await.unwrap();
        let page = service
            .covered_works("", None, None, 20, None)
            .await
            .unwrap();
        assert_eq!(page.items, vec![detail.clone()], "list and detail agree");
        detail.cover
    }

    fn expected(
        &self,
        sha256: &str,
        mime: dto::CoverMime,
        length: usize,
        alt: &str,
    ) -> dto::CoverImage {
        dto::CoverImage {
            url: format!(
                "{ORIGIN}/api/v1/theatre/works/{}/cover/{sha256}",
                self.work.work_id
            ),
            sha256: sha256.into(),
            content_type: mime,
            width_px: 2,
            height_px: 3,
            byte_length: length as u64,
            alt_text: alt.into(),
        }
    }

    fn router(&self) -> Router {
        let publication = Arc::new(
            FilesystemPublicationStorage::new(&self.fixture.dir().join("publication")).unwrap(),
        );
        let origin = OriginPlayback::new(
            PublicMediaOrigin::parse(ORIGIN).unwrap(),
            GrantKey::new(&[5; 32]).unwrap(),
            publication.clone(),
        );
        router_with_public_media(
            AppState {
                store: self.store.clone(),
                web_origin: STUDIO.into(),
            },
            PublicationService::new(self.store.clone()).with_origin_publication(publication),
            origin,
        )
    }
}

fn refusal<T>(result: Result<T, ListenerError>) -> ListenerError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected a refusal"),
    }
}

const ALT_A: &str = "Sân khấu tối, một ngọn đèn vàng chiếu xuống chiếc ghế trống.";
const ALT_B: &str = "Bìa mới: tấm rèm nhung đỏ khép hờ.";

#[path = "metadata_cover_turso_tests.rs"]
mod metadata;

#[tokio::test]
async fn metadata_consent_is_explicit_revision_bound_atomic_and_replay_safe() {
    let world = World::new().await;
    let image = world
        .admit(PNG, PNG_SHA256, studio_dto::ImageMime::Png)
        .await;
    world.select(Some(&image), ALT_A).await;
    let revision = world.head_revision().await;
    let service = CoverDisclosureService::new(world.store.clone());
    let request = studio_dto::RecordMetadataCoverDisclosureRequest {
        expected_work_revision: revision,
        evidence_reference: "synthetic-metadata-image-public-permission".into(),
        disclosure: studio_dto::RecordCoverDisclosureRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            image_asset_digest: image.digest.clone(),
            decision: studio_dto::CoverDisclosureKind::Cleared,
            valid_until_ms: Some((world.now_ms() + 10_000) as u64),
        },
    };
    let mut stale = request.clone();
    stale.expected_work_revision -= 1;
    assert!(
        matches!(service.record_metadata(TOKEN, &world.work.work_id, stale).await,
        Err(CoverDisclosureApplicationError::Storage(crate::platform::store::StoreError::StaleRevision(r))) if r == revision)
    );
    assert_eq!(
        world
            .fixture
            .count("SELECT count(*) FROM media_cover_disclosures", &[])
            .await,
        0
    );
    let first = service
        .record_metadata(TOKEN, &world.work.work_id, request.clone())
        .await
        .unwrap();
    assert_eq!(
        service
            .record_metadata(TOKEN, &world.work.work_id, request.clone())
            .await
            .unwrap(),
        first
    );
    let mut changed = request.clone();
    changed.evidence_reference = "different-permission".into();
    assert!(matches!(
        service
            .record_metadata(TOKEN, &world.work.work_id, changed)
            .await,
        Err(CoverDisclosureApplicationError::Storage(
            crate::platform::store::StoreError::OperationReused
        ))
    ));
    // The same operation cannot be replayed through the old unscoped API.
    assert!(matches!(
        service
            .record(TOKEN, &world.work.work_id, request.disclosure.clone())
            .await,
        Err(CoverDisclosureApplicationError::Storage(
            crate::platform::store::StoreError::OperationReused
        ))
    ));
    let connection = world.store.connection().await.unwrap();
    let tx = connection.read().await.unwrap();
    let facts = super::pinned_cover_facts(
        &tx,
        &[super::PinnedWork {
            work_id: world.work.work_id.clone(),
            owner_id: "alice".into(),
            revision,
        }],
    )
    .await
    .unwrap();
    assert_eq!(facts[&world.work.work_id].metadata_revision, Some(revision));
    tx.commit().await.unwrap();
    assert_eq!(
        world
            .fixture
            .count("SELECT count(*) FROM media_metadata_cover_bindings", &[])
            .await,
        1
    );
    for sql in [
        "UPDATE media_metadata_cover_bindings SET evidence_reference='changed'",
        "DELETE FROM media_metadata_cover_bindings",
        "INSERT OR REPLACE INTO media_metadata_cover_bindings SELECT * FROM media_metadata_cover_bindings",
    ] { assert!(world.fixture.exec(sql, &[]).await.is_err()); }
    // A new legacy decision never silently inherits the metadata-only scope.
    world.disclose(&image, None).await.unwrap();
    let tx = connection.read().await.unwrap();
    let facts = super::pinned_cover_facts(
        &tx,
        &[super::PinnedWork {
            work_id: world.work.work_id.clone(),
            owner_id: "alice".into(),
            revision,
        }],
    )
    .await
    .unwrap();
    assert_eq!(facts[&world.work.work_id].metadata_revision, None);
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn published_cover_is_pinned_disclosed_and_survives_private_replacement() {
    let mut world = World::new().await;
    let a = world
        .admit(PNG, PNG_SHA256, studio_dto::ImageMime::Png)
        .await;
    world.select(Some(&a), ALT_A).await;
    // The seeded release pinned the Work before any artwork: no cover, whatever is disclosed.
    world.disclose(&a, Some(HOUR / 1000 * 24)).await.unwrap();
    assert_eq!(world.cover().await, None, "the release pins no artwork");

    world.republish().await;
    // The new release pins A and the 24h clearance is current: A shows its admitted facts.
    let shown = world.expected(PNG_SHA256, dto::CoverMime::Png, PNG.len(), ALT_A);
    assert_eq!(world.cover().await, Some(shown.clone()));

    // Default and presentation=1 readers are unchanged by a disclosed cover.
    let released = world
        .covered()
        .presented_work(&world.work.work_id)
        .await
        .unwrap();
    assert_eq!(
        ListenerService::new(world.store.clone())
            .presented_work(&world.work.work_id)
            .await
            .unwrap(),
        released
    );
    let covered = world
        .covered()
        .covered_work(&world.work.work_id)
        .await
        .unwrap();
    assert_eq!(
        (covered.title.as_str(), covered.unit_count),
        (released.title.as_str(), released.unit_count)
    );

    // Owner privately replaces A with B and even discloses B: public stays A (release-pinned).
    let b = world
        .admit(JPEG, JPEG_SHA256, studio_dto::ImageMime::Jpeg)
        .await;
    world.select(Some(&b), ALT_B).await;
    world.disclose(&b, Some(HOUR / 1000 * 24)).await.unwrap();
    assert_eq!(world.cover().await, Some(shown.clone()));
    // A private clear changes nothing public either.
    world.select(None, "").await;
    assert_eq!(world.cover().await, Some(shown.clone()));

    // Restart preserves the binding: the same file served again shows the same A.
    let store_root = world.fixture.dir().join("private");
    // Every handle on the file must be released before it can be served again.
    drop(std::mem::replace(
        &mut world.store,
        crate::test_support::detached_store(),
    ));
    world.fixture.restart().await;
    world.store = world
        .fixture
        .store()
        .clone()
        .with_audio_storage(&store_root)
        .unwrap();
    assert_eq!(world.cover().await, Some(shown.clone()));

    // A newer release pins the clear: the cover goes away, and the old A URL is not_found.
    world.republish().await;
    assert_eq!(world.cover().await, None);
    assert_eq!(
        refusal(
            world
                .covered()
                .admit_cover(&world.work.work_id, PNG_SHA256)
                .await
        ),
        ListenerError::NotFound
    );
    // Re-selecting B and republishing moves the cover to B, which is already disclosed.
    world.select(Some(&b), ALT_B).await;
    world.republish().await;
    let shown_b = world.expected(JPEG_SHA256, dto::CoverMime::Jpeg, JPEG.len(), ALT_B);
    // The 24h clearance of B was recorded ~4h ago and is still current.
    assert_eq!(world.cover().await, Some(shown_b));
    assert_eq!(
        refusal(
            world
                .covered()
                .admit_cover(&world.work.work_id, PNG_SHA256)
                .await
        ),
        ListenerError::NotFound,
        "a stale SHA after republish is not_found"
    );
}

#[tokio::test]
async fn disclosure_is_required_and_revocation_expiry_and_withdrawal_hide_the_cover() {
    let mut world = World::new().await;
    let a = world
        .admit(PNG, PNG_SHA256, studio_dto::ImageMime::Png)
        .await;
    world.select(Some(&a), ALT_A).await;
    world.republish().await;
    let shown = world.expected(PNG_SHA256, dto::CoverMime::Png, PNG.len(), ALT_A);

    // Published and visible, but undisclosed: no cover; title and units are unaffected.
    assert_eq!(world.cover().await, None);
    assert!(world
        .covered()
        .covered_units(&world.work.work_id, 20, None)
        .await
        .is_ok());

    // Cleared for one hour: shown; half-open expiry hides it exactly at the boundary.
    world.disclose(&a, Some(HOUR / 1000)).await.unwrap();
    assert_eq!(world.cover().await, Some(shown.clone()));
    world.clock.advance(HOUR - 1000);
    assert_eq!(
        world.cover().await,
        Some(shown.clone()),
        "1 ms before expiry"
    );
    world.clock.advance(1000);
    assert_eq!(world.cover().await, None, "expired at valid_until_ms");

    // Renewal is a new decision; a later revocation wins by server sequence.
    let renewed = world.disclose(&a, Some(HOUR / 1000)).await.unwrap();
    assert_eq!(world.cover().await, Some(shown.clone()));
    let revoked = world.disclose(&a, None).await.unwrap();
    assert!(revoked.sequence > renewed.sequence);
    assert_eq!(world.cover().await, None);
    assert_eq!(
        refusal(
            world
                .covered()
                .admit_cover(&world.work.work_id, PNG_SHA256)
                .await
        ),
        ListenerError::NotFound
    );
    world.disclose(&a, Some(HOUR / 1000)).await.unwrap();
    assert_eq!(world.cover().await, Some(shown.clone()));

    // An uncomposed host answers null and not_found for the same facts.
    let uncomposed = ListenerService::new(world.store.clone());
    assert_eq!(
        uncomposed
            .covered_work(&world.work.work_id)
            .await
            .unwrap()
            .cover,
        None
    );
    assert_eq!(
        refusal(
            uncomposed
                .admit_cover(&world.work.work_id, PNG_SHA256)
                .await
        ),
        ListenerError::NotFound
    );

    // Withdrawal removes the Work and its cover; no independent selector revives it.
    let unit = world.unit.clone();
    world
        .fixture
        .withdraw_release(&world.work.owner, &unit)
        .await;
    assert_eq!(
        refusal(world.covered().covered_work(&world.work.work_id).await),
        ListenerError::NotFound
    );
    assert!(world
        .covered()
        .covered_works("", None, None, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        refusal(
            world
                .covered()
                .admit_cover(&world.work.work_id, PNG_SHA256)
                .await
        ),
        ListenerError::NotFound
    );
}

#[tokio::test]
async fn studio_disclosure_is_owner_only_digest_bound_and_replayable() {
    let world = World::new().await;
    let a = world
        .admit(PNG, PNG_SHA256, studio_dto::ImageMime::Png)
        .await;
    let service = CoverDisclosureService::new(world.store.clone());
    let request = studio_dto::RecordCoverDisclosureRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        image_asset_digest: a.digest.clone(),
        decision: studio_dto::CoverDisclosureKind::Cleared,
        valid_until_ms: Some(u64::try_from(world.now_ms() + 60_000).unwrap()),
    };
    let first = service
        .record(TOKEN, &world.work.work_id, request.clone())
        .await
        .unwrap();
    assert_eq!(
        (first.sequence, first.recorded_at_ms),
        (1, u64::try_from(world.now_ms()).unwrap())
    );
    // A lost response is recovered by replaying the same operation, even later.
    world.clock.advance(HOUR);
    assert_eq!(
        service
            .record(TOKEN, &world.work.work_id, request.clone())
            .await
            .unwrap(),
        first
    );
    let mut reused = request.clone();
    reused.decision = studio_dto::CoverDisclosureKind::Revoked;
    reused.valid_until_ms = None;
    assert!(matches!(
        service.record(TOKEN, &world.work.work_id, reused).await,
        Err(CoverDisclosureApplicationError::Disclosure(
            crate::contexts::media::domain::cover_disclosure::CoverDisclosureError::Image(
                crate::contexts::media::domain::image::ImageError::OperationReused
            )
        ))
    ));
    // A clearance window that already closed, and a digest of no admitted image, are refused.
    let mut stale = request.clone();
    stale.operation_id = uuid::Uuid::new_v4().to_string();
    assert!(matches!(
        service.record(TOKEN, &world.work.work_id, stale).await,
        Err(CoverDisclosureApplicationError::Disclosure(
            crate::contexts::media::domain::cover_disclosure::CoverDisclosureError::InvalidExpiry
        ))
    ));
    let mut unknown = request.clone();
    unknown.operation_id = uuid::Uuid::new_v4().to_string();
    unknown.image_asset_digest = format!("ia-r1:sha256:{}", "e".repeat(64));
    unknown.valid_until_ms = Some(u64::try_from(world.now_ms() + 60_000).unwrap());
    assert!(matches!(
        service.record(TOKEN, &world.work.work_id, unknown).await,
        Err(CoverDisclosureApplicationError::Disclosure(
            crate::contexts::media::domain::cover_disclosure::CoverDisclosureError::UnknownImage
        ))
    ));
    // Another actor (no owner, no delegate inference) is refused as not found.
    world
        .fixture
        .studio_caller("mallory", &"d".repeat(64))
        .await;
    let mut foreign = request.clone();
    foreign.operation_id = uuid::Uuid::new_v4().to_string();
    assert!(matches!(
        service
            .record(&"d".repeat(64), &world.work.work_id, foreign)
            .await,
        Err(CoverDisclosureApplicationError::Catalog(_))
    ));
    // The table refuses updates, deletes and a decision about another Work's image.
    for sql in [
        "UPDATE media_cover_disclosures SET decision='revoked',valid_until_ms=NULL",
        "DELETE FROM media_cover_disclosures",
    ] {
        assert!(world.fixture.try_exec(sql, &[]).await.is_err(), "{sql}");
    }
    // The SQL barrier independently refuses a decision naming no admitted image of this Work.
    let error = world
        .fixture
        .try_exec(
            "INSERT INTO media_cover_disclosures(work_id,sequence,owner_id,image_asset_digest,decision,valid_until_ms,recorded_by,recorded_at_ms,operation_id,fingerprint) VALUES(?1,9,'alice',?2,'revoked',NULL,'alice',1,?3,?4)",
            &[
                &world.work.work_id.as_str(),
                &format!("ia-r1:sha256:{}", "e".repeat(64)).as_str(),
                &uuid::Uuid::new_v4().to_string().as_str(),
                &format!("image-operation1:sha256:{}", "f".repeat(64)).as_str(),
            ],
        )
        .await
        .unwrap_err();
    assert!(
        format!("{error:?}").contains("binding.cover_disclosure"),
        "{error:?}"
    );
    assert_eq!(
        world
            .fixture
            .count("SELECT count(*) FROM media_cover_disclosures", &[])
            .await,
        1
    );
}

async fn send(
    app: &Router,
    request: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, headers, body)
}

fn get(path: &str) -> axum::http::request::Builder {
    Request::get(path)
}

#[tokio::test]
async fn origin_route_serves_only_the_shown_bytes_with_revalidation_headers() {
    let mut world = World::new().await;
    let a = world
        .admit(PNG, PNG_SHA256, studio_dto::ImageMime::Png)
        .await;
    world.select(Some(&a), ALT_A).await;
    world.republish().await;
    let work = world.work.work_id.clone();
    let path = format!("/api/v1/theatre/works/{work}/cover/{PNG_SHA256}");
    let app = world.router();

    // Before disclosure the composed route is the anonymous not_found, never cacheable.
    let (status, headers, body) = send(&app, get(&path).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"code": "not_found", "retryable": false})
    );

    // Studio records the clearance over HTTP with the owner's session.
    let disclose = Request::post(format!("/api/v1/studio/works/{work}/cover-disclosures"))
        .header(header::ORIGIN, STUDIO)
        .header(header::COOKIE, format!("cantos_session={TOKEN}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "operation_id": uuid::Uuid::new_v4().to_string(),
                "image_asset_digest": a.digest,
                "decision": "cleared",
                "valid_until_ms": world.now_ms() + 3_600_000,
            })
            .to_string(),
        ))
        .unwrap();
    let (status, headers, body) = send(&app, disclose).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let recorded: studio_dto::CoverDisclosureResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(recorded.decision, studio_dto::CoverDisclosureKind::Cleared);
    // A missing expiry is malformed: a clearance is never perpetual by default.
    let perpetual = Request::post(format!("/api/v1/studio/works/{work}/cover-disclosures"))
        .header(header::ORIGIN, STUDIO)
        .header(header::COOKIE, format!("cantos_session={TOKEN}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "operation_id": uuid::Uuid::new_v4().to_string(),
                "image_asset_digest": a.digest,
                "decision": "cleared",
            })
            .to_string(),
        ))
        .unwrap();
    assert_eq!(send(&app, perpetual).await.0, StatusCode::BAD_REQUEST);

    // Discovery v1.2 over HTTP carries the same URL the route serves.
    let (status, _, body) = send(
        &app,
        get(&format!("/api/v1/theatre/works/{work}?presentation=2"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let summary: dto::CoveredWorkSummary = serde_json::from_slice(&body).unwrap();
    let cover = summary.cover.expect("a shown cover");
    assert_eq!(cover.url, format!("{ORIGIN}{path}"));
    let text = String::from_utf8_lossy(&body);
    for private in [
        PROVENANCE,
        RIGHTS,
        a.id.as_str(),
        a.digest.as_str(),
        "alice",
        "private",
        "object_key",
        "valid_until",
    ] {
        assert!(!text.contains(private), "summary leaks {private}");
    }
    let (status, _, body) = send(
        &app,
        get("/api/v1/theatre/works?presentation=2")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let page: dto::CoveredWorkPage = serde_json::from_slice(&body).unwrap();
    assert_eq!(page.items[0].cover.as_ref(), Some(&cover));
    // Units at presentation=2 answer exactly as at presentation=1, apart from cursor identity.
    let units = |level: u8| {
        get(&format!(
            "/api/v1/theatre/works/{work}/units?presentation={level}"
        ))
        .body(Body::empty())
        .unwrap()
    };
    assert_eq!(send(&app, units(1)).await.2, send(&app, units(2)).await.2);

    // Shown: exact bytes, strong ETag, revalidate-every-use, no ranges.
    let (status, headers, body) = send(
        &app,
        get(&path)
            .header(header::RANGE, "bytes=0-3")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, PNG);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert_eq!(headers[header::CONTENT_LENGTH], PNG.len().to_string());
    assert_eq!(headers[header::ETAG], format!("\"{PNG_SHA256}\""));
    assert_eq!(headers[header::CACHE_CONTROL], "public, no-cache");
    assert_eq!(headers[header::ACCEPT_RANGES], "none");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    let (status, headers, body) =
        send(&app, Request::head(&path).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_empty());
    assert_eq!(headers[header::CONTENT_LENGTH], PNG.len().to_string());
    for tag in [
        format!("\"{PNG_SHA256}\""),
        format!("W/\"{PNG_SHA256}\""),
        "*".into(),
    ] {
        let (status, headers, body) = send(
            &app,
            get(&path)
                .header(header::IF_NONE_MATCH, tag)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_MODIFIED);
        assert!(body.is_empty());
        assert_eq!(headers[header::CACHE_CONTROL], "public, no-cache");
        assert_eq!(headers[header::ETAG], format!("\"{PNG_SHA256}\""));
    }

    // Every denial is the same anonymous not_found: stale/malformed SHA, unknown/malformed Work.
    for denied in [
        format!("/api/v1/theatre/works/{work}/cover/{JPEG_SHA256}"),
        format!(
            "/api/v1/theatre/works/{work}/cover/{}",
            PNG_SHA256.to_uppercase()
        ),
        format!("/api/v1/theatre/works/{work}/cover/not-a-sha"),
        format!("/api/v1/theatre/works/90000000-0000-4000-8000-000000000009/cover/{PNG_SHA256}"),
        format!("/api/v1/theatre/works/not-a-work/cover/{PNG_SHA256}"),
    ] {
        let (status, headers, body) = send(&app, get(&denied).body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{denied}");
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({"code": "not_found", "retryable": false})
        );
    }
    // A revalidation after revocation is denied on the very next request.
    world.disclose(&a, None).await.unwrap();
    let (status, headers, _) = send(
        &app,
        get(&path)
            .header(header::IF_NONE_MATCH, format!("\"{PNG_SHA256}\""))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");

    // An API host without public media composition never serves or names a cover.
    world.disclose(&a, Some(HOUR / 1000)).await.unwrap();
    let plain = api_router(AppState {
        store: world.store.clone(),
        web_origin: STUDIO.into(),
    });
    assert_eq!(
        send(&plain, get(&path).body(Body::empty()).unwrap())
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, _, body) = send(
        &plain,
        get(&format!("/api/v1/theatre/works/{work}?presentation=2"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let summary: dto::CoveredWorkSummary = serde_json::from_slice(&body).unwrap();
    assert_eq!(summary.cover, None);
}

#[tokio::test]
async fn avif_cover_intake_disclosure_and_origin_delivery_preserve_avif_content_type_and_etag() {
    let mut world = World::new().await;
    let avif = world
        .admit(AVIF, AVIF_SHA256, studio_dto::ImageMime::Avif)
        .await;
    world.select(Some(&avif), ALT_A).await;
    world.republish().await;
    world.disclose(&avif, Some(HOUR / 1000)).await.unwrap();
    let expected = world.expected(AVIF_SHA256, dto::CoverMime::Avif, AVIF.len(), ALT_A);
    assert_eq!(world.cover().await, Some(expected.clone()));

    let work = world.work.work_id.clone();
    let path = format!("/api/v1/theatre/works/{work}/cover/{AVIF_SHA256}");
    let app = world.router();
    let (status, headers, body) = send(&app, get(&path).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, AVIF);
    assert_eq!(headers[header::CONTENT_TYPE], "image/avif");
    assert_eq!(headers[header::CONTENT_LENGTH], AVIF.len().to_string());
    assert_eq!(headers[header::ETAG], format!("\"{AVIF_SHA256}\""));
    assert_eq!(headers[header::CACHE_CONTROL], "public, no-cache");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
}

#[tokio::test]
async fn stored_bytes_that_contradict_the_admitted_hash_are_never_served() {
    let mut world = World::new().await;
    let a = world
        .admit(PNG, PNG_SHA256, studio_dto::ImageMime::Png)
        .await;
    world.select(Some(&a), ALT_A).await;
    world.republish().await;
    world.disclose(&a, Some(HOUR / 1000)).await.unwrap();
    let path = format!(
        "/api/v1/theatre/works/{}/cover/{PNG_SHA256}",
        world.work.work_id
    );
    let app = world.router();
    assert_eq!(
        send(&app, get(&path).body(Body::empty()).unwrap()).await.0,
        StatusCode::OK
    );
    // Same length, different bytes, at the exact recorded private object.
    let object = find_object(&world.fixture.dir().join("private"), PNG_SHA256)
        .expect("the admitted object is on disk");
    let mut damaged = PNG.to_vec();
    let last = damaged.len() - 1;
    damaged[last] ^= 0xff;
    let mut permissions = std::fs::metadata(&object).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&object, permissions).unwrap();
    std::fs::write(&object, &damaged).unwrap();
    let (status, headers, body) = send(&app, get(&path).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"code": "unavailable", "retryable": false})
    );
    // The summary is metadata only: its title and cover facts are unaffected by the bytes.
    assert!(world.cover().await.is_some());
}

fn find_object(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    for entry in std::fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        if path.is_dir() {
            if let Some(found) = find_object(&path, name) {
                return Some(found);
            }
        } else if path
            .file_name()
            .is_some_and(|f| f.to_string_lossy().contains(name))
        {
            return Some(path);
        }
    }
    None
}

/// A restored or hand-edited file can hold damaged cover rows. Each damages only the cover:
/// the summary, its title and the units still answer (ADR 0033 rule 3), and the route denies.
#[tokio::test]
async fn damaged_cover_rows_fall_back_without_hiding_the_title() {
    for (trigger, statement) in [
        (
            "media_work_artwork_changes_no_update",
            "UPDATE media_work_artwork_changes SET alt_text='Bìa giả mạo' WHERE alt_text IS NOT NULL",
        ),
        (
            "media_image_assets_no_update",
            "UPDATE media_image_assets SET width_px=width_px+1",
        ),
    ] {
        let mut world = World::new().await;
        let a = world.admit(PNG, PNG_SHA256, studio_dto::ImageMime::Png).await;
        world.select(Some(&a), ALT_A).await;
        world.republish().await;
        world.disclose(&a, Some(HOUR / 1000)).await.unwrap();
        assert!(world.cover().await.is_some(), "{trigger}: shown before damage");
        world
            .fixture
            .exec(&format!("DROP TRIGGER {trigger}"), &[])
            .await
            .unwrap();
        world.fixture.exec(statement, &[]).await.unwrap();
        let service = world.covered();
        let detail = service.covered_work(&world.work.work_id).await.unwrap();
        assert_eq!(detail.cover, None, "{trigger}");
        assert_eq!(detail.title, "Ánh đèn cuối sân khấu");
        assert_eq!(
            service.covered_works("", None, None, 20, None).await.unwrap().items,
            vec![detail]
        );
        assert!(service
            .covered_units(&world.work.work_id, 20, None)
            .await
            .is_ok());
        assert_eq!(
            refusal(service.admit_cover(&world.work.work_id, PNG_SHA256).await),
            ListenerError::NotFound,
            "{trigger}"
        );
    }
}
