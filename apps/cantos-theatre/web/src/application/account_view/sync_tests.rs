use super::super::library_sync::{MutationPayload, RecoveryMode};
use super::*;
use crate::contract::library::{MutationState, ProgressWrite, WriteOutcome};
use crate::core::{
    playback::{CallbackIdentity, Source},
    public_ids::PublicWorkId,
    public_track::{PinnedRelease, ReleaseRequest},
};

fn id(value: u128) -> String {
    uuid::Uuid::from_u128(value).to_string()
}
fn signed_in() -> AccountViewState {
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
fn audio() -> AudioIdentity {
    serde_json::from_str::<ProgressWrite>(include_str!(
        "../../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/progress_write.json"
    ))
    .unwrap()
    .audio
}
fn source() -> Source {
    let manifest: crate::contract::discovery::PublicationManifest =
        serde_json::from_str(include_str!(
            "../../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/manifest.json"
        ))
        .unwrap();
    let request = ReleaseRequest::parse(
        PublicWorkId::parse(&manifest.work_id).unwrap(),
        &manifest.unit_id,
        &manifest.publication_id,
    )
    .unwrap();
    let release = PinnedRelease::admit(&manifest, request).unwrap();
    Source::public(release.track(), release.duration_ms()).unwrap()
}
fn record() -> SavedRecord {
    SavedRecord {
        work_id: id(99),
        saved: true,
        revision: 1,
        listener_order: 1,
        accepted_at_ms: 1000,
    }
}
fn receipt() -> Receipt {
    Receipt {
        operation_id: id(500),
        outcome: WriteOutcome::Accepted,
        resolution: None,
        state: MutationState::Saved { current: record() },
    }
}
fn unit<T>(result: SyncResult<T>) -> SyncResult<()> {
    result.map(|admitted| Admitted {
        fence: admitted.fence,
        value: (),
    })
}
type Intent = fn(&mut AccountViewState) -> SyncResult<()>;
type Completion = fn(&mut AccountViewState, &RequestFence) -> SyncResult<()>;

#[test]
fn every_user_capability_has_explicit_atomic_admission() {
    let intents: Vec<(&str, Intent)> = vec![
        ("request_saved", |s| {
            unit(s.request_saved(&id(99), true, None, || id(500), None))
        }),
        ("retry_saved", |s| {
            unit(s.retry_saved(&id(99), || id(501), None))
        }),
        ("mark_completed", |s| {
            unit(
                s.request_progress(audio(), ExplicitProgress::MarkCompleted, None, 1000, || {
                    id(500)
                }),
            )
        }),
        ("restart", |s| {
            unit(s.request_progress(audio(), ExplicitProgress::Restart, None, 1000, || id(500)))
        }),
        ("resolve_progress", |s| {
            unit(s.resolve_progress_conflict(&audio().publication_id, true, || id(500), None))
        }),
        ("retry_progress", |s| {
            unit(s.retry_progress(&audio().publication_id, || id(500), None))
        }),
        ("prompt_history", |s| unit(s.prompt_clear_history())),
        ("cancel_history", |s| unit(s.cancel_clear_history())),
        ("confirm_history", |s| {
            unit(s.confirm_clear_history(|| id(500), None))
        }),
        ("retry_history", |s| unit(s.retry_history(|| id(500), None))),
        ("begin_resume", |s| {
            unit(s.begin_resume(audio().publication_id, ResumeStatus::ExactAvailable, None))
        }),
        ("reset_resume", |s| unit(s.reset_resume())),
        ("ensure_saved", |s| unit(s.ensure_saved_read(&id(99), None))),
        ("ensure_progress", |s| {
            unit(s.ensure_progress_read(&audio().publication_id, None))
        }),
        ("playback_reset", |s| {
            unit(s.observe_playback(
                PlaybackObservation::SessionReset,
                None,
                1000,
                || id(50),
                || id(500),
            ))
        }),
        ("engine_started", |s| {
            unit(s.observe_playback(
                PlaybackObservation::EngineStarted {
                    identity: CallbackIdentity {
                        generation: 1,
                        track: source().track(),
                    },
                    source: source(),
                },
                None,
                1000,
                || id(50),
                || id(500),
            ))
        }),
        ("tick", |s| {
            unit(s.observe_playback(
                PlaybackObservation::PositionTick {
                    source: source(),
                    position_ms: 1000,
                },
                None,
                1000,
                || id(50),
                || id(500),
            ))
        }),
        ("seek", |s| {
            unit(s.observe_playback(
                PlaybackObservation::SeekConfirmed {
                    source: source(),
                    position_ms: 1000,
                },
                None,
                1000,
                || id(50),
                || id(500),
            ))
        }),
        ("flush", |s| {
            unit(s.observe_playback(
                PlaybackObservation::FlushCheckpoint {
                    source: source(),
                    position_ms: 1000,
                },
                None,
                1000,
                || id(50),
                || id(500),
            ))
        }),
    ];
    for (name, intent) in intents {
        let mut admitted = signed_in();
        assert_eq!(
            intent(&mut admitted),
            Ok(Admitted {
                fence: admitted.fence().unwrap(),
                value: ()
            }),
            "{name}"
        );
        for refusal in [
            IntentRefusal::SignedOut,
            IntentRefusal::RestoreBlocked,
            IntentRefusal::NoSync,
        ] {
            let mut state = signed_in();
            match refusal {
                IntentRefusal::SignedOut => {
                    state.begin_logout().unwrap();
                }
                IntentRefusal::RestoreBlocked => {
                    state.record_journal_restore(
                        &state.fence().unwrap(),
                        Err(JournalStorageFault::Corrupt),
                    );
                }
                IntentRefusal::NoSync => state.sync = None,
                IntentRefusal::StaleFence => unreachable!(),
            }
            let before = state.clone();
            assert_eq!(intent(&mut state), Err(refusal), "{name}");
            assert_eq!(state, before, "{name}: refusal mutated state");
        }
    }
}

#[test]
fn every_completion_rejects_old_fences_without_changing_any_state() {
    let completions: Vec<(&str, Completion)> = vec![
        ("reread_saved", |s, f| unit(s.reread_saved(f, &id(99)))),
        ("reread_progress", |s, f| {
            unit(s.reread_progress(f, &audio().publication_id))
        }),
        ("saved_read", |s, f| unit(s.finish_saved_read(f, record()))),
        ("saved_failure", |s, f| unit(s.fail_saved_read(f, &id(99)))),
        ("progress_read", |s, f| {
            unit(s.finish_progress_read(f, &audio().publication_id, None, || id(501), None))
        }),
        ("progress_failure", |s, f| {
            unit(s.fail_progress_read(f, &audio().publication_id))
        }),
        ("receipt", |s, f| {
            unit(s.apply_receipt(f, &id(500), receipt(), false, || id(501), None))
        }),
        ("unknown", |s, f| {
            unit(s.record_unknown_outcome(f, &id(500)))
        }),
        ("command_error", |s, f| {
            unit(s.record_command_error(f, &id(500), CommandError::Protocol, || id(102)))
        }),
        ("resume", |s, f| {
            unit(s.finish_resume(f, resume_ticket_for_test(), Ok(1000)))
        }),
    ];
    for (name, completion) in completions {
        let mut state = signed_in();
        let fence = state.fence().unwrap();
        assert!(completion(&mut state, &fence).is_ok(), "{name}");
        state.sync = None;
        let before = state.clone();
        assert_eq!(
            completion(&mut state, &fence),
            Err(IntentRefusal::NoSync),
            "{name}"
        );
        assert_eq!(state, before);
        let mut state = signed_in();
        let old = state.fence().unwrap();
        let issued = state.begin_logout().unwrap();
        let before = state.clone();
        assert_eq!(
            completion(&mut state, &old),
            Err(IntentRefusal::StaleFence),
            "{name}"
        );
        assert_eq!(state, before);
        state.finish_logout(issued, Ok(()));
        let lookup = state.begin_refresh().unwrap();
        state
            .finish_session(
                lookup,
                Ok(AccountScope::from_session("b".into(), id(2)).unwrap()),
                || id(101),
            )
            .unwrap();
        let before = state.clone();
        assert_eq!(
            completion(&mut state, &old),
            Err(IntentRefusal::StaleFence),
            "{name}"
        );
        assert_eq!(state, before, "{name}: leaked A completion into B");
    }
}

fn resume_ticket_for_test() -> ResumeTicket {
    signed_in()
        .begin_resume(audio().publication_id, ResumeStatus::ExactAvailable, None)
        .unwrap()
        .value
        .unwrap()
}

#[test]
fn receipt_first_recovery_retains_identity_payload_and_actor_binding_after_logout() {
    let mut state = signed_in();
    let admitted = state
        .request_saved(&id(99), true, None, || id(500), None)
        .unwrap();
    let first = admitted.value.unwrap();
    assert_eq!(first.recovery, RecoveryMode::SendIntent);
    assert!(
        state
            .record_unknown_outcome(&admitted.fence, &id(500))
            .unwrap()
            .value
    );
    let retry = state
        .retry_saved(&id(99), || panic!("must reuse original ID"), None)
        .unwrap()
        .value
        .unwrap();
    assert_eq!(retry.operation_id, first.operation_id);
    assert_eq!(retry.payload, first.payload);
    assert_eq!(retry.recovery, RecoveryMode::ReadReceiptFirst);
    let journal = state.sync().unwrap().to_journal();
    let cleanup = state.begin_logout().unwrap();
    assert!(state.sync().is_none());
    assert!(journal.clone().restore_for_actor("b").is_none());
    state.finish_logout(cleanup, Ok(()));
    let issued = state.begin_refresh().unwrap();
    state
        .finish_session(
            issued,
            Ok(AccountScope::from_session("a".into(), id(3)).unwrap()),
            || id(103),
        )
        .unwrap();
    let fence = state.fence().unwrap();
    assert!(state.restore_sync_journal(&fence, journal.clone()));
    assert_eq!(state.sync().unwrap().to_journal(), journal);
    let result = state
        .apply_receipt(&fence, &id(500), receipt(), true, || id(501), None)
        .unwrap()
        .value
        .unwrap();
    assert_eq!(
        result.reread,
        Some(super::super::library_sync::RereadTarget::Saved { work_id: id(99) })
    );
    assert_eq!(state.sync().unwrap().pending_mutation(&id(500)), None);
    assert_eq!(
        state.sync().unwrap().saved_work(&id(99)).unwrap().record,
        Some(record())
    );
}

#[test]
fn progress_history_and_resume_intents_return_exact_effects_and_ignore_old_tickets() {
    let mut state = signed_in();
    let pending = state
        .request_progress(audio(), ExplicitProgress::Restart, None, 1000, || id(500))
        .unwrap()
        .value
        .unwrap();
    let MutationPayload::Progress { write, .. } = pending.payload else {
        panic!()
    };
    assert_eq!(write.position_ms, 0);
    assert_eq!(write.kind, ProgressKind::Restart);
    let mut state = signed_in();
    let pending = state
        .request_progress(audio(), ExplicitProgress::MarkCompleted, None, 1000, || {
            id(500)
        })
        .unwrap()
        .value
        .unwrap();
    let MutationPayload::Progress { write, .. } = pending.payload else {
        panic!()
    };
    assert_eq!(write.position_ms, audio().duration_ms);
    assert_eq!(write.kind, ProgressKind::MarkCompleted);
    state.seed_sync(|sync, _| {
        sync.observe_history_state(HistoryState {
            revision: 2,
            epoch: 1,
        })
    });
    assert_eq!(
        state.prompt_clear_history().unwrap().value,
        Some(HistoryState {
            revision: 2,
            epoch: 1
        })
    );
    state.cancel_clear_history().unwrap();
    assert_eq!(state.sync().unwrap().clear_confirmation(), None);
    state.prompt_clear_history().unwrap();
    let clear = state
        .confirm_clear_history(|| id(501), None)
        .unwrap()
        .value
        .unwrap();
    let MutationPayload::HistoryClear { write } = clear.payload else {
        panic!()
    };
    assert_eq!(write.base_revision, 2);
    let fence = state.fence().unwrap();
    let first = state
        .begin_resume(id(3), ResumeStatus::ExactAvailable, None)
        .unwrap()
        .value
        .unwrap();
    let second = state
        .begin_resume(id(4), ResumeStatus::ExactAvailable, None)
        .unwrap()
        .value
        .unwrap();
    let before = state.clone();
    assert!(!state.finish_resume(&fence, first, Ok(1000)).unwrap().value);
    assert_eq!(state, before);
    assert!(state.finish_resume(&fence, second, Ok(1234)).unwrap().value);
    assert_eq!(
        state.sync().unwrap().resume_attempt(),
        &ResumeAttemptState::Restored {
            publication_id: id(4),
            position_ms: 1234
        }
    );
}

#[test]
fn point_reads_failures_and_retries_follow_the_owned_registers() {
    let mut state = signed_in();
    let fence = state.fence().unwrap();
    assert!(state.ensure_saved_read(&id(99), None).unwrap().value);
    assert!(!state.ensure_saved_read(&id(99), None).unwrap().value);
    state.fail_saved_read(&fence, &id(99)).unwrap();
    assert!(!state.sync().unwrap().saved_work(&id(99)).unwrap().loading);
    assert!(state.reread_saved(&fence, &id(99)).unwrap().value);
    assert!(state.finish_saved_read(&fence, record()).unwrap().value);
    assert_eq!(
        state.sync().unwrap().saved_work(&id(99)).unwrap().record,
        Some(record())
    );
    let publication = audio().publication_id;
    assert!(
        state
            .ensure_progress_read(&publication, None)
            .unwrap()
            .value
    );
    assert!(
        !state
            .ensure_progress_read(&publication, None)
            .unwrap()
            .value
    );
    state.fail_progress_read(&fence, &publication).unwrap();
    assert!(
        !state
            .sync()
            .unwrap()
            .progress_for(&publication)
            .unwrap()
            .point_read_in_flight
    );
    assert!(state.reread_progress(&fence, &publication).unwrap().value);
    assert_eq!(
        state
            .request_progress(audio(), ExplicitProgress::Restart, None, 1000, || panic!(
                "wait for point read"
            ))
            .unwrap()
            .value,
        None
    );
    let pending = state
        .finish_progress_read(&fence, &publication, None, || id(500), None)
        .unwrap()
        .value
        .unwrap();
    assert!(
        state
            .record_unknown_outcome(&fence, &pending.operation_id)
            .unwrap()
            .value
    );
    let retried = state
        .retry_progress(&publication, || panic!("reuse progress operation"), None)
        .unwrap()
        .value
        .unwrap();
    assert_eq!(retried.payload, pending.payload);
    assert_eq!(retried.operation_id, pending.operation_id);
    assert_eq!(retried.recovery, RecoveryMode::ReadReceiptFirst);
    assert!(
        state
            .record_command_error(
                &fence,
                &pending.operation_id,
                CommandError::Protocol,
                || panic!("no rotation needed")
            )
            .unwrap()
            .value
    );
    assert_eq!(
        state
            .sync()
            .unwrap()
            .pending_mutation(&pending.operation_id),
        None
    );
    assert!(state
        .retry_progress(&publication, || id(501), None)
        .unwrap()
        .value
        .is_some());
}

#[test]
fn playback_capabilities_preserve_engine_session_checkpoint_seek_and_history_retry_meaning() {
    for variant in 0..3 {
        let mut state = signed_in();
        let source = source();
        let observation = match variant {
            0 => PlaybackObservation::PositionTick {
                source,
                position_ms: 1000,
            },
            1 => PlaybackObservation::SeekConfirmed {
                source,
                position_ms: 1000,
            },
            _ => PlaybackObservation::FlushCheckpoint {
                source,
                position_ms: 1000,
            },
        };
        let pending = state
            .observe_playback(
                observation,
                None,
                10000,
                || panic!("positions do not mint engine sessions"),
                || id(500),
            )
            .unwrap()
            .value
            .unwrap();
        let MutationPayload::Progress { write, .. } = pending.payload else {
            panic!()
        };
        assert_eq!(write.audio, audio_identity_from_source(source).unwrap());
        assert_eq!(write.position_ms, 1000);
        assert_eq!(
            write.kind,
            if variant == 1 {
                ProgressKind::ExplicitSeek
            } else {
                ProgressKind::Checkpoint
            }
        );
    }
    let mut state = signed_in();
    let source = source();
    let fence = state.fence().unwrap();
    state.seed_sync(|sync, _| {
        sync.observe_history_state(HistoryState {
            revision: 2,
            epoch: 1,
        })
    });
    let observation = PlaybackObservation::EngineStarted {
        identity: CallbackIdentity {
            generation: 1,
            track: source.track(),
        },
        source,
    };
    let pending = state
        .observe_playback(observation, None, 1000, || id(50), || id(500))
        .unwrap()
        .value
        .unwrap();
    let MutationPayload::HistoryStart { write } = &pending.payload else {
        panic!()
    };
    assert_eq!(write.engine_session_id, id(50));
    assert_eq!(write.history_epoch, 1);
    assert_eq!(
        state
            .observe_playback(
                observation,
                None,
                1001,
                || panic!("duplicate engine start"),
                || panic!("duplicate mutation")
            )
            .unwrap()
            .value,
        None
    );
    state
        .record_unknown_outcome(&fence, &pending.operation_id)
        .unwrap();
    let retried = state
        .retry_history(|| panic!("reuse history operation"), None)
        .unwrap()
        .value
        .unwrap();
    assert_eq!(retried.payload, pending.payload);
    assert_eq!(retried.operation_id, pending.operation_id);
    assert_eq!(retried.recovery, RecoveryMode::ReadReceiptFirst);
    assert_eq!(
        state
            .observe_playback(
                PlaybackObservation::SessionReset,
                None,
                1002,
                || panic!(),
                || panic!()
            )
            .unwrap()
            .value,
        None
    );
}
