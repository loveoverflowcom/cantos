//! Real PostgreSQL + Router regressions. Ignored until the owned disposable-cluster gate runs.
use super::*;
use crate::{
    contexts::{
        catalog::{
            domain::{ContentUnitId, UnitPatch, Visibility, WorkId, WorkPatch},
            CatalogCommand,
        },
        listening::auth::{
            AuthenticatedListener, AuthenticationFuture, ListenerAuthenticator, ListenerCredentials,
        },
        media::ListenerService,
    },
    interfaces::http::{router_with_listener_identity, AppState},
    listener_postgres_test_fixture::audio_lifecycle_http::publication_http::PublishedFixture,
    platform::identity::domain::ActorId,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;
fn uid(v: u64) -> String {
    format!("00000000-0000-4000-8000-{v:012x}")
}
fn principal(actor: &str) -> AuthenticatedListener {
    AuthenticatedListener::from_verified_actor(ActorId::new(actor).unwrap())
}
fn intent(sequence: u64, device: u64) -> dto::Operation {
    dto::Operation {
        operation_id: Uuid::new_v4().to_string(),
        device_id: uid(device),
        device_sequence: sequence,
        device_reported_at_ms: None,
    }
}
async fn world() -> (PublishedFixture, LibraryService, dto::AudioIdentity) {
    let f = PublishedFixture::listener_world().await;
    let p = f.listener_publish(0, Value::Null).await;
    let store = f.listener_handles().1;
    let m = ListenerService::new(store.clone())
        .manifest(p["release"]["id"].as_str().unwrap())
        .await
        .unwrap();
    (f, LibraryService::new(store), audio(&m).unwrap().raw())
}
fn write(
    a: &dto::AudioIdentity,
    sequence: u64,
    device: u64,
    base: u64,
    position: u64,
    kind: dto::ProgressKind,
) -> dto::ProgressWrite {
    dto::ProgressWrite {
        operation: intent(sequence, device),
        audio: a.clone(),
        base_revision: base,
        position_ms: position,
        kind,
    }
}
struct SyntheticVerifiedListener;
impl ListenerAuthenticator for SyntheticVerifiedListener {
    fn authenticate(&self, c: ListenerCredentials) -> AuthenticationFuture {
        let actor = match c.expose_to_verifier() {
            "synthetic-listener-alice" => Some("alice"),
            "synthetic-listener-bob" => Some("bob"),
            _ => None,
        };
        Box::pin(async move {
            actor
                .map(principal)
                .ok_or(super::super::auth::AuthenticationError::Unauthenticated)
        })
    }
}
fn app(store: Store) -> Router {
    router_with_listener_identity(
        AppState {
            store,
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "unused-static-root",
        Arc::new(SyntheticVerifiedListener),
    )
}
async fn request(
    app: &Router,
    method: &str,
    path: &str,
    actor: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(format!("/api/v1/theatre{path}"))
                .header(
                    "authorization",
                    format!("Bearer synthetic-listener-{actor}"),
                )
                .header("content-type", "application/json")
                .body(body.map_or(Body::empty(), |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = r.status();
    assert_eq!(r.headers()["cache-control"], "no-store");
    let value = serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let mut root: Value = serde_json::from_str(include_str!(
        "../../../../../../packages/contracts/schema/theatre/library/v1.schema.json"
    ))
    .unwrap();
    if status.is_success() && path.contains("presentation=1") {
        let definition = if path.starts_with("/library/continue") {
            "PresentedContinuePage"
        } else if path.starts_with("/library/saved") {
            "PresentedSavedPage"
        } else if path.starts_with("/library/history") {
            "PresentedHistoryPage"
        } else {
            panic!("unexpected presentation route: {path}");
        };
        root = json!({"$defs":root["$defs"],"$ref":format!("#/$defs/{definition}")});
    }
    assert!(
        jsonschema::validator_for(&root).unwrap().is_valid(&value),
        "{value}"
    );
    (status, value)
}

async fn remember_audio(
    service: &LibraryService,
    listener: &AuthenticatedListener,
    audio: &dto::AudioIdentity,
) {
    service
        .put_progress(
            listener,
            &audio.publication_id,
            write(audio, 1, 90, 0, 100, dto::ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
    service
        .set_saved(
            listener,
            &audio.work_id,
            dto::SavedWrite {
                operation: intent(1, 90),
                base_revision: 0,
                saved: true,
            },
        )
        .await
        .unwrap();
    service
        .playback_started(
            listener,
            dto::HistoryStart {
                operation: intent(1, 90),
                audio: audio.clone(),
                engine_session_id: uid(99),
                event: dto::PlaybackEvent::PlaybackStarted,
                history_epoch: 0,
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL; no native-engine or live-delivery claim"]
async fn presented_library_survives_restart_and_does_not_disclose_edited_studio_heads() {
    const STUDIO: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let (f, service, a) = world().await;
    let listener = principal("alice");
    remember_audio(&service, &listener, &a).await;
    let expected = dto::AudioPresentation {
        work: dto::WorkPresentation {
            work_id: a.work_id.clone(),
            publication_id: a.publication_id.clone(),
            title: "Ánh đèn cuối sân khấu".into(),
            kind: cantos_contracts::theatre::WorkKind::Podcast,
            language: "vi-VN".into(),
        },
        unit_id: a.unit_id.clone(),
        unit_title: "Tập có âm thanh riêng tư".into(),
    };
    let (_, store, mut config) = f.listener_handles();
    let work_id = WorkId::parse(&a.work_id).unwrap();
    let unit_id = ContentUnitId::parse(&a.unit_id).unwrap();
    let work = store.catalog_work(STUDIO, &work_id).await.unwrap();
    let unit = store.catalog_unit(STUDIO, &unit_id).await.unwrap();
    let edited = store
        .catalog_execute(
            STUDIO,
            &Uuid::new_v4().to_string(),
            CatalogCommand::PatchWork {
                id: work_id,
                expected_revision: work.revision(),
                patch: WorkPatch {
                    title: Some("Bản nháp riêng chưa phát hành".into()),
                    ..WorkPatch::default()
                },
            },
        )
        .await
        .unwrap();
    store
        .catalog_execute(
            STUDIO,
            &Uuid::new_v4().to_string(),
            CatalogCommand::PatchUnit {
                id: unit_id,
                expected_revision: unit.revision(),
                expected_work_revision: edited.work.revision(),
                patch: UnitPatch {
                    title: Some("Tên tập riêng chưa phát hành".into()),
                    ..UnitPatch::default()
                },
            },
        )
        .await
        .unwrap();
    config.user("cantos_app");
    let restarted_store = Store::new(config).unwrap();
    let restarted = LibraryService::new(restarted_store.clone());
    let continued = restarted
        .presented_continue_listening(&listener, 1, None)
        .await
        .unwrap();
    assert_eq!(continued.items[0].presentation, Some(expected.clone()));
    assert_eq!(continued.items[0].progress.audio, a);
    assert_eq!(continued.items[0].resume, dto::ResumeStatus::ExactAvailable);
    assert_eq!(
        restarted
            .presented_saved_works(&listener, 1, None)
            .await
            .unwrap()
            .items[0]
            .presentation,
        Some(expected.work.clone())
    );
    assert_eq!(
        restarted
            .presented_history(&listener, 1, None)
            .await
            .unwrap()
            .items[0]
            .presentation,
        Some(expected.clone())
    );
    let router = app(restarted_store);
    for (path, presentation) in [
        (
            "/library/continue",
            serde_json::to_value(&expected).unwrap(),
        ),
        (
            "/library/saved",
            serde_json::to_value(&expected.work).unwrap(),
        ),
        ("/library/history", serde_json::to_value(&expected).unwrap()),
    ] {
        let (status, legacy) = request(&router, "GET", path, "alice", None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(legacy["items"][0].get("presentation").is_none());
        let (status, presented) = request(
            &router,
            "GET",
            &format!("{path}?presentation=1&limit=1"),
            "alice",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(presented["items"][0]["presentation"], presentation);
        let (_, bob) = request(
            &router,
            "GET",
            &format!("{path}?presentation=1&limit=1"),
            "bob",
            None,
        )
        .await;
        assert_eq!(bob["items"], json!([]));
    }
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL; no retained-release or public-byte claim"]
async fn presented_library_never_substitutes_replacement_titles_for_owned_audio() {
    let (f, service, a) = world().await;
    let listener = principal("alice");
    remember_audio(&service, &listener, &a).await;
    let replacement = f.listener_publish(1, json!(a.publication_id)).await;
    let replacement_id = replacement["release"]["id"].as_str().unwrap();
    let continued = service
        .presented_continue_listening(&listener, 1, None)
        .await
        .unwrap();
    assert_eq!(continued.items[0].progress.audio, a);
    assert_eq!(
        continued.items[0].resume,
        dto::ResumeStatus::ReleaseReplaced
    );
    assert_eq!(continued.items[0].presentation, None);
    let history = service.presented_history(&listener, 1, None).await.unwrap();
    assert_eq!(history.items[0].audio, a);
    assert_eq!(history.items[0].resume, dto::ResumeStatus::ReleaseReplaced);
    assert_eq!(history.items[0].presentation, None);
    let saved = service
        .presented_saved_works(&listener, 1, None)
        .await
        .unwrap();
    assert_eq!(
        saved.items[0].presentation.as_ref().unwrap().publication_id,
        replacement_id
    );
    assert_eq!(saved.items[0].saved.work_id, a.work_id);
    f.listener_withdraw(replacement_id, 2).await;
    let router = app(f.listener_handles().1);
    for path in ["/library/continue", "/library/saved", "/library/history"] {
        let (_, page) = request(
            &router,
            "GET",
            &format!("{path}?presentation=1"),
            "alice",
            None,
        )
        .await;
        assert_eq!(page["items"].as_array().unwrap().len(), 1);
        assert_eq!(page["items"][0]["presentation"], Value::Null);
        if path != "/library/saved" {
            assert_eq!(page["items"][0]["resume"], "access_unavailable");
        }
    }
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL; changes affect only the disposable synthetic fixture"]
async fn presented_library_hides_titles_after_current_privacy_or_submission_revocation() {
    const STUDIO: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    for revoke in [false, true] {
        let (f, service, a) = world().await;
        let listener = principal("alice");
        remember_audio(&service, &listener, &a).await;
        let (_, store, _) = f.listener_handles();
        if revoke {
            store
                .revoke_audio_submission(STUDIO, &a.unit_id, f.listener_submission_id())
                .await
                .unwrap();
        } else {
            let id = WorkId::parse(&a.work_id).unwrap();
            let current = store.catalog_work(STUDIO, &id).await.unwrap();
            store
                .catalog_execute(
                    STUDIO,
                    &Uuid::new_v4().to_string(),
                    CatalogCommand::PatchWork {
                        id,
                        expected_revision: current.revision(),
                        patch: WorkPatch {
                            visibility: Some(Visibility::Private),
                            ..WorkPatch::default()
                        },
                    },
                )
                .await
                .unwrap();
        }
        let continued = service
            .presented_continue_listening(&listener, 1, None)
            .await
            .unwrap();
        assert_eq!(continued.items[0].progress.audio, a);
        assert_eq!(
            continued.items[0].resume,
            dto::ResumeStatus::AccessUnavailable
        );
        assert_eq!(continued.items[0].presentation, None);
        let saved = service
            .presented_saved_works(&listener, 1, None)
            .await
            .unwrap();
        assert_eq!(saved.items[0].saved.work_id, a.work_id);
        assert_eq!(saved.items[0].presentation, None);
        let history = service.presented_history(&listener, 1, None).await.unwrap();
        assert_eq!(history.items[0].audio, a);
        assert_eq!(
            history.items[0].resume,
            dto::ResumeStatus::AccessUnavailable
        );
        assert_eq!(history.items[0].presentation, None);
    }
}
#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL; no production issuer or physical-engine claim"]
async fn duplicate_and_same_base_first_write_races_commit_one_revision_and_identical_receipts() {
    let (f, service, a) = world().await;
    let p = principal("alice");
    let original = write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint);
    let (left, right) = tokio::join!(
        service.put_progress(&p, &a.publication_id, original.clone()),
        service.put_progress(&p, &a.publication_id, original.clone())
    );
    assert_eq!(left, right);
    let receipt = left.unwrap();
    for _ in 0..8 {
        assert_eq!(
            service
                .put_progress(&p, &a.publication_id, original.clone())
                .await,
            Ok(receipt.clone())
        );
    }
    assert_eq!(
        f.listener_handles()
            .0
            .query_one(
                "SELECT count(*) FROM listener_progress WHERE listener_id='alice'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        f.listener_handles()
            .0
            .query_one(
                "SELECT count(*) FROM listener_operations WHERE listener_id='alice'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    let restarted = LibraryService::new(f.listener_handles().1);
    assert_eq!(
        restarted
            .receipt(&p, &original.operation.operation_id)
            .await
            .unwrap(),
        receipt
    );
    let one = write(&a, 2, 90, 1, 500, dto::ProgressKind::ExplicitSeek);
    let other = write(&a, 1, 91, 1, 1000, dto::ProgressKind::Checkpoint);
    let (x, y) = tokio::join!(
        service.put_progress(&p, &a.publication_id, one),
        service.put_progress(&p, &a.publication_id, other)
    );
    let (x, y) = (x.unwrap(), y.unwrap());
    assert_eq!(
        [x.outcome, y.outcome]
            .iter()
            .filter(|o| **o == dto::WriteOutcome::Accepted)
            .count(),
        1
    );
    assert_eq!(
        [x.outcome, y.outcome]
            .iter()
            .filter(|o| **o == dto::WriteOutcome::RevisionConflict)
            .count(),
        1
    );
    assert_eq!(
        service
            .progress(&p, &a.publication_id)
            .await
            .unwrap()
            .unwrap()
            .revision,
        2
    );
    assert_eq!(
        service
            .progress(&principal("bob"), &a.publication_id)
            .await
            .unwrap(),
        None
    );
}
#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL and synthetic publication fixture"]
async fn clear_preserves_progress_saved_and_sessions_and_router_is_account_isolated() {
    let (f, service, a) = world().await;
    let store = f.listener_handles().1;
    let router = app(store);
    let p = principal("alice");
    let progress = write(
        &a,
        1,
        90,
        0,
        a.duration_ms,
        dto::ProgressKind::MarkCompleted,
    );
    assert_eq!(
        request(
            &router,
            "PUT",
            &format!("/progress/{}", a.publication_id),
            "alice",
            Some(serde_json::to_value(&progress).unwrap())
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(service
        .history(&p, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
    service
        .set_saved(
            &p,
            &a.work_id,
            dto::SavedWrite {
                operation: intent(1, 90),
                base_revision: 0,
                saved: true,
            },
        )
        .await
        .unwrap();
    let start = dto::HistoryStart {
        operation: intent(1, 90),
        audio: a.clone(),
        engine_session_id: uid(99),
        event: dto::PlaybackEvent::PlaybackStarted,
        history_epoch: 0,
    };
    let r = service.playback_started(&p, start.clone()).await.unwrap();
    assert_eq!(
        service.history(&p, 20, None).await.unwrap().items[0]
            .progress
            .as_ref()
            .unwrap()
            .completion,
        dto::Completion::Completed
    );
    service
        .clear_history(
            &p,
            dto::HistoryClear {
                operation: intent(2, 90),
                base_revision: 1,
                history_epoch: 0,
                confirmation: dto::ClearConfirmation::ClearHistory,
            },
        )
        .await
        .unwrap();
    assert_eq!(service.playback_started(&p, start).await.unwrap(), r);
    assert!(service
        .history(&p, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(service.saved(&p, &a.work_id).await.unwrap().saved);
    assert_eq!(
        service
            .progress(&p, &a.publication_id)
            .await
            .unwrap()
            .unwrap()
            .completion,
        dto::Completion::Completed
    );
    let (_, bob) = request(&router, "GET", "/library/history", "bob", None).await;
    assert_eq!(bob["items"], json!([]));
    let (_, bob) = request(
        &router,
        "GET",
        &format!("/progress/{}", a.publication_id),
        "bob",
        None,
    )
    .await;
    assert_eq!(bob, json!({"resource":"progress","current":null}));
    let (c, s) = request(
        &router,
        "GET",
        &format!("/operations/{}", progress.operation.operation_id),
        "bob",
        None,
    )
    .await;
    assert_eq!(
        (c, s["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
}
#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL; injected trigger is confined to the disposable test DB"]
async fn receipt_failure_rolls_back_progress_sequences_and_account_state_before_retry() {
    let (f, service, a) = world().await;
    let p = principal("alice");
    f.listener_handles().0.batch_execute("CREATE FUNCTION listener_test_receipt_failure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic before receipt' USING ERRCODE='55000'; END $$; CREATE TRIGGER listener_test_receipt_failure BEFORE INSERT ON listener_operations FOR EACH ROW EXECUTE FUNCTION listener_test_receipt_failure();").await.unwrap();
    let w = write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint);
    assert_eq!(
        service.put_progress(&p, &a.publication_id, w.clone()).await,
        Err(LibraryError::Corrupt)
    );
    for table in [
        "listener_progress",
        "listener_operations",
        "listener_device_sequences",
        "listener_state",
    ] {
        assert_eq!(
            f.listener_handles()
                .0
                .query_one(
                    &format!("SELECT count(*) FROM {table} WHERE listener_id='alice'"),
                    &[]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
    }
    f.listener_handles().0.batch_execute("DROP TRIGGER listener_test_receipt_failure ON listener_operations; DROP FUNCTION listener_test_receipt_failure();").await.unwrap();
    assert_eq!(
        service
            .put_progress(&p, &a.publication_id, w)
            .await
            .unwrap()
            .outcome,
        dto::WriteOutcome::Accepted
    );
}
#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL; no retained-release/public-byte claim"]
async fn replacement_and_withdrawal_retain_owned_progress_without_silent_substitution() {
    let (f, service, a) = world().await;
    let p = principal("alice");
    service
        .put_progress(
            &p,
            &a.publication_id,
            write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
    let replaced = f.listener_publish(1, json!(a.publication_id)).await;
    let replacement = replaced["release"]["id"].as_str().unwrap();
    let projected = service.continue_listening(&p, 20, None).await.unwrap();
    assert_eq!(
        projected.items[0].resume,
        dto::ResumeStatus::ReleaseReplaced
    );
    assert_eq!(projected.items[0].progress.audio, a);
    f.listener_withdraw(replacement, 2).await;
    assert_eq!(
        service
            .continue_listening(&p, 20, None)
            .await
            .unwrap()
            .items[0]
            .resume,
        dto::ResumeStatus::AccessUnavailable
    );
    assert_eq!(
        service
            .put_progress(
                &p,
                &a.publication_id,
                write(&a, 2, 90, 1, 0, dto::ProgressKind::ExplicitSeek)
            )
            .await
            .unwrap()
            .outcome,
        dto::WriteOutcome::Accepted
    );
    assert_eq!(
        service
            .progress(&p, &a.publication_id)
            .await
            .unwrap()
            .unwrap()
            .position_ms,
        0
    );
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL; exercises real conditional Save/Unsave and first-write races"]
async fn first_write_and_save_unsave_races_preserve_one_authority_and_old_receipts() {
    let (f, service, a) = world().await;
    let bob = principal("bob");
    let (x, y) = tokio::join!(
        service.put_progress(
            &bob,
            &a.publication_id,
            write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint)
        ),
        service.put_progress(
            &bob,
            &a.publication_id,
            write(&a, 1, 91, 0, 200, dto::ProgressKind::ExplicitSeek)
        )
    );
    let outcomes = [x.unwrap().outcome, y.unwrap().outcome];
    assert_eq!(
        outcomes
            .iter()
            .filter(|v| **v == dto::WriteOutcome::Accepted)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|v| **v == dto::WriteOutcome::RevisionConflict)
            .count(),
        1
    );
    assert_eq!(
        service
            .progress(&bob, &a.publication_id)
            .await
            .unwrap()
            .unwrap()
            .revision,
        1
    );
    let p = principal("alice");
    let save = dto::SavedWrite {
        operation: intent(1, 90),
        base_revision: 0,
        saved: true,
    };
    let remove = dto::SavedWrite {
        operation: intent(1, 91),
        base_revision: 0,
        saved: false,
    };
    let (saved, removed) = tokio::join!(
        service.set_saved(&p, &a.work_id, save.clone()),
        service.set_saved(&p, &a.work_id, remove)
    );
    let old = saved.unwrap();
    assert_eq!(old.outcome, dto::WriteOutcome::Accepted);
    assert!(matches!(
        removed.unwrap().outcome,
        dto::WriteOutcome::AlreadyCurrent | dto::WriteOutcome::RevisionConflict
    ));
    assert_eq!(service.saved(&p, &a.work_id).await.unwrap().revision, 1);
    let current = service
        .set_saved(
            &p,
            &a.work_id,
            dto::SavedWrite {
                operation: intent(2, 90),
                base_revision: 1,
                saved: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(service.set_saved(&p, &a.work_id, save).await.unwrap(), old);
    assert!(!service.saved(&p, &a.work_id).await.unwrap().saved);
    assert_eq!(service.saved(&p, &a.work_id).await.unwrap().revision, 2);
    assert_ne!(current.state, old.state);
    assert_eq!(
        f.listener_handles()
            .0
            .query_one(
                "SELECT count(*) FROM listener_saved WHERE listener_id='alice'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL; copied scalar filters must fail database checks"]
async fn snapshot_scalar_constraints_prevent_hidden_saved_and_completion_corruption() {
    let (f, service, a) = world().await;
    let p = principal("alice");
    service
        .put_progress(
            &p,
            &a.publication_id,
            write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
    service
        .set_saved(
            &p,
            &a.work_id,
            dto::SavedWrite {
                operation: intent(1, 90),
                base_revision: 0,
                saved: true,
            },
        )
        .await
        .unwrap();
    for sql in [
        "WITH changed AS (SELECT listener_id,work_id,convert_to((convert_from(snapshot,'UTF8')::jsonb || jsonb_build_object('revision',revision+1,'listener_order',listener_order+1))::text,'UTF8') AS bytes FROM listener_saved WHERE listener_id='alice') UPDATE listener_saved s SET saved=false,revision=s.revision+1,listener_order=s.listener_order+1,snapshot=c.bytes,snapshot_digest=sha256(c.bytes) FROM changed c WHERE s.listener_id=c.listener_id AND s.work_id=c.work_id",
        "WITH changed AS (SELECT listener_id,publication_id,convert_to((convert_from(snapshot,'UTF8')::jsonb || jsonb_build_object('revision',revision+1,'listener_order',listener_order+1))::text,'UTF8') AS bytes FROM listener_progress WHERE listener_id='alice') UPDATE listener_progress p SET completion='completed',revision=p.revision+1,listener_order=p.listener_order+1,snapshot=c.bytes,snapshot_digest=sha256(c.bytes) FROM changed c WHERE p.listener_id=c.listener_id AND p.publication_id=c.publication_id",
    ] {
        let e=f.listener_handles().0.execute(sql,&[]).await.unwrap_err();
        assert_eq!(e.code().map(|v|v.code()),Some("23514"));
    }
    assert_eq!(
        service.saved_works(&p, 20, None).await.unwrap().items.len(),
        1
    );
    assert_eq!(
        service
            .continue_listening(&p, 20, None)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL; latest-completed projection is tested through SQL and Router"]
async fn latest_completed_release_suppresses_older_unfinished_work_without_erasing_either_register()
{
    let (f, service, a) = world().await;
    let p = principal("alice");
    service
        .put_progress(
            &p,
            &a.publication_id,
            write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
    let next = f.listener_publish(1, json!(a.publication_id)).await;
    let manifest = ListenerService::new(f.listener_handles().1)
        .manifest(next["release"]["id"].as_str().unwrap())
        .await
        .unwrap();
    let b = audio(&manifest).unwrap().raw();
    service
        .put_progress(
            &p,
            &b.publication_id,
            write(
                &b,
                1,
                90,
                0,
                b.duration_ms,
                dto::ProgressKind::MarkCompleted,
            ),
        )
        .await
        .unwrap();
    assert!(service
        .continue_listening(&p, 1, None)
        .await
        .unwrap()
        .items
        .is_empty());
    let (_, page) = request(
        &app(f.listener_handles().1),
        "GET",
        "/library/continue?limit=1",
        "alice",
        None,
    )
    .await;
    assert_eq!(page, json!({"items":[],"next_cursor":null}));
    assert_eq!(
        service
            .progress(&p, &a.publication_id)
            .await
            .unwrap()
            .unwrap()
            .position_ms,
        100
    );
    assert_eq!(
        service
            .progress(&p, &b.publication_id)
            .await
            .unwrap()
            .unwrap()
            .completion,
        dto::Completion::Completed
    );
}

mod bookmarks {
    //! Release-bound bookmarks against real PostgreSQL through the production Router.
    use super::*;
    fn bm(n: u64) -> String {
        uid(0xb00 + n)
    }
    fn create(
        a: &dto::AudioIdentity,
        id: u64,
        seq: u64,
        device: u64,
        at: u64,
    ) -> dto::BookmarkWrite {
        dto::BookmarkWrite {
            operation: intent(seq, device),
            bookmark_id: bm(id),
            audio: a.clone(),
            position_ms: at,
        }
    }
    fn delete(seq: u64, device: u64) -> dto::BookmarkDelete {
        dto::BookmarkDelete {
            operation: intent(seq, device),
        }
    }
    async fn put(r: &Router, actor: &str, w: &dto::BookmarkWrite) -> (StatusCode, Value) {
        request(
            r,
            "PUT",
            &format!("/library/bookmarks/{}", w.bookmark_id),
            actor,
            Some(serde_json::to_value(w).unwrap()),
        )
        .await
    }
    async fn remove(
        r: &Router,
        actor: &str,
        id: u64,
        d: &dto::BookmarkDelete,
    ) -> (StatusCode, Value) {
        request(
            r,
            "POST",
            &format!("/library/bookmarks/{}/delete", bm(id)),
            actor,
            Some(serde_json::to_value(d).unwrap()),
        )
        .await
    }
    async fn list(r: &Router, actor: &str, query: &str) -> (StatusCode, Value) {
        request(r, "GET", &format!("/library/bookmarks{query}"), actor, None).await
    }
    /// Positions relative to the fixture's verified duration, distinct and strictly inside it.
    fn at(a: &dto::AudioIdentity, tenths: u64) -> u64 {
        assert!(
            a.duration_ms >= 100,
            "fixture audio too short for distinct positions"
        );
        a.duration_ms * tenths / 10
    }
    fn ids(page: &Value) -> Vec<String> {
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["bookmark"]["bookmark_id"].as_str().unwrap().to_owned())
            .collect()
    }

    #[tokio::test]
    #[ignore = "requires xtask disposable PostgreSQL; no native-engine or live-delivery claim"]
    async fn create_list_delete_over_http_survive_restart_isolate_accounts_and_bound_positions() {
        let (f, _service, a) = world().await;
        let store = f.listener_handles().1;
        let router = app(store.clone());
        let first = create(&a, 1, 1, 90, at(&a, 1));
        let (status, accepted) = put(&router, "alice", &first).await;
        assert_eq!(
            (status, accepted["outcome"].as_str()),
            (StatusCode::OK, Some("accepted"))
        );
        assert_eq!(
            accepted["state"]["current"]["audio"],
            serde_json::to_value(&a).unwrap()
        );
        // The same operation again is byte-identical, whatever happened in between.
        assert_eq!(
            put(&router, "alice", &first).await,
            (StatusCode::OK, accepted.clone())
        );
        assert_eq!(
            put(&router, "alice", &create(&a, 1, 2, 90, at(&a, 1)))
                .await
                .1["outcome"],
            "already_current"
        );
        for (id, seq, at) in [(2, 1, at(&a, 2)), (3, 1, at(&a, 3)), (4, 1, a.duration_ms)] {
            assert_eq!(
                put(&router, "alice", &create(&a, id, seq, 90, at)).await.0,
                StatusCode::OK
            );
        }
        // The account row is a real restart boundary: a new Router and service see the same rows.
        let restarted = app(store.clone());
        let (status, page) = list(&restarted, "alice", "?limit=3").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(ids(&page), vec![bm(4), bm(3), bm(2)]);
        let item = &page["items"][0];
        assert_eq!(item["resume"], "exact_available");
        assert_eq!(item["presentation"]["unit_id"], a.unit_id);
        assert_eq!(
            item["presentation"]["work"]["publication_id"],
            a.publication_id
        );
        let cursor = page["next_cursor"].as_str().unwrap().to_owned();
        let (_, rest) = list(&restarted, "alice", &format!("?limit=3&cursor={cursor}")).await;
        assert_eq!(ids(&rest), vec![bm(1)]);
        assert!(rest["next_cursor"].is_null());
        // A walk across a process restart is a refused cursor, never a silent restart of the walk.
        let (status, refused) = list(
            &app(store.clone()),
            "alice",
            &format!("?limit=3&cursor={cursor}"),
        )
        .await;
        assert_eq!(
            (status, refused["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("invalid_cursor"))
        );
        // Bounds: past the verified duration, an unknown release and a foreign path ID are refused.
        let (status, error) = put(&router, "alice", &create(&a, 5, 1, 90, a.duration_ms + 1)).await;
        assert_eq!(
            (status, error["code"].as_str()),
            (StatusCode::UNPROCESSABLE_ENTITY, Some("invalid_position"))
        );
        let mut unknown = create(&a, 5, 1, 90, 1);
        unknown.audio.publication_id = uid(0xdead);
        let (status, error) = put(&router, "alice", &unknown).await;
        assert_eq!(
            (status, error["code"].as_str()),
            (StatusCode::NOT_FOUND, Some("not_found"))
        );
        let mismatched = create(&a, 6, 1, 90, 1);
        let (status, _) = request(
            &router,
            "PUT",
            &format!("/library/bookmarks/{}", bm(7)),
            "alice",
            Some(serde_json::to_value(&mismatched).unwrap()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            list(&router, "alice", "?presentation=1").await.0,
            StatusCode::BAD_REQUEST
        );
        for limit in ["0", "51"] {
            assert_eq!(
                list(&router, "alice", &format!("?limit={limit}")).await.0,
                StatusCode::BAD_REQUEST
            );
        }
        // Authentication and account isolation.
        assert_eq!(
            list(&router, "mallory", "").await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            put(&router, "mallory", &first).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(ids(&list(&router, "bob", "").await.1), Vec::<String>::new());
        assert_eq!(
            remove(&router, "bob", 1, &delete(1, 91)).await.0,
            StatusCode::NOT_FOUND
        );
        let (_, theirs) = put(&router, "bob", &create(&a, 1, 1, 91, at(&a, 5))).await;
        assert_eq!(theirs["outcome"], "accepted");
        assert_eq!(theirs["state"]["current"]["position_ms"], at(&a, 5));
        assert_eq!(ids(&list(&router, "bob", "").await.1), vec![bm(1)]);
        // Delete: tombstone, never listed again, replays never resurrect it.
        let (status, gone) = remove(&router, "alice", 1, &delete(3, 90)).await;
        assert_eq!(
            (status, gone["outcome"].as_str()),
            (StatusCode::OK, Some("accepted"))
        );
        assert_eq!(gone["state"]["current"]["deleted"], true);
        assert_eq!(
            ids(&list(&router, "alice", "?limit=50").await.1),
            vec![bm(4), bm(3), bm(2)]
        );
        assert_eq!(
            put(&router, "alice", &first).await,
            (StatusCode::OK, accepted.clone())
        );
        let (status, refused) = put(&router, "alice", &create(&a, 1, 4, 90, at(&a, 1))).await;
        assert_eq!(
            (status, refused["outcome"].as_str()),
            (StatusCode::CONFLICT, Some("bookmark_deleted"))
        );
        assert_eq!(refused["resolution"], "keep_server");
        assert_eq!(refused["state"]["current"]["deleted"], true);
        assert_eq!(
            remove(&router, "alice", 1, &delete(5, 90)).await.1["outcome"],
            "already_current"
        );
        assert_eq!(
            ids(&list(&router, "alice", "?limit=50").await.1),
            vec![bm(4), bm(3), bm(2)]
        );
        // Lost response: the receipt is readable and carries the original answer.
        let (status, receipt) = request(
            &router,
            "GET",
            &format!("/operations/{}", first.operation.operation_id),
            "alice",
            None,
        )
        .await;
        assert_eq!((status, receipt), (StatusCode::OK, accepted));
        // Bookmarks leave Saved, progress and history alone (and the converse).
        let service = LibraryService::new(store);
        let p = principal("alice");
        remember_audio(&service, &p, &a).await;
        assert_eq!(
            service.bookmarks(&p, 50, None).await.unwrap().items.len(),
            3
        );
        assert_eq!(
            service.saved_works(&p, 20, None).await.unwrap().items.len(),
            1
        );
    }

    #[tokio::test]
    #[ignore = "requires xtask disposable PostgreSQL; exercises real concurrent bookmark intents"]
    async fn concurrent_duplicate_create_and_delete_intents_commit_one_row_and_one_final_tombstone()
    {
        let (f, service, a) = world().await;
        let p = principal("alice");
        let id = bm(1);
        let (x, y, z) = tokio::join!(
            service.create_bookmark(&p, &id, create(&a, 1, 1, 90, at(&a, 1))),
            service.create_bookmark(&p, &id, create(&a, 1, 1, 91, at(&a, 1))),
            service.create_bookmark(&p, &id, create(&a, 1, 1, 92, at(&a, 1))),
        );
        let outcomes = [x.unwrap(), y.unwrap(), z.unwrap()].map(|r| r.outcome);
        assert_eq!(
            outcomes
                .iter()
                .filter(|o| **o == dto::WriteOutcome::Accepted)
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|o| **o == dto::WriteOutcome::AlreadyCurrent)
                .count(),
            2
        );
        let count = |sql: &'static str| {
            let admin = f.listener_handles().0;
            async move { admin.query_one(sql, &[]).await.unwrap().get::<_, i64>(0) }
        };
        assert_eq!(
            count("SELECT count(*) FROM listener_bookmarks WHERE listener_id='alice'").await,
            1
        );
        let (d1, d2) = tokio::join!(
            service.delete_bookmark(&p, &id, delete(2, 90)),
            service.delete_bookmark(&p, &id, delete(2, 91)),
        );
        let outcomes = [d1.unwrap(), d2.unwrap()].map(|r| r.outcome);
        assert_eq!(
            outcomes
                .iter()
                .filter(|o| **o == dto::WriteOutcome::Accepted)
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|o| **o == dto::WriteOutcome::AlreadyCurrent)
                .count(),
            1
        );
        let (c1, c2) = tokio::join!(
            service.create_bookmark(&p, &id, create(&a, 1, 3, 90, at(&a, 1))),
            service.create_bookmark(&p, &id, create(&a, 1, 3, 91, at(&a, 1))),
        );
        for r in [c1.unwrap(), c2.unwrap()] {
            assert_eq!(r.outcome, dto::WriteOutcome::BookmarkDeleted);
        }
        assert_eq!(
            count("SELECT count(*) FROM listener_bookmarks WHERE listener_id='alice' AND deleted")
                .await,
            1
        );
        assert_eq!(
            count(
                "SELECT count(*) FROM listener_bookmarks WHERE listener_id='alice' AND NOT deleted"
            )
            .await,
            0
        );
        assert!(service
            .bookmarks(&p, 20, None)
            .await
            .unwrap()
            .items
            .is_empty());
    }

    #[tokio::test]
    #[ignore = "requires xtask disposable PostgreSQL; no retained-release/public-byte claim"]
    async fn replaced_and_withdrawn_releases_keep_exact_bookmarks_without_remap_or_new_access() {
        let (f, service, a) = world().await;
        let p = principal("alice");
        service
            .create_bookmark(&p, &bm(1), create(&a, 1, 1, 90, at(&a, 1)))
            .await
            .unwrap();
        let replaced = f.listener_publish(1, json!(a.publication_id)).await;
        let replacement = replaced["release"]["id"].as_str().unwrap().to_owned();
        let page = service.bookmarks(&p, 20, None).await.unwrap();
        assert_eq!(page.items[0].resume, dto::ResumeStatus::ReleaseReplaced);
        assert_eq!(
            page.items[0].presentation, None,
            "no replacement title labels the old release"
        );
        assert_eq!(
            page.items[0].bookmark.audio, a,
            "never remapped to the replacement"
        );
        assert_eq!(page.items[0].bookmark.position_ms, at(&a, 1));
        assert_eq!(
            service
                .create_bookmark(&p, &bm(2), create(&a, 2, 2, 90, 100))
                .await,
            Err(LibraryError::NotFound),
            "a bookmark does not grant or restore access to a superseded release"
        );
        assert_eq!(
            service
                .create_bookmark(&p, &bm(1), create(&a, 1, 3, 90, at(&a, 1)))
                .await
                .unwrap()
                .outcome,
            dto::WriteOutcome::AlreadyCurrent
        );
        f.listener_withdraw(&replacement, 2).await;
        let page = service.bookmarks(&p, 20, None).await.unwrap();
        assert_eq!(page.items[0].resume, dto::ResumeStatus::AccessUnavailable);
        assert_eq!(page.items[0].presentation, None);
        // Deleting is still possible for the owner: it needs no listening access at all.
        assert_eq!(
            service
                .delete_bookmark(&p, &bm(1), delete(4, 90))
                .await
                .unwrap()
                .outcome,
            dto::WriteOutcome::Accepted
        );
        assert!(service
            .bookmarks(&p, 20, None)
            .await
            .unwrap()
            .items
            .is_empty());
    }

    #[tokio::test]
    #[ignore = "requires xtask disposable PostgreSQL; table guards are exercised with the owner role"]
    async fn bookmark_table_refuses_resurrection_mutation_deletion_and_snapshot_drift() {
        let (f, service, a) = world().await;
        let p = principal("alice");
        service
            .create_bookmark(&p, &bm(1), create(&a, 1, 1, 90, at(&a, 1)))
            .await
            .unwrap();
        service
            .create_bookmark(&p, &bm(2), create(&a, 2, 1, 90, at(&a, 2)))
            .await
            .unwrap();
        service
            .delete_bookmark(&p, &bm(2), delete(2, 90))
            .await
            .unwrap();
        let admin = f.listener_handles().0;
        let code = |e: tokio_postgres::Error| e.code().map(|c| c.code().to_owned());
        let (live, gone) = (bm(1), bm(2));
        for (sql, id, expected) in [
            // A tombstone is final; a live value cannot change identity or position.
            ("UPDATE listener_bookmarks SET deleted=false,listener_order=listener_order+1 WHERE bookmark_id=$1", &gone, "55000"),
            ("UPDATE listener_bookmarks SET position_ms=position_ms+1,deleted=true,listener_order=listener_order+1 WHERE bookmark_id=$1", &live, "55000"),
            ("UPDATE listener_bookmarks SET deleted=true WHERE bookmark_id=$1", &live, "55000"),
            ("UPDATE listener_bookmarks SET listener_id='bob' WHERE bookmark_id=$1", &live, "55000"),
            ("DELETE FROM listener_bookmarks WHERE bookmark_id=$1", &live, "55000"),
            // The snapshot must name the scalar columns, including the tombstone flag.
            ("UPDATE listener_bookmarks SET deleted=true,listener_order=listener_order+1 WHERE bookmark_id=$1", &live, "23514"),
        ] {
            let error = admin.execute(sql, &[id]).await.unwrap_err();
            assert_eq!(code(error).as_deref(), Some(expected), "{sql}");
        }
        let error = admin
            .batch_execute("TRUNCATE listener_bookmarks")
            .await
            .unwrap_err();
        assert_eq!(code(error).as_deref(), Some("55000"));
        assert_eq!(
            service.bookmarks(&p, 20, None).await.unwrap().items.len(),
            1
        );
    }
}
