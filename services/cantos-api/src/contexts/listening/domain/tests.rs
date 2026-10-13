use super::*;
use proptest::prelude::*;
fn uid(v: u32) -> String {
    format!("00000000-0000-4000-8000-{v:012x}")
}
fn operation(seq: u64) -> dto::Operation {
    dto::Operation {
        operation_id: uid(seq as u32 + 100),
        device_id: uid(90),
        device_sequence: seq,
        device_reported_at_ms: None,
    }
}
fn audio() -> dto::AudioIdentity {
    dto::AudioIdentity {
        work_id: uid(1),
        unit_id: uid(2),
        publication_id: uid(3),
        rendition_id: uid(4),
        sha256: "a".repeat(64),
        duration_ms: 60_000,
    }
}
fn write(base: u64, pos: u64, kind: dto::ProgressKind) -> ProgressCandidate {
    ProgressCandidate::new(&dto::ProgressWrite {
        operation: operation(base + 1),
        audio: audio(),
        base_revision: base,
        position_ms: pos,
        kind,
    })
    .unwrap()
}
fn apply(current: Option<&ProgressRecord>, candidate: &ProgressCandidate) -> ProgressOutcome {
    reconcile_progress(current, candidate, 1000, Revision::new(100).unwrap()).unwrap()
}
fn accepted(out: ProgressOutcome) -> ProgressRecord {
    match out {
        ProgressOutcome::Accepted(r) => r,
        other => panic!("{other:?}"),
    }
}
#[test]
fn current_revision_admits_legitimate_rewind_and_stale_write_preserves_it() {
    let first = accepted(apply(
        None,
        &write(0, 30_000, dto::ProgressKind::Checkpoint),
    ));
    let rewind = accepted(apply(
        Some(&first),
        &write(1, 1000, dto::ProgressKind::ExplicitSeek),
    ));
    assert_eq!((rewind.raw().revision, rewind.raw().position_ms), (2, 1000));
    assert_eq!(
        apply(
            Some(&rewind),
            &write(1, 59_000, dto::ProgressKind::Checkpoint)
        ),
        ProgressOutcome::Conflict {
            current: Some(rewind.clone()),
            resolution: dto::Resolution::KeepServer
        }
    );
    assert_eq!(
        apply(
            Some(&rewind),
            &write(1, 800, dto::ProgressKind::ExplicitSeek)
        ),
        ProgressOutcome::Conflict {
            current: Some(rewind.clone()),
            resolution: dto::Resolution::ListenerChooses
        }
    );
    assert_eq!(rewind.raw().position_ms, 1000);
}
#[test]
fn competing_sessions_require_choice_and_equal_stale_write_is_already_current() {
    let first = accepted(apply(None, &write(0, 1000, dto::ProgressKind::Checkpoint)));
    let second = accepted(apply(
        Some(&first),
        &write(1, 2000, dto::ProgressKind::Checkpoint),
    ));
    assert_eq!(
        apply(
            Some(&second),
            &write(1, 1900, dto::ProgressKind::Checkpoint)
        ),
        ProgressOutcome::Conflict {
            current: Some(second.clone()),
            resolution: dto::Resolution::ListenerChooses
        }
    );
    assert_eq!(
        apply(
            Some(&second),
            &write(1, 2000, dto::ProgressKind::Checkpoint)
        ),
        ProgressOutcome::AlreadyCurrent(second.clone())
    );
    let choice = accepted(apply(
        Some(&second),
        &write(2, 1900, dto::ProgressKind::ResolveHere),
    ));
    assert_eq!(choice.raw().revision, 3);
    assert_eq!(choice.raw().position_ms, 1900);
}
#[test]
fn end_position_is_not_completion_and_only_explicit_restart_resets_completion() {
    let end = accepted(apply(
        None,
        &write(0, 60_000, dto::ProgressKind::ExplicitSeek),
    ));
    assert_eq!(end.raw().completion, dto::Completion::InProgress);
    let done = accepted(apply(
        Some(&end),
        &write(1, 60_000, dto::ProgressKind::MarkCompleted),
    ));
    assert_eq!(done.raw().completion, dto::Completion::Completed);
    let sought = accepted(apply(
        Some(&done),
        &write(2, 2000, dto::ProgressKind::ExplicitSeek),
    ));
    assert_eq!(sought.raw().completion, dto::Completion::Completed);
    let restart = accepted(apply(
        Some(&sought),
        &write(3, 0, dto::ProgressKind::Restart),
    ));
    assert_eq!(restart.raw().completion, dto::Completion::InProgress);
}
#[test]
fn malformed_bounds_and_phantom_base_are_rejected_without_state_change() {
    let mut raw = dto::ProgressWrite {
        operation: operation(1),
        audio: audio(),
        base_revision: 0,
        position_ms: 60_001,
        kind: dto::ProgressKind::Checkpoint,
    };
    assert_eq!(
        ProgressCandidate::new(&raw),
        Err(DomainError::InvalidPosition)
    );
    raw.position_ms = 1;
    raw.kind = dto::ProgressKind::Restart;
    assert_eq!(
        ProgressCandidate::new(&raw),
        Err(DomainError::InvalidPosition)
    );
    raw.kind = dto::ProgressKind::MarkCompleted;
    assert_eq!(
        ProgressCandidate::new(&raw),
        Err(DomainError::InvalidPosition)
    );
    assert_eq!(
        apply(None, &write(1, 1000, dto::ProgressKind::Checkpoint)),
        ProgressOutcome::Conflict {
            current: None,
            resolution: dto::Resolution::ListenerChooses
        }
    );
}
#[test]
fn device_clocks_are_context_and_audio_replacement_never_changes_stored_identity() {
    let current = accepted(apply(None, &write(0, 1000, dto::ProgressKind::Checkpoint)));
    let original = write(1, 2000, dto::ProgressKind::Checkpoint);
    let mut future = original.clone();
    future.intent.reported_at = Some(i64::MAX);
    assert_eq!(
        apply(Some(&current), &original),
        apply(Some(&current), &future)
    );
    let mut other = original.clone();
    other.audio.publication = PublicationId::parse(&uid(5)).unwrap();
    assert_eq!(
        reconcile_progress(Some(&current), &other, 1000, Revision::new(100).unwrap()),
        Err(DomainError::BindingMismatch)
    );
    assert_eq!(
        resume_status(&original.audio, None, Some(&other.audio)),
        dto::ResumeStatus::ReleaseReplaced
    );
    assert_eq!(
        resume_status(&original.audio, Some(&original.audio), Some(&other.audio)),
        dto::ResumeStatus::ExactAvailable
    );
    assert_eq!(
        resume_status(&original.audio, None, None),
        dto::ResumeStatus::AccessUnavailable
    );
    assert_eq!(current.raw().audio.publication_id, uid(3));
}
#[test]
fn desired_saved_state_retains_false_revision_and_refuses_delayed_divergence() {
    let c = SavedCandidate::new(
        &uid(1),
        &dto::SavedWrite {
            operation: operation(1),
            base_revision: 0,
            saved: true,
        },
    )
    .unwrap();
    let empty = SavedRecord::empty(c.work());
    let SavedOutcome::Accepted(saved) =
        reconcile_saved(&empty, &c, 100, Revision::new(1).unwrap()).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        reconcile_saved(&saved, &c, 200, Revision::new(2).unwrap()),
        Ok(SavedOutcome::AlreadyCurrent(saved.clone()))
    );
    let mut unsave = c.clone();
    unsave.saved = false;
    unsave.base = Revision::new(1).unwrap();
    let SavedOutcome::Accepted(removed) =
        reconcile_saved(&saved, &unsave, 300, Revision::new(2).unwrap()).unwrap()
    else {
        panic!()
    };
    assert_eq!((removed.raw().saved, removed.raw().revision), (false, 2));
    assert_eq!(
        reconcile_saved(&removed, &c, 400, Revision::new(3).unwrap()),
        Ok(SavedOutcome::Conflict(removed.clone()))
    );
    assert!(saved.raw().saved);
}
#[test]
fn history_epoch_and_seen_sessions_prevent_stale_acknowledgement_after_clear() {
    let start = HistoryStart::new(&dto::HistoryStart {
        operation: operation(1),
        audio: audio(),
        engine_session_id: uid(9),
        event: dto::PlaybackEvent::PlaybackStarted,
        history_epoch: 0,
    })
    .unwrap();
    let zero = HistoryState::restore(0, 0).unwrap();
    let HistoryOutcome::Accepted(one) = zero.start(&start, false).unwrap() else {
        panic!()
    };
    assert_eq!(
        one.start(&start, true),
        Ok(HistoryOutcome::AlreadyCurrent(one))
    );
    let clear = HistoryClear::new(&dto::HistoryClear {
        operation: operation(2),
        base_revision: 1,
        history_epoch: 0,
        confirmation: dto::ClearConfirmation::ClearHistory,
    })
    .unwrap();
    let HistoryOutcome::Accepted(cleared) = one.clear(&clear).unwrap() else {
        panic!()
    };
    assert_eq!(
        cleared.raw(),
        dto::HistoryState {
            revision: 2,
            epoch: 1
        }
    );
    assert_eq!(
        cleared.start(&start, false),
        Ok(HistoryOutcome::EpochConflict(cleared))
    );
    assert_eq!(
        cleared.clear(&clear),
        Ok(HistoryOutcome::EpochConflict(cleared))
    );
    let mut fresh = start;
    fresh.epoch = Revision::new(1).unwrap();
    assert_eq!(
        cleared.start(&fresh, true),
        Ok(HistoryOutcome::AlreadyCurrent(cleared))
    );
    assert!(matches!(
        cleared.start(&fresh, false),
        Ok(HistoryOutcome::Accepted(_))
    ));
}
#[test]
fn a_concurrent_actual_start_invalidates_stale_clear_confirmation() {
    let state = HistoryState::restore(3, 1).unwrap();
    let clear = HistoryClear::new(&dto::HistoryClear {
        operation: operation(2),
        base_revision: 2,
        history_epoch: 1,
        confirmation: dto::ClearConfirmation::ClearHistory,
    })
    .unwrap();
    assert_eq!(
        state.clear(&clear),
        Ok(HistoryOutcome::RevisionConflict(state))
    );
}
#[test]
fn refined_restore_rejects_corruption_and_revision_arithmetic_never_wraps() {
    assert_eq!(Revision::new(u64::MAX), Err(DomainError::InvalidRevision));
    assert_eq!(
        successor(i64::MAX as u64),
        Err(DomainError::RevisionExhausted)
    );
    let good = accepted(apply(None, &write(0, 1000, dto::ProgressKind::Checkpoint)));
    let mut nonzero_restart = good.raw().clone();
    nonzero_restart.kind = dto::ProgressKind::Restart;
    assert_eq!(
        ProgressRecord::restore(nonzero_restart),
        Err(DomainError::BindingMismatch)
    );
    let mut corrupt = good.raw().clone();
    corrupt.revision = 0;
    assert_eq!(
        ProgressRecord::restore(corrupt),
        Err(DomainError::InvalidPosition)
    );
    assert_eq!(
        HistoryState::restore(1, 2),
        Err(DomainError::BindingMismatch)
    );
    let mut bad = operation(1);
    bad.device_sequence = 0;
    assert_eq!(Intent::new(&bad), Err(DomainError::InvalidSequence));
}
proptest! {
    #[test]
    fn revision_classification_matches_independent_partition(base in any::<u64>(), current in any::<u64>(), same in any::<bool>(), protected in any::<bool>(), checkpoint in any::<bool>()) {
        let actual = classify(base, current, same, protected, checkpoint);
        let expected = match (base <= i64::MAX as u64 && current <= i64::MAX as u64, base == current, current == i64::MAX as u64, current > 0 && same, current > 0 && protected && checkpoint) {
            (false, _, _, _, _) => Err(DomainError::InvalidRevision),
            (true, true, true, _, _) => Err(DomainError::RevisionExhausted),
            (true, true, false, _, _) => Ok(Admission::Accept(current + 1)),
            (true, false, _, true, _) => Ok(Admission::AlreadyCurrent),
            (true, false, _, false, true) => Ok(Admission::KeepServer),
            (true, false, _, false, false) => Ok(Admission::ListenerChooses),
        };
        prop_assert_eq!(actual, expected);
    }
    #[test]
    fn arbitrary_two_device_schedule_accepts_only_observed_current_base(events in prop::collection::vec((any::<bool>(), 0u64..60_001, any::<bool>(), any::<bool>()), 1..100)) {
        let mut current: Option<ProgressRecord> = None;
        let mut bases = [0u64; 2]; let mut revision = 0u64;
        for (device, pos, seek, observe) in events {
            let index = usize::from(device);
            if observe { bases[index] = revision; }
            let c = write(bases[index], pos, if seek { dto::ProgressKind::ExplicitSeek } else { dto::ProgressKind::Checkpoint });
            let before = current.clone();
            match apply(current.as_ref(), &c) {
                ProgressOutcome::Accepted(next) => { prop_assert_eq!(bases[index], revision); revision += 1; prop_assert_eq!(next.raw().revision, revision); prop_assert_eq!(next.raw().position_ms, pos); bases[index] = revision; current = Some(next); }
                ProgressOutcome::AlreadyCurrent(r) => { prop_assert_eq!(Some(r), before.clone()); prop_assert_eq!(&current, &before); }
                ProgressOutcome::Conflict { current: returned, .. } => { prop_assert_eq!(returned, before.clone()); prop_assert_eq!(&current, &before); }
            }
        }
    }
}
