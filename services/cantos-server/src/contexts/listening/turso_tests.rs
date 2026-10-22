//! The Library over a real file-backed `server.db`: the production `LibraryService` and its Turso
//! adapter, the production HTTP router for the transport cases, and a publication seeded with
//! production domain values. Ported from the retired PostgreSQL oracles; every assertion that
//! still has a meaning is kept. Nothing here establishes the creator -> publication pipeline, a
//! device, or a live Identity provider (see `tests/theatre_business_journey.rs`).
use super::*;
use crate::{
    adapters::inbound::http::{router_with_listener_identity, AppState},
    contexts::{
        catalog::domain::Visibility,
        listening::auth::{
            AuthenticatedListener, AuthenticationError, AuthenticationFuture, ListenerAdmission,
            ListenerAuthenticator, ListenerCredentials,
        },
    },
    platform::{
        identity::domain::ActorId,
        identity_auth::{
            credential::{Channel, Secrets},
            decision::{Epoch, ObservedSecurity, Subject},
            session::SessionStore,
        },
        turso::Acknowledgement,
    },
    test_support::{
        library::{
            audio, bookmark as bookmark_write, bookmark_delete, history_clear, history_start,
            operation, progress as progress_write, saved as saved_write, uid,
        },
        SeededWork, ServerFixture, WorkSeed,
    },
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use cantos_contracts::listener_library as dto;
use cantos_store_turso::testing::FixedClock;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

/// 2030-03-17, in microseconds.
const T0: i64 = 1_900_000_000_000_000;

fn principal(actor: &str) -> AuthenticatedListener {
    AuthenticatedListener::from_verified_actor(ActorId::new(actor).unwrap())
}

struct World {
    fixture: ServerFixture,
    clock: Arc<FixedClock>,
    work: SeededWork,
    audio: dto::AudioIdentity,
}

impl World {
    fn service(&self) -> LibraryService {
        LibraryService::new(self.fixture.store().clone())
    }
    async fn count(&self, table: &str, who: &str) -> i64 {
        self.fixture
            .count(
                &format!("SELECT count(*) FROM {table} WHERE listener_id=?1"),
                &[&who],
            )
            .await
    }
}

async fn world() -> World {
    let clock = FixedClock::new(T0);
    let fixture = ServerFixture::with_clock(clock.clone()).await;
    let work = fixture
        .seed_work(
            None,
            &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Tập có âm thanh riêng tư"]),
        )
        .await;
    fixture.seed_actors(&["alice", "bob"]).await;
    let audio = audio(&work, &work.units[0]);
    World {
        fixture,
        clock,
        work,
        audio,
    }
}

async fn remember_audio(
    service: &LibraryService,
    who: &AuthenticatedListener,
    a: &dto::AudioIdentity,
) {
    service
        .put_progress(
            who,
            &a.publication_id,
            progress_write(a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
    service
        .set_saved(who, &a.work_id, saved_write(1, 90, 0, true))
        .await
        .unwrap();
    service
        .playback_started(who, history_start(a, 1, 90, 99, 0))
        .await
        .unwrap();
}

// ---- transport: the production router over a trusted host principal ---------------------------

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
                .ok_or(AuthenticationError::Unauthenticated)
        })
    }
}

