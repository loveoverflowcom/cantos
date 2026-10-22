//! The Library's Turso adapter over a real file-backed `server.db`: the production
//! `LibraryService`, the real session adapter for the admission cases and a publication seeded
//! with production domain values. Ported from the retired PostgreSQL oracles; every assertion that
//! still has a meaning is kept. The transport cases (production router, CMP client) are in
//! `theatre_library_http_tests.rs`. Nothing here establishes the creator -> publication pipeline,
//! a device, or a live Identity provider (see `tests/theatre_business_journey.rs`).
use super::*;
use crate::{
    contexts::{
        catalog::domain::Visibility,
        listening::auth::{AuthenticatedListener, ListenerAdmission},
        media::ListenerService,
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
            bookmark as bookmark_write, bookmark_delete, history_start, operation, principal,
            progress as progress_write, remember_audio, saved as saved_write, uid, LibraryWorld,
            T0,
        },
        ServerFixture, WorkSeed,
    },
};
use cantos_contracts::listener_library as dto;

fn bm(n: u64) -> String {
    uid(0xb00 + n)
}

fn at(a: &dto::AudioIdentity, tenths: u64) -> u64 {
    a.duration_ms * tenths / 10
}

#[tokio::test]
async fn presented_library_hides_titles_after_current_privacy_or_submission_revocation() {
    for revoke in [false, true] {
        let w = LibraryWorld::new().await;
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
    let mut w = LibraryWorld::new().await;
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
    let w = LibraryWorld::new().await;
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
async fn replacement_and_withdrawal_retain_owned_progress_without_silent_substitution() {
    let w = LibraryWorld::new().await;
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
// ---- atomicity and unknown commit outcomes -----------------------------------------------------

#[tokio::test]
async fn a_failed_receipt_rolls_back_progress_sequences_and_account_state_before_the_retry() {
    let w = LibraryWorld::new().await;
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
    let w = LibraryWorld::new().await;
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
async fn session(w: &LibraryWorld, byte: u8) -> Session {
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

async fn effects(w: &LibraryWorld, actor: &str) -> [i64; 5] {
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
    let w = LibraryWorld::new().await;
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
    let w = LibraryWorld::new().await;
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
async fn sessions_for(w: &LibraryWorld, actor: &str) -> AuthenticatedListener {
    let channel = Channel::Native;
    let secrets = Secrets::new(channel).unwrap();
    let subject = w.fixture.subject_of(actor).await;
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
    let w = LibraryWorld::new().await;
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
    w.fixture.set_actor_active(&s.actor, false).await;
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
#[tokio::test]
async fn concurrent_duplicate_create_and_delete_intents_commit_one_row_and_one_final_tombstone() {
    let w = LibraryWorld::new().await;
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
    assert_eq!(w.fixture.bookmark_rows("alice", true).await, 1);
    assert_eq!(w.fixture.bookmark_rows("alice", false).await, 0);
    assert!(service
        .bookmarks(&p, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
}
#[tokio::test]
async fn replaced_and_withdrawn_releases_keep_exact_bookmarks_without_remap_or_new_access() {
    let w = LibraryWorld::new().await;
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
#[tokio::test]
async fn saved_is_labelled_by_the_release_work_detail_resolves() {
    let fixture = ServerFixture::new().await;
    let work = fixture
        .seed_work(
            None,
            &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Một", "Hai", "Ba"]),
        )
        .await;
    fixture.seed_actors(&["alice"]).await;
    let library = LibraryService::new(fixture.store().clone());
    let alice = principal("alice");
    // All three releases were published at the same instant: the representative is the smallest
    // release id, whatever order the rows were read in.
    let mut ids: Vec<_> = work
        .units
        .iter()
        .map(|u| u.chain.release_id.clone().unwrap())
        .collect();
    ids.sort();
    library
        .set_saved(&alice, &work.work_id, saved_write(1, 90, 0, true))
        .await
        .unwrap();
    let saved = library
        .presented_saved_works(&alice, 20, None)
        .await
        .unwrap();
    assert_eq!(saved.items.len(), 1);
    assert_eq!(
        saved.items[0].presentation.as_ref().unwrap().publication_id,
        ids[0],
        "Saved is labelled by the same release Work detail resolves"
    );
    let detail = ListenerService::new(fixture.store().clone())
        .presented_work(&work.work_id)
        .await
        .unwrap();
    assert_eq!(
        detail.title,
        saved.items[0].presentation.as_ref().unwrap().title
    );
}
