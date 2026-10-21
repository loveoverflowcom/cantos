use super::super::account_view::{Admitted, IntentRefusal};
use super::super::library_sync::RecoveryMode;
use super::*;
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

#[path = "publication_tests.rs"]
mod publication_tests;

fn id(value: u128) -> String {
    uuid::Uuid::from_u128(value).to_string()
}
fn account() -> AccountViewState {
    let mut state = AccountViewState::default();
    let issued = state.begin_refresh().unwrap();
    state
        .finish_session(
            issued,
            Ok(AccountScope::from_session("a".into(), id(1)).unwrap()),
            || id(100),
        )
        .unwrap();
    state
}

#[test]
fn admitted_noop_does_not_notify_the_production_writer_subscriber() {
    Owner::new().with(|| {
        let state = RwSignal::new(account());
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let subscriber = ArcMemo::new(move |_| {
            counted.fetch_add(1, Ordering::SeqCst);
            state.with(|s| s.phase())
        });
        subscriber.get();
        let before = state.with_untracked(Clone::clone);
        AccountWriter::new(state)
            .cancel_clear_history()
            .into_value()
            .unwrap()
            .unwrap();
        subscriber.get();
        assert_eq!(state.with_untracked(Clone::clone), before);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn production_writer_orders_mint_persist_send_and_releases_guards_before_effects() {
    Owner::new().with(|| {
        let state = RwSignal::new(account());
        let writer = AccountWriter::new(state);
        let trace = RefCell::new(Vec::new());
        let Admitted {
            fence,
            value: Some(pending),
        } = writer
            .request_saved(
                &id(99),
                true,
                None,
                || {
                    trace.borrow_mut().push("mint");
                    id(500)
                },
                None,
            )
            .into_value()
            .unwrap()
            .unwrap()
        else {
            panic!()
        };
        let payload = pending.payload.clone();
        let sent = writer.dispatch_after_persist(
            fence,
            pending,
            |journal| {
                trace.borrow_mut().push("persist");
                assert_eq!(journal.pending[0].payload, payload);
                // A storage adapter may synchronously reenter the owner; no write/read guard survives.
                writer.record_journal_restore(
                    &state.with_untracked(AccountViewState::fence).unwrap(),
                    Ok(()),
                );
                Ok(())
            },
            |fence, pending| {
                trace.borrow_mut().push("send");
                assert!(state.with_untracked(|s| s.accepts(&fence)));
                let _ = writer
                    .record_unknown_outcome(&fence, &pending.operation_id)
                    .into_value();
                pending.operation_id
            },
        );
        assert_eq!(sent, Some(id(500)));
        assert_eq!(*trace.borrow(), ["mint", "persist", "send"]);
    });
}

#[test]
fn failed_persistence_never_sends_or_loses_the_original_pending_operation() {
    Owner::new().with(|| {
        for failure in [
            JournalStorageFault::Unavailable,
            JournalStorageFault::Corrupt,
        ] {
            let state = RwSignal::new(account());
            let writer = AccountWriter::new(state);
            let Admitted {
                fence,
                value: Some(pending),
            } = writer
                .request_saved(&id(99), true, None, || id(500), None)
                .into_value()
                .unwrap()
                .unwrap()
            else {
                panic!()
            };
            let original = pending.payload.clone();
            assert_eq!(
                writer.dispatch_after_persist(
                    fence,
                    pending,
                    |_| Err(failure),
                    |_, _| panic!("must not send")
                ),
                None::<()>
            );
            state.with_untracked(|state| {
                assert_eq!(state.storage_fault(), Some(failure));
                let pending = state.sync().unwrap().pending_mutation(&id(500)).unwrap();
                assert_eq!(pending.payload, original);
                assert_eq!(pending.recovery, RecoveryMode::ReadReceiptFirst);
            });
            let retried = writer
                .retry_saved(&id(99), || panic!("must not mint on unknown outcome"), None)
                .into_value()
                .unwrap()
                .unwrap()
                .value
                .unwrap();
            assert_eq!(retried.operation_id, id(500));
            assert_eq!(retried.payload, original);
            assert_eq!(retried.recovery, RecoveryMode::ReadReceiptFirst);
        }
    });
}

#[test]
fn stale_or_replaced_dispatch_cannot_persist_or_send_and_storage_reentry_is_readmitted() {
    Owner::new().with(|| {
        for switch_during_storage in [false, true] {
            let state = RwSignal::new(account());
            let writer = AccountWriter::new(state);
            let Admitted {
                fence,
                value: Some(pending),
            } = writer
                .request_saved(&id(99), true, None, || id(500), None)
                .into_value()
                .unwrap()
                .unwrap()
            else {
                panic!()
            };
            if !switch_during_storage {
                writer.begin_logout().into_value().flatten().unwrap();
            }
            assert_eq!(
                writer.dispatch_after_persist(
                    fence,
                    pending,
                    |_| {
                        assert!(
                            switch_during_storage,
                            "old dispatch must not persist B's journal"
                        );
                        writer.begin_logout().into_value().flatten().unwrap();
                        Ok(())
                    },
                    |_, _| panic!("old dispatch must not send")
                ),
                None::<()>
            );
        }
    });
}

#[test]
fn production_sync_writer_refusals_do_not_publish() {
    Owner::new().with(|| {
        let state = RwSignal::new(account());
        let writer = AccountWriter::new(state);
        let runs = Arc::new(AtomicUsize::new(0));
        let count = runs.clone();
        let observed = Memo::new(move |_| {
            state.with(|s| {
                count.fetch_add(1, Ordering::SeqCst);
                s.phase()
            })
        });
        assert_eq!(observed.get(), SessionPhase::SignedIn);
        let old = state.with_untracked(AccountViewState::fence).unwrap();
        let issued = writer.begin_logout().into_value().flatten().unwrap().0;
        assert_eq!(observed.get(), SessionPhase::LogoutPending);
        let before_runs = runs.load(Ordering::SeqCst);
        for _ in 0..3 {
            assert_eq!(
                writer
                    .request_saved(
                        &id(99),
                        true,
                        None,
                        || panic!("refused intent must not mint"),
                        None
                    )
                    .into_value(),
                Some(Err(IntentRefusal::SignedOut))
            );
            assert_eq!(
                writer.record_unknown_outcome(&old, &id(500)).into_value(),
                Some(Err(IntentRefusal::StaleFence))
            );
            observed.get();
        }
        assert_eq!(runs.load(Ordering::SeqCst), before_runs);
        assert_eq!(
            writer.finish_logout(issued, Ok(())).into_value(),
            Some(true)
        );
    });
}

#[test]
fn writer_publishes_fail_closed_exhaustion_instead_of_discarding_a_candidate() {
    Owner::new().with(|| {
        let mut initial = account();
        initial.set_generation_for_test(u64::MAX);
        let state = RwSignal::new(initial);
        let writer = AccountWriter::new(state);
        assert_eq!(
            writer.begin_refresh().into_value(),
            Some(Err(AccountError::Exhausted))
        );
        state.with_untracked(|s| {
            assert_eq!(s.phase(), SessionPhase::Failed(AccountFailure::Unavailable));
            assert!(s.library().is_none());
            assert!(s.sync().is_none());
            assert!(s.fence().is_none());
        });
    });
}

#[test]
fn unread_journal_is_never_overwritten_and_blocks_new_intents_but_not_recovery_facts() {
    Owner::new().with(|| {
        let state = RwSignal::new(account());
        let writer = AccountWriter::new(state);
        let admitted = writer
            .request_saved(&id(99), true, None, || id(500), None)
            .into_value()
            .unwrap()
            .unwrap();
        writer.record_journal_restore(&admitted.fence, Err(JournalStorageFault::Corrupt));
        assert!(!writer.persist_journal(|_| panic!("must preserve unread durable bytes")));
        assert_eq!(
            writer
                .request_saved(
                    &id(99),
                    false,
                    None,
                    || panic!("blocked intent must not mint"),
                    None
                )
                .into_value(),
            Some(Err(IntentRefusal::RestoreBlocked))
        );
        assert!(
            writer
                .record_unknown_outcome(&admitted.fence, &id(500))
                .into_value()
                .unwrap()
                .unwrap()
                .value
        );
        state.with_untracked(|state| {
            assert!(state.journal_restore_blocked());
            assert_eq!(
                state
                    .sync()
                    .unwrap()
                    .pending_mutation(&id(500))
                    .unwrap()
                    .recovery,
                RecoveryMode::ReadReceiptFirst
            );
        });
    });
}