fn app(store: crate::application::Store) -> Router {
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
        "../../../../../packages/contracts/schema/theatre/library/v1.schema.json"
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

// ---- restart, presentation and what is never disclosed -----------------------------------------

#[tokio::test]
async fn presented_library_survives_restart_and_does_not_disclose_edited_studio_heads() {
    let mut w = world().await;
    let alice = principal("alice");
    let a = w.audio.clone();
    remember_audio(&w.service(), &alice, &a).await;
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
    // A later, unreleased edit of the Work head must not label the owned release.
    w.fixture
        .edit_work(&w.work, |details| {
            details.title = "Bản nháp riêng chưa phát hành".into()
        })
        .await;
    w.fixture.restart().await;
    let restarted = w.service();
    let continued = restarted
        .presented_continue_listening(&alice, 1, None)
        .await
        .unwrap();
    assert_eq!(continued.items[0].presentation, Some(expected.clone()));
    assert_eq!(continued.items[0].progress.audio, a);
    assert_eq!(continued.items[0].resume, dto::ResumeStatus::ExactAvailable);
    assert_eq!(
        restarted
            .presented_saved_works(&alice, 1, None)
            .await
            .unwrap()
            .items[0]
            .presentation,
        Some(expected.work.clone())
    );
    assert_eq!(
        restarted
            .presented_history(&alice, 1, None)
            .await
            .unwrap()
            .items[0]
            .presentation,
        Some(expected.clone())
    );
    let router = app(w.fixture.store().clone());
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
async fn presented_library_never_substitutes_replacement_titles_for_owned_audio() {
    let w = world().await;
    let alice = principal("alice");
    let service = w.service();
    remember_audio(&service, &alice, &w.audio).await;
    let replacement = w
        .fixture
        .publish_replacement(None, &w.work, &w.work.units[0])
        .await;
    let replacement_id = replacement.release_id.clone().unwrap();
    let continued = service
        .presented_continue_listening(&alice, 1, None)
        .await
        .unwrap();
    assert_eq!(continued.items[0].progress.audio, w.audio);
    assert_eq!(
        continued.items[0].resume,
        dto::ResumeStatus::ReleaseReplaced
    );
    assert_eq!(continued.items[0].presentation, None);
    let history = service.presented_history(&alice, 1, None).await.unwrap();
    assert_eq!(history.items[0].audio, w.audio);
    assert_eq!(history.items[0].resume, dto::ResumeStatus::ReleaseReplaced);
    assert_eq!(history.items[0].presentation, None);
    // Saved is about the Work, so the Work's current representative labels it.
    let saved = service
        .presented_saved_works(&alice, 1, None)
        .await
        .unwrap();
    assert_eq!(
        saved.items[0].presentation.as_ref().unwrap().publication_id,
        replacement_id
    );
    assert_eq!(saved.items[0].saved.work_id, w.audio.work_id);
    // Withdrawing the replacement leaves no title anywhere, and the records themselves remain.
    let mut after = w.work.units[0].clone();
    after.chain = replacement;
    w.fixture.withdraw_release("alice", &after).await;
    let router = app(w.fixture.store().clone());
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
async fn presented_library_hides_titles_after_current_privacy_or_submission_revocation() {
    for revoke in [false, true] {
        let w = world().await;
        let alice = principal("alice");
        let service = w.service();
        remember_audio(&service, &alice, &w.audio).await;
        if revoke {
            w.fixture.revoke_submission("alice", &w.work.units[0]).await;
        } else {
            w.fixture
                .edit_work_visibility(&w.work, Visibility::Private)
                .await;
        }
        let continued = service
            .presented_continue_listening(&alice, 1, None)
            .await
            .unwrap();
        assert_eq!(continued.items[0].progress.audio, w.audio);
        assert_eq!(
            continued.items[0].resume,
            dto::ResumeStatus::AccessUnavailable
        );
        assert_eq!(continued.items[0].presentation, None);
        let saved = service
            .presented_saved_works(&alice, 1, None)
            .await
            .unwrap();
        assert_eq!(saved.items[0].saved.work_id, w.audio.work_id);
        assert_eq!(saved.items[0].presentation, None);
        let history = service.presented_history(&alice, 1, None).await.unwrap();
        assert_eq!(history.items[0].audio, w.audio);
        assert_eq!(
            history.items[0].resume,
            dto::ResumeStatus::AccessUnavailable
        );
        assert_eq!(history.items[0].presentation, None);
    }
}

// ---- idempotency, order and races --------------------------------------------------------------

#[tokio::test]
async fn duplicate_and_same_base_first_write_races_commit_one_revision_and_identical_receipts() {
    let mut w = world().await;
    let (a, p) = (w.audio.clone(), principal("alice"));
    let service = w.service();
    let original = progress_write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint);
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
    assert_eq!(w.count("listener_progress", "alice").await, 1);
    assert_eq!(w.count("listener_operations", "alice").await, 1);
    // The receipt is durable across a restart and still the original answer.
    drop(service);
    w.fixture.restart().await;
    let restarted = w.service();
    assert_eq!(
        restarted
            .receipt(&p, &original.operation.operation_id)
            .await
            .unwrap(),
        receipt
    );
    let one = progress_write(&a, 2, 90, 1, 500, dto::ProgressKind::ExplicitSeek);
    let other = progress_write(&a, 1, 91, 1, 1000, dto::ProgressKind::Checkpoint);
    let (x, y) = tokio::join!(
        restarted.put_progress(&p, &a.publication_id, one),
        restarted.put_progress(&p, &a.publication_id, other)
    );
    let outcomes = [x.unwrap().outcome, y.unwrap().outcome];
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
            .filter(|o| **o == dto::WriteOutcome::RevisionConflict)
            .count(),
        1
    );
    assert_eq!(
        restarted
            .progress(&p, &a.publication_id)
            .await
            .unwrap()
            .unwrap()
            .revision,
        2
    );
    assert_eq!(
        restarted
            .progress(&principal("bob"), &a.publication_id)
            .await
            .unwrap(),
        None
    );
    // Reusing an operation id for a different request is refused, not answered.
    let mut reused = original.clone();
    reused.position_ms = 7;
    assert_eq!(
        restarted.put_progress(&p, &a.publication_id, reused).await,
        Err(LibraryError::OperationReused)
    );
    // A stale device sequence is refused and changes nothing.
    assert_eq!(
        restarted
            .put_progress(
                &p,
                &a.publication_id,
                progress_write(&a, 2, 90, 2, 900, dto::ProgressKind::Checkpoint)
            )
            .await,
        Err(LibraryError::StaleSequence)
    );
}

#[tokio::test]
async fn first_write_and_save_unsave_races_preserve_one_authority_and_old_receipts() {
    let w = world().await;
    let (a, service) = (w.audio.clone(), w.service());
    let bob = principal("bob");
    let (x, y) = tokio::join!(
        service.put_progress(
            &bob,
            &a.publication_id,
            progress_write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint)
        ),
        service.put_progress(
            &bob,
            &a.publication_id,
            progress_write(&a, 1, 91, 0, 200, dto::ProgressKind::ExplicitSeek)
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
    let save = saved_write(1, 90, 0, true);
    let (saved, removed) = tokio::join!(
        service.set_saved(&p, &a.work_id, save.clone()),
        service.set_saved(&p, &a.work_id, saved_write(1, 91, 0, false))
    );
    let old = saved.unwrap();
    assert_eq!(old.outcome, dto::WriteOutcome::Accepted);
    assert!(matches!(
        removed.unwrap().outcome,
        dto::WriteOutcome::AlreadyCurrent | dto::WriteOutcome::RevisionConflict
    ));
    assert_eq!(service.saved(&p, &a.work_id).await.unwrap().revision, 1);
    let current = service
        .set_saved(&p, &a.work_id, saved_write(2, 90, 1, false))
        .await
        .unwrap();
    // The old receipt replays as it was, even though the state moved on.
    assert_eq!(service.set_saved(&p, &a.work_id, save).await.unwrap(), old);
    assert!(!service.saved(&p, &a.work_id).await.unwrap().saved);
    assert_eq!(service.saved(&p, &a.work_id).await.unwrap().revision, 2);
    assert_ne!(current.state, old.state);
    assert_eq!(w.count("listener_saved", "alice").await, 1);
}

#[tokio::test]
async fn clear_preserves_progress_saved_and_sessions_and_the_router_is_account_isolated() {
    let w = world().await;
    let (a, service, p) = (w.audio.clone(), w.service(), principal("alice"));
    let router = app(w.fixture.store().clone());
    let progress = progress_write(
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
        .set_saved(&p, &a.work_id, saved_write(1, 90, 0, true))
        .await
        .unwrap();
    let start = history_start(&a, 1, 90, 99, 0);
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
        .clear_history(&p, history_clear(2, 90, 1, 0))
        .await
        .unwrap();
    // A delayed start of the old epoch replays its receipt and does not resurrect the history.
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
    assert_eq!(w.count("listener_playback_sessions", "alice").await, 1);
    // A fresh start of the old epoch is a conflict that names the server's epoch.
    let late = service
        .playback_started(&p, history_start(&a, 3, 90, 100, 0))
        .await
        .unwrap();
    assert_eq!(late.outcome, dto::WriteOutcome::HistoryEpochConflict);
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
async fn replacement_and_withdrawal_retain_owned_progress_without_silent_substitution() {
    let w = world().await;
    let (a, service, p) = (w.audio.clone(), w.service(), principal("alice"));
    service
        .put_progress(
            &p,
            &a.publication_id,
            progress_write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
    let replacement = w
        .fixture
        .publish_replacement(None, &w.work, &w.work.units[0])
        .await;
    let projected = service.continue_listening(&p, 20, None).await.unwrap();
    assert_eq!(
        projected.items[0].resume,
        dto::ResumeStatus::ReleaseReplaced
    );
    assert_eq!(projected.items[0].progress.audio, a);
    let mut replaced = w.work.units[0].clone();
    replaced.chain = replacement;
    w.fixture.withdraw_release("alice", &replaced).await;
    assert_eq!(
        service
            .continue_listening(&p, 20, None)
            .await
            .unwrap()
            .items[0]
            .resume,
        dto::ResumeStatus::AccessUnavailable
    );
    // The owner can still move their own position on the exact audio they hold.
    assert_eq!(
        service
            .put_progress(
                &p,
                &a.publication_id,
                progress_write(&a, 2, 90, 1, 0, dto::ProgressKind::ExplicitSeek)
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
    // But a brand-new account cannot start on a release that is no longer current.
    assert_eq!(
        service
            .put_progress(
                &principal("bob"),
                &a.publication_id,
                progress_write(&a, 1, 90, 0, 5, dto::ProgressKind::Checkpoint)
            )
            .await,
        Err(LibraryError::NotFound)
    );
}

#[tokio::test]
async fn latest_completed_release_suppresses_older_unfinished_work_without_erasing_either_register()
{
    let w = world().await;
    let (a, service, p) = (w.audio.clone(), w.service(), principal("alice"));
    service
        .put_progress(
            &p,
            &a.publication_id,
            progress_write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
    let next = w
        .fixture
        .publish_replacement(None, &w.work, &w.work.units[0])
        .await;
    let b = crate::test_support::library::audio_of(&w.work, &w.work.units[0], &next);
    service
        .put_progress(
            &p,
            &b.publication_id,
            progress_write(
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
        &app(w.fixture.store().clone()),
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

// ---- atomicity and unknown commit outcomes -----------------------------------------------------

#[tokio::test]
async fn a_failed_receipt_rolls_back_progress_sequences_and_account_state_before_the_retry() {
    let w = world().await;
    let (a, service, p) = (w.audio.clone(), w.service(), principal("alice"));
    w.fixture
        .exec(
            "CREATE TRIGGER listener_test_receipt_failure BEFORE INSERT ON listener_operations BEGIN SELECT RAISE(ABORT,'cantos:synthetic.before_receipt'); END",
            &[],
        )
        .await
        .unwrap();
    let write = progress_write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint);
    assert_eq!(
        service
            .put_progress(&p, &a.publication_id, write.clone())
            .await,
        Err(LibraryError::Corrupt)
    );
    for table in [
        "listener_progress",
        "listener_operations",
        "listener_device_sequences",
        "listener_state",
    ] {
        assert_eq!(w.count(table, "alice").await, 0, "{table}");
    }
    w.fixture
        .exec("DROP TRIGGER listener_test_receipt_failure", &[])
        .await
        .unwrap();
    assert_eq!(
        service
            .put_progress(&p, &a.publication_id, write)
            .await
            .unwrap()
            .outcome,
        dto::WriteOutcome::Accepted
    );
    assert_eq!(w.count("listener_progress", "alice").await, 1);
}

#[tokio::test]
async fn a_lost_commit_acknowledgement_is_reconciled_by_replaying_the_operation() {
    let w = world().await;
    let (a, p) = (w.audio.clone(), principal("alice"));
    let write = progress_write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint);
    // COMMIT happens, the answer is lost: the write may be durable, so it is Unavailable, never a
    // rollback claim, and the same operation answers with the receipt that was committed.
    let mut store = w.fixture.store().clone();
    store.acknowledgement = Acknowledgement::LostAfterCommit;
    let lost = LibraryService::new(store);
    assert_eq!(
        lost.put_progress(&p, &a.publication_id, write.clone())
            .await,
        Err(LibraryError::Unavailable)
    );
    assert_eq!(w.count("listener_operations", "alice").await, 1);
    let receipt = w
        .service()
        .put_progress(&p, &a.publication_id, write)
        .await
        .unwrap();
    assert_eq!(receipt.outcome, dto::WriteOutcome::Accepted);
    assert_eq!(w.count("listener_progress", "alice").await, 1);
    assert_eq!(w.count("listener_operations", "alice").await, 1);
    assert_eq!(
        w.service()
            .receipt(&p, &receipt.operation_id)
            .await
            .unwrap(),
        receipt
    );

    // COMMIT never happens and that answer is lost too: nothing is durable, the retry is fresh.
    let again = progress_write(&a, 2, 90, 1, 700, dto::ProgressKind::ExplicitSeek);
    let mut store = w.fixture.store().clone();
    store.acknowledgement = Acknowledgement::LostAfterRollback;
    assert_eq!(
        LibraryService::new(store)
            .put_progress(&p, &a.publication_id, again.clone())
            .await,
        Err(LibraryError::Unavailable)
    );
    assert_eq!(w.count("listener_operations", "alice").await, 1);
    assert_eq!(
        w.service()
            .put_progress(&p, &a.publication_id, again)
            .await
            .unwrap()
            .outcome,
        dto::WriteOutcome::Accepted
    );
    assert_eq!(w.count("listener_operations", "alice").await, 2);
}

// ---- admission inside the business transaction -------------------------------------------------

struct Session {
    principal: AuthenticatedListener,
    token: String,
    actor: String,
    session: String,
}

/// A native Identity session issued through the production session adapter, and the principal the
/// Server would seal from it after admission.
async fn session(w: &World, byte: u8) -> Session {
    let channel = Channel::Native;
    let secrets = Secrets::new(channel).unwrap();
    let observed = ObservedSecurity {
        subject: Subject::from_bytes([byte; 16]).unwrap(),
        epoch: Epoch::from_bytes([0xe0; 16]).unwrap(),
        version: 1,
    };
    let issued = w
        .fixture
        .store()
        .sessions()
        .issue(
            channel,
            &observed,
            u64::try_from(T0 / 1_000_000).unwrap(),
            &secrets,
        )
        .await
        .unwrap();
    Session {
        principal: AuthenticatedListener::from_admitted_session(
            ActorId::new(issued.actor_id.clone()).unwrap(),
            ListenerAdmission::new(&secrets.access, channel, issued.session_id.clone()),
        ),
        token: secrets.access,
        actor: issued.actor_id,
        session: issued.session_id,
    }
}

async fn effects(w: &World, actor: &str) -> [i64; 5] {
    let mut out = [0; 5];
    for (slot, table) in out.iter_mut().zip([
        "listener_state",
        "listener_progress",
        "listener_operations",
        "listener_device_sequences",
        "listener_saved",
    ]) {
        *slot = w.count(table, actor).await;
    }
    out
}

#[tokio::test]
async fn a_logout_or_revocation_that_commits_first_denies_the_write_and_leaves_no_effect() {
    let w = world().await;
    let (a, service) = (w.audio.clone(), w.service());
    let s = session(&w, 0x11).await;
    // Admitted and working.
    assert_eq!(
        service
            .put_progress(
                &s.principal,
                &a.publication_id,
                progress_write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint)
            )
            .await
            .unwrap()
            .outcome,
        dto::WriteOutcome::Accepted
    );
    assert_eq!(effects(&w, &s.actor).await, [1, 1, 1, 1, 0]);
    // The principal was sealed before the logout (the middleware's admission), and the logout
    // commits before the next business unit begins.
    w.fixture
        .store()
        .logout(&s.token, Channel::Native)
        .await
        .unwrap();
    let before = effects(&w, &s.actor).await;
    for result in [
        service
            .put_progress(
                &s.principal,
                &a.publication_id,
                progress_write(&a, 2, 90, 1, 900, dto::ProgressKind::Checkpoint),
            )
            .await
            .map(|_| ()),
        service
            .set_saved(&s.principal, &a.work_id, saved_write(1, 90, 0, true))
            .await
            .map(|_| ()),
        service
            .playback_started(&s.principal, history_start(&a, 1, 90, 5, 0))
            .await
            .map(|_| ()),
        service
            .create_bookmark(&s.principal, &uid(0xb01), bookmark_write(1, &a, 1, 90, 10))
            .await
            .map(|_| ()),
        service
            .progress(&s.principal, &a.publication_id)
            .await
            .map(|_| ()),
        service
            .receipt(&s.principal, &operation(1, 1).operation_id)
            .await
            .map(|_| ()),
        service
            .continue_listening(&s.principal, 20, None)
            .await
            .map(|_| ()),
    ] {
        assert_eq!(result, Err(LibraryError::Unauthenticated));
    }
    assert_eq!(
        effects(&w, &s.actor).await,
        before,
        "no write or receipt after the logout"
    );
}

#[tokio::test]
async fn a_write_that_commits_before_the_logout_keeps_its_durable_receipt() {
    let w = world().await;
    let (a, service) = (w.audio.clone(), w.service());
    let s = session(&w, 0x12).await;
    let write = progress_write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint);
    let receipt = service
        .put_progress(&s.principal, &a.publication_id, write)
        .await
        .unwrap();
    w.fixture
        .store()
        .logout(&s.token, Channel::Native)
        .await
        .unwrap();
    // The account's rows and its receipt outlive the session that wrote them.
    assert_eq!(effects(&w, &s.actor).await, [1, 1, 1, 1, 0]);
    let stored = w
        .fixture
        .rows(
            "SELECT response_bytes FROM listener_operations WHERE listener_id=?1",
            &[&s.actor.as_str()],
        )
        .await
        .unwrap();
    let bytes: Vec<u8> = stored[0].get(0);
    assert_eq!(
        serde_json::from_slice::<dto::Receipt>(&bytes).unwrap(),
        receipt
    );
    // Another session of the same account (a new login) reads it back.
    let again = sessions_for(&w, &s.actor).await;
    assert_eq!(
        service
            .receipt(&again, &receipt.operation_id)
            .await
            .unwrap(),
        receipt
    );
}

/// A fresh native session for an existing account: the same Identity subject logs in again.
async fn sessions_for(w: &World, actor: &str) -> AuthenticatedListener {
    let channel = Channel::Native;
    let secrets = Secrets::new(channel).unwrap();
    let subject = w
        .fixture
        .rows(
            "SELECT subject FROM actor_external_identities WHERE actor_id=?1",
            &[&actor],
        )
        .await
        .unwrap()[0]
        .get::<_, String>(0);
    let issued = w
        .fixture
        .store()
        .sessions()
        .issue(
            channel,
            &ObservedSecurity {
                subject: Subject::from_hex(&subject).unwrap(),
                epoch: Epoch::from_bytes([0xe0; 16]).unwrap(),
                version: 1,
            },
            u64::try_from(T0 / 1_000_000).unwrap(),
            &secrets,
        )
        .await
        .unwrap();
    assert_eq!(issued.actor_id, actor);
    AuthenticatedListener::from_admitted_session(
        ActorId::new(issued.actor_id).unwrap(),
        ListenerAdmission::new(&secrets.access, channel, issued.session_id),
    )
}

#[tokio::test]
async fn admission_binds_the_actor_the_session_and_the_deadline_to_the_unit() {
    let w = world().await;
    let (a, service) = (w.audio.clone(), w.service());
    let s = session(&w, 0x13).await;
    let other = session(&w, 0x14).await;
    // A principal naming one actor with another actor's session is refused: the actor and the
    // session are both derived from the accepted authority and must agree.
    let crossed = AuthenticatedListener::from_admitted_session(
        ActorId::new(s.actor.clone()).unwrap(),
        ListenerAdmission::new(&other.token, Channel::Native, other.session.clone()),
    );
    let mismatched_session = AuthenticatedListener::from_admitted_session(
        ActorId::new(s.actor.clone()).unwrap(),
        ListenerAdmission::new(&s.token, Channel::Native, other.session.clone()),
    );
    let wrong_channel = AuthenticatedListener::from_admitted_session(
        ActorId::new(s.actor.clone()).unwrap(),
        ListenerAdmission::new(
            &s.token,
            Channel::Web(crate::platform::identity_auth::credential::Surface::Theatre),
            s.session.clone(),
        ),
    );
    for forged in [&crossed, &mismatched_session, &wrong_channel] {
        assert_eq!(
            service
                .put_progress(
                    forged,
                    &a.publication_id,
                    progress_write(&a, 1, 90, 0, 1, dto::ProgressKind::Checkpoint)
                )
                .await,
            Err(LibraryError::Unauthenticated)
        );
    }
    assert_eq!(effects(&w, &s.actor).await, [0; 5]);
    assert_eq!(effects(&w, &other.actor).await, [0; 5]);
    // A deactivated actor is refused with no effect.
    w.fixture
        .exec(
            "UPDATE actors SET active=0 WHERE id=?1",
            &[&s.actor.as_str()],
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .put_progress(
                &s.principal,
                &a.publication_id,
                progress_write(&a, 1, 90, 0, 1, dto::ProgressKind::Checkpoint)
            )
            .await,
        Err(LibraryError::Unauthenticated)
    );
    assert_eq!(effects(&w, &s.actor).await, [0; 5]);
    // The other session is unaffected, and an expired access secret is refused at the unit's stamp.
    assert_eq!(
        service
            .put_progress(
                &other.principal,
                &a.publication_id,
                progress_write(&a, 1, 90, 0, 1, dto::ProgressKind::Checkpoint)
            )
            .await
            .unwrap()
            .outcome,
        dto::WriteOutcome::Accepted
    );
    w.clock.advance(121_000_000);
    assert_eq!(
        service
            .put_progress(
                &other.principal,
                &a.publication_id,
                progress_write(&a, 2, 90, 1, 2, dto::ProgressKind::Checkpoint)
            )
            .await,
        Err(LibraryError::Unauthenticated)
    );
    assert_eq!(effects(&w, &other.actor).await, [1, 1, 1, 1, 0]);
}

// ---- bookmarks ---------------------------------------------------------------------------------

fn bm(n: u64) -> String {
    uid(0xb00 + n)
}

fn at(a: &dto::AudioIdentity, tenths: u64) -> u64 {
    a.duration_ms * tenths / 10
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

async fn remove(r: &Router, actor: &str, id: u64, d: &dto::BookmarkDelete) -> (StatusCode, Value) {
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

fn ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["bookmark"]["bookmark_id"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn bookmark_create_list_delete_over_http_survive_restart_isolate_accounts_and_bound_positions(
) {
    let mut w = world().await;
    let a = w.audio.clone();
    let router = app(w.fixture.store().clone());
    let first = bookmark_write(1, &a, 1, 90, at(&a, 1));
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
        put(&router, "alice", &bookmark_write(1, &a, 2, 90, at(&a, 1)))
            .await
            .1["outcome"],
        "already_current"
    );
    for (id, seq, pos) in [(2, 1, at(&a, 2)), (3, 1, at(&a, 3)), (4, 1, a.duration_ms)] {
        assert_eq!(
            put(&router, "alice", &bookmark_write(id, &a, seq, 90, pos))
                .await
                .0,
            StatusCode::OK
        );
    }
    drop(router);
    w.fixture.restart().await;
    let router = app(w.fixture.store().clone());
    let (status, page) = list(&router, "alice", "?limit=3").await;
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
    let (_, rest) = list(&router, "alice", &format!("?limit=3&cursor={cursor}")).await;
    assert_eq!(ids(&rest), vec![bm(1)]);
    assert!(rest["next_cursor"].is_null());
    // A walk across a process restart is a refused cursor, never a silent restart of the walk.
    drop(router);
    w.fixture.restart().await;
    let router = app(w.fixture.store().clone());
    let (status, refused) = list(&router, "alice", &format!("?limit=3&cursor={cursor}")).await;
    assert_eq!(
        (status, refused["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_cursor"))
    );
    // Bounds: past the verified duration, an unknown release and a foreign path ID are refused.
    let (status, error) = put(
        &router,
        "alice",
        &bookmark_write(5, &a, 1, 90, a.duration_ms + 1),
    )
    .await;
    assert_eq!(
        (status, error["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("invalid_position"))
    );
    let mut unknown = bookmark_write(5, &a, 1, 90, 1);
    unknown.audio.publication_id = uid(0xdead);
    let (status, error) = put(&router, "alice", &unknown).await;
    assert_eq!(
        (status, error["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
    let mismatched = bookmark_write(6, &a, 1, 90, 1);
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
        remove(&router, "bob", 1, &bookmark_delete(1, 91)).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, theirs) = put(&router, "bob", &bookmark_write(1, &a, 1, 91, at(&a, 5))).await;
    assert_eq!(theirs["outcome"], "accepted");
    assert_eq!(theirs["state"]["current"]["position_ms"], at(&a, 5));
    assert_eq!(ids(&list(&router, "bob", "").await.1), vec![bm(1)]);
    // Delete: tombstone, never listed again, replays never resurrect it.
    let (status, gone) = remove(&router, "alice", 1, &bookmark_delete(3, 90)).await;
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
    let (status, refused) = put(&router, "alice", &bookmark_write(1, &a, 4, 90, at(&a, 1))).await;
    assert_eq!(
        (status, refused["outcome"].as_str()),
        (StatusCode::CONFLICT, Some("bookmark_deleted"))
    );
    assert_eq!(refused["resolution"], "keep_server");
    assert_eq!(refused["state"]["current"]["deleted"], true);
    assert_eq!(
        remove(&router, "alice", 1, &bookmark_delete(5, 90)).await.1["outcome"],
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
    let service = LibraryService::new(w.fixture.store().clone());
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
async fn concurrent_duplicate_create_and_delete_intents_commit_one_row_and_one_final_tombstone() {
    let w = world().await;
    let (a, service, p) = (w.audio.clone(), w.service(), principal("alice"));
    let id = bm(1);
    let (x, y, z) = tokio::join!(
        service.create_bookmark(&p, &id, bookmark_write(1, &a, 1, 90, at(&a, 1))),
        service.create_bookmark(&p, &id, bookmark_write(1, &a, 1, 91, at(&a, 1))),
        service.create_bookmark(&p, &id, bookmark_write(1, &a, 1, 92, at(&a, 1))),
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
    assert_eq!(w.count("listener_bookmarks", "alice").await, 1);
    let (d1, d2) = tokio::join!(
        service.delete_bookmark(&p, &id, bookmark_delete(2, 90)),
        service.delete_bookmark(&p, &id, bookmark_delete(2, 91)),
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
        service.create_bookmark(&p, &id, bookmark_write(1, &a, 3, 90, at(&a, 1))),
        service.create_bookmark(&p, &id, bookmark_write(1, &a, 3, 91, at(&a, 1))),
    );
    for r in [c1.unwrap(), c2.unwrap()] {
        assert_eq!(r.outcome, dto::WriteOutcome::BookmarkDeleted);
    }
    assert_eq!(
        w.fixture
            .count(
                "SELECT count(*) FROM listener_bookmarks WHERE listener_id='alice' AND deleted=1",
                &[]
            )
            .await,
        1
    );
    assert_eq!(
        w.fixture
            .count(
                "SELECT count(*) FROM listener_bookmarks WHERE listener_id='alice' AND deleted=0",
                &[]
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
async fn replaced_and_withdrawn_releases_keep_exact_bookmarks_without_remap_or_new_access() {
    let w = world().await;
    let (a, service, p) = (w.audio.clone(), w.service(), principal("alice"));
    service
        .create_bookmark(&p, &bm(1), bookmark_write(1, &a, 1, 90, at(&a, 1)))
        .await
        .unwrap();
    let replacement = w
        .fixture
        .publish_replacement(None, &w.work, &w.work.units[0])
        .await;
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
            .create_bookmark(&p, &bm(2), bookmark_write(2, &a, 2, 90, 100))
            .await,
        Err(LibraryError::NotFound),
        "a bookmark does not grant or restore access to a superseded release"
    );
    assert_eq!(
        service
            .create_bookmark(&p, &bm(1), bookmark_write(1, &a, 3, 90, at(&a, 1)))
            .await
            .unwrap()
            .outcome,
        dto::WriteOutcome::AlreadyCurrent
    );
    let mut replaced = w.work.units[0].clone();
    replaced.chain = replacement;
    w.fixture.withdraw_release("alice", &replaced).await;
    let page = service.bookmarks(&p, 20, None).await.unwrap();
    assert_eq!(page.items[0].resume, dto::ResumeStatus::AccessUnavailable);
    assert_eq!(page.items[0].presentation, None);
    // Deleting is still possible for the owner: it needs no listening access at all.
    assert_eq!(
        service
            .delete_bookmark(&p, &bm(1), bookmark_delete(4, 90))
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

/// The production CMP client (sync adapter, SQLite store, codec, HTTP client) against this real
/// router over loopback HTTP and the real `server.db`. The Kotlin side is `LiveBookmarkApiTest`;
/// this harness owns the server, the seeded rows and the final database assertions, so a Kotlin
/// pass cannot stand in for them. It runs through the Theatre trusted-host principal, not a real
/// Identity session (the production composition is covered by `tests/theatre_business_journey.rs`).
///
/// ```sh
/// CANTOS_TEST_GRADLE=/path/to/gradle cargo test -p cantos-server --lib \
///   cmp_production_client_against_the_real_api -- --ignored
/// ```
#[tokio::test]
#[ignore = "requires Gradle and the CMP desktop toolchain (CANTOS_TEST_GRADLE or gradle on PATH); no audio decoder or real identity provider"]
async fn cmp_production_client_against_the_real_api() {
    const SEEDED: u64 = 22;
    let w = world().await;
    let (a, service) = (w.audio.clone(), w.service());
    assert!(
        a.duration_ms >= 100,
        "fixture audio too short for distinct bookmark positions"
    );
    let alice = principal("alice");
    for n in 1..=SEEDED {
        service
            .create_bookmark(&alice, &bm(n), bookmark_write(n, &a, 1, 90, n))
            .await
            .unwrap();
    }
    let router = app(w.fixture.store().clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let identity = serde_json::to_string(&a).unwrap();
    let gradle = std::env::var("CANTOS_TEST_GRADLE").unwrap_or_else(|_| "gradle".into());
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(gradle)
            .current_dir(root)
            .args([
                "--project-dir",
                "apps/cantos-theatre/mobile",
                "--no-daemon",
                "-Dorg.gradle.java.installations.auto-download=false",
                "--max-workers",
                "2",
                "-Pkotlin.compiler.execution.strategy=in-process",
                "--no-configuration-cache",
                "-i",
                ":shared:desktopTest",
                "--rerun",
                "--tests",
                "com.cantos.theatre.adapters.library.LiveBookmarkApiTest",
            ])
            .env("CANTOS_LIVE_API_ORIGIN", format!("http://127.0.0.1:{port}"))
            .env("CANTOS_LIVE_AUDIO", identity)
            .env("CANTOS_LIVE_SEEDED", SEEDED.to_string())
            .output()
    })
    .await
    .unwrap()
    .expect("gradle could not be started");
    server.abort();
    let log = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{log}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        log.contains("LIVE-BOOKMARK-OK"),
        "the live test was skipped, not passed:\n{log}"
    );
    println!(
        "{}",
        log.lines()
            .find(|l| l.contains("LIVE-BOOKMARK-OK"))
            .unwrap_or_default()
            .trim()
    );
    // The database is the authority: what the client believes must be exactly what it holds.
    let count = |sql: String| {
        let fixture = &w.fixture;
        async move { fixture.count(&sql, &[]).await }
    };
    assert_eq!(
        count(
            "SELECT count(*) FROM listener_bookmarks WHERE listener_id='alice' AND deleted=0"
                .into()
        )
        .await,
        SEEDED as i64 + 2
    );
    assert_eq!(
        count(
            "SELECT count(*) FROM listener_bookmarks WHERE listener_id='alice' AND deleted=1"
                .into()
        )
        .await,
        1
    );
    assert_eq!(
        count("SELECT count(*) FROM listener_bookmarks WHERE listener_id='bob'".into()).await,
        0,
        "no row for another account"
    );
    assert_eq!(
        count("SELECT count(*) FROM (SELECT publication_id,position_ms FROM listener_bookmarks WHERE listener_id='alice' AND deleted=0 GROUP BY 1,2 HAVING count(*)>1) d".into()).await,
        0,
        "no duplicate at one release position"
    );
    assert_eq!(
        count(format!(
            "SELECT count(*) FROM listener_bookmarks WHERE listener_id='alice' AND publication_id<>'{}'",
            a.publication_id
        ))
        .await,
        0,
        "every row names the exact release"
    );
}
