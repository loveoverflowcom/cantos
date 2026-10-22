use super::*;
use crate::{
    application::library_sync::*,
    core::{
        playback::{CallbackIdentity, Source},
        public_ids::PublicWorkId,
        public_track::{PinnedRelease, ReleaseRequest},
    },
};
use cantos_contracts::listener_library::*;

fn page(section: Section) -> LibraryPage {
    let bytes: &[u8] = match section {
        Section::Continue => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_continue.json")),
        Section::Saved => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_saved.json")),
        Section::History => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_history.json")),
    };
    crate::live::library::decode_page(section, bytes).unwrap()
}
fn source() -> Source {
    let mut manifest: cantos_contracts::theatre::PublicationManifest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../packages/contracts/fixtures/theatre/discovery/v1/manifest.json"
        )))
        .unwrap();
    manifest.duration_ms = 120_000; // synthetic long release for throttle/seek coverage
    let request = ReleaseRequest::parse(
        PublicWorkId::parse(&manifest.work_id).unwrap(),
        &manifest.unit_id,
        &manifest.publication_id,
    )
    .unwrap();
    let release = PinnedRelease::admit(&manifest, request).unwrap();
    Source::public(release.track(), release.duration_ms()).unwrap()
}
fn audio() -> AudioIdentity {
    audio_identity_from_source(source()).unwrap()
}
fn presentation() -> AudioPresentation {
    let LibraryPage::Continue(p) = page(Section::Continue) else {
        unreachable!()
    };
    p.items[0].presentation.clone().unwrap()
}
fn saved() -> SavedRecord {
    SavedRecord {
        work_id: id(99),
        saved: true,
        revision: 1,
        accepted_at_ms: 1000,
        listener_order: 1,
    }
}
fn progress() -> ProgressRecord {
    ProgressRecord {
        audio: audio(),
        position_ms: 1000,
        completion: Completion::InProgress,
        revision: 1,
        accepted_at_ms: 1000,
        listener_order: 1,
        written_by: id(100),
        kind: ProgressKind::Checkpoint,
    }
}
fn stale() -> RequestFence {
    let mut s = AccountViewState::default();
    let generation = s.begin_refresh().unwrap();
    s.finish_session(
        generation,
        Ok(AccountScope::from_session("b".into(), id(2)).unwrap()),
        || id(101),
    )
    .unwrap();
    s.fence().unwrap()
}

/// Independent test oracle, deliberately built from the public getters the UI/shell uses.
/// It does not call production publication snapshots, compare their types, or inspect flags.
/// The finite key universe includes every key writable by this fixture/action matrix plus
/// absent/secondary keys, and each getter's Option presence (not merely rendered defaults).
fn reader_facts(s: &AccountViewState) -> Vec<String> {
    let mut facts = vec![
        format!(
            "{:?}",
            (
                s.phase(),
                s.actor(),
                s.generation(),
                s.fence(),
                s.cleanup_in_flight(),
                s.cleanup_intent(),
                s.storage_fault(),
                s.journal_restore_blocked()
            )
        ),
        format!(
            "{:?}",
            (
                s.can_authenticate(),
                s.can_retry_logout(),
                s.can_persist_journal(),
                s.can_login(),
                s.login_refusal()
            )
        ),
    ];
    facts.push(format!(
        "{:?}",
        s.library()
            .map(|l| (&l.continuing, &l.saved, &l.history, &l.history_state))
    ));
    for section in Section::ALL {
        facts.push(format!(
            "{:?}",
            s.library().map(|l| (l.phase(section), l.has_more(section)))
        ));
    }
    let mut keys = vec![
        id(3),
        id(10),
        id(99),
        id(333),
        id(999),
        audio().work_id,
        audio().publication_id,
    ];
    for section in Section::ALL {
        match page(section) {
            LibraryPage::Continue(p) => {
                for r in p.items {
                    keys.extend([r.progress.audio.work_id, r.progress.audio.publication_id]);
                }
            }
            LibraryPage::Saved(p) => {
                for r in p.items {
                    keys.push(r.saved.work_id);
                }
            }
            LibraryPage::History(p) => {
                for r in p.items {
                    keys.extend([r.audio.work_id, r.audio.publication_id]);
                }
            }
        }
    }
    keys.sort();
    keys.dedup();
    facts.push(format!(
        "{:?}",
        s.sync().map(|sync| (
            sync.history_state(),
            sync.history_command(),
            sync.clear_confirmation(),
            sync.resume_attempt()
        ))
    ));
    for key in keys {
        facts.push(format!(
            "{:?}",
            s.sync().and_then(|sync| sync.saved_work(&key))
        ));
        facts.push(format!(
            "{:?}",
            s.sync().and_then(|sync| sync.progress_for(&key)).map(|p| (
                &p.record,
                p.point_read_resolved,
                p.point_read_in_flight,
                &p.command,
                &p.presentation
            ))
        ));
    }
    facts
}

fn checked<R>(state: RwSignal<AccountViewState>, call: impl FnOnce() -> Write<R>) -> R {
    let runs = Arc::new(AtomicUsize::new(0));
    let counter = runs.clone();
    let subscriber = Memo::new(move |_| {
        state.with(|_| ());
        counter.fetch_add(1, Ordering::SeqCst);
    });
    subscriber.get();
    let before = state.with_untracked(reader_facts);
    let Write::Done { value, published } = call() else {
        panic!("live owner disposed")
    };
    let changed = before != state.with_untracked(reader_facts);
    assert_eq!(
        published,
        if changed {
            Published::Yes
        } else {
            Published::No
        }
    );
    subscriber.get();
    assert_eq!(runs.load(Ordering::SeqCst), 1 + usize::from(changed));
    value
}

fn fixture(index: usize) -> AccountViewState {
    let mut s = account();
    match index {
        0 => return AccountViewState::default(),
        1 => {
            s.begin_refresh().unwrap();
        }
        2 => {
            let g = s.begin_logout().unwrap();
            s.finish_logout(g, Ok(()));
        }
        3 => (),
        4 => {
            for section in Section::ALL {
                let request = s.begin_library(section, false).unwrap();
                s.finish_page(&request, Ok(page(section))).unwrap();
            }
        }
        5 => {
            s.record_journal_restore(&s.fence().unwrap(), Err(JournalStorageFault::Corrupt));
        }
        6 => {
            let g = s.begin_refresh().unwrap();
            s.finish_session(
                g,
                Ok(AccountScope::from_session("a".into(), id(1)).unwrap()),
                || "invalid".into(),
            )
            .unwrap();
        }
        7 => s.set_generation_for_test(u64::MAX),
        8 => {
            s.begin_logout().unwrap();
        }
        9 => {
            let g = s.begin_logout().unwrap();
            s.finish_logout(g, Err(AccountFailure::Unavailable));
        }
        10 => {
            s.begin_library(Section::Saved, false).unwrap();
        }
        11..=14 => {
            s.request_saved(&id(99), true, None, || id(500), None)
                .unwrap();
            let fence = s.fence().unwrap();
            match index {
                12 => {
                    s.record_unknown_outcome(&fence, &id(500)).unwrap();
                }
                13 => {
                    s.record_command_error(&fence, &id(500), CommandError::Unavailable, || id(101))
                        .unwrap();
                }
                14 => {
                    let receipt = Receipt {
                        operation_id: id(500),
                        outcome: WriteOutcome::RevisionConflict,
                        resolution: Some(Resolution::KeepServer),
                        state: MutationState::Saved { current: saved() },
                    };
                    s.apply_receipt(&fence, &id(500), receipt, false, || id(501), None)
                        .unwrap()
                        .value
                        .unwrap();
                }
                _ => (),
            }
        }
        15 => {
            s.observe_playback(
                PlaybackObservation::PositionTick {
                    source: source(),
                    position_ms: 1000,
                },
                None,
                1000,
                || id(600),
                || id(500),
            )
            .unwrap();
        }
        16 => {
            s.ensure_progress_read(&audio().publication_id, None)
                .unwrap();
            s.observe_playback(
                PlaybackObservation::PositionTick {
                    source: source(),
                    position_ms: 1000,
                },
                None,
                1000,
                || id(600),
                || id(500),
            )
            .unwrap();
        }
        17 => {
            s.request_progress(audio(), ExplicitProgress::Restart, None, 1000, || id(500))
                .unwrap();
            s.apply_receipt(
                &s.fence().unwrap(),
                &id(500),
                Receipt {
                    operation_id: id(500),
                    outcome: WriteOutcome::RevisionConflict,
                    resolution: Some(Resolution::ListenerChooses),
                    state: MutationState::Progress {
                        current: Some(progress()),
                    },
                },
                false,
                || id(501),
                None,
            )
            .unwrap()
            .value
            .unwrap();
        }
        18..=20 => {
            s.seed_sync(|sync, _| {
                sync.observe_history_state(HistoryState {
                    revision: 1,
                    epoch: 0,
                });
            });
            s.prompt_clear_history().unwrap();
            if index >= 19 {
                s.confirm_clear_history(|| id(500), None).unwrap();
            }
            if index == 20 {
                s.record_command_error(
                    &s.fence().unwrap(),
                    &id(500),
                    CommandError::Unavailable,
                    || id(101),
                )
                .unwrap();
            }
        }
        21 => {
            s.begin_resume(
                audio().publication_id,
                ResumeStatus::ExactAvailable,
                Some(presentation()),
            )
            .unwrap();
        }
        22..=24 => {
            for section in Section::ALL {
                let request = s.begin_library(section, false).unwrap();
                s.finish_page(&request, Ok(page(section))).unwrap();
            }
            let section = match index {
                22 => Section::Saved,
                23 => Section::History,
                _ => Section::Continue,
            };
            let request = s.begin_library(section, true).unwrap();
            if index != 23 {
                s.finish_page(&request, Err(AccountFailure::Unavailable))
                    .unwrap();
            }
        }
        _ => unreachable!(),
    }
    s
}

#[test]
fn finite_reader_oracle_covers_every_production_writer_method_and_duplicate() {
    // Exhaustive Cartesian product: 25 named states × 52 calls × 2 fence choices × 2 repetitions.
    // This is a finite, deterministic property; it is not a random or formal proof.
    for seed in 0..25 {
        for action in 0..52 {
            for stale_fence in [false, true] {
                Owner::new().with(|| {
                let state = RwSignal::new(fixture(seed));
                let writer = AccountWriter::new(state);
                let current = state.with_untracked(AccountViewState::fence).unwrap_or_else(stale);
                let fence = if stale_fence { stale() } else { current };
                let generation = state.with_untracked(AccountViewState::generation);
                let mut request_state = account();
                let request = request_state.begin_library(Section::Saved, false).unwrap();
                let mut ticket_state = account();
                let ticket = ticket_state.begin_resume(audio().publication_id, ResumeStatus::ExactAvailable, None).unwrap().value.unwrap();
                for _ in 0..2 {
                    macro_rules! run { ($call:expr) => {{ let _ = checked(state, || $call); }}; }
                    match action {
                        0 => run!(writer.begin_refresh()),
                        1 => run!(writer.begin_logout()),
                        2 => run!(writer.credentials_lost()),
                        3 => run!(writer.finish_session(generation, Ok(AccountScope::from_session("a".into(), id(1)).unwrap()), || id(100))),
                        4 => run!(writer.finish_session(generation, Err(AccountFailure::SignedOut), || id(100))),
                        5 => run!(writer.finish_session(generation, Err(AccountFailure::Unavailable), || id(100))),
                        6 => run!(writer.finish_logout(generation, Ok(()))),
                        7 => run!(writer.finish_logout(generation, Err(AccountFailure::Unavailable))),
                        8 => run!(writer.finish_logout(generation, Err(AccountFailure::SessionChanged))),
                        9 => run!(writer.fail_fenced_command(&fence, AccountFailure::SignedOut)),
                        10 => run!(writer.fail_fenced_command(&fence, AccountFailure::SessionChanged)),
                        11 => run!(writer.begin_library(Section::Saved, false)),
                        12 => run!(writer.begin_library(Section::History, true)),
                        13 => run!(writer.begin_library(Section::Continue, true)),
                        14 => run!(writer.finish_page(&request, Ok(page(Section::Saved)), || id(501))),
                        15 => run!(writer.finish_page(&request, Err(AccountFailure::Unavailable), || id(501))),
                        16 => run!(writer.finish_session(generation, Err(AccountFailure::LoginRefused(crate::application::account_view::LoginRefusal::Credentials)), || id(100))),
                        17 => run!(writer.restore_sync_journal(&fence, AccountSyncJournal::new("a", id(100)).unwrap())),
                        18 => run!(writer.record_journal_restore(&fence, Err(JournalStorageFault::Corrupt))),
                        19 => run!(writer.record_journal_restore(&fence, Ok(()))),
                        20 => run!(writer.record_storage(&fence, Err(JournalStorageFault::Unavailable))),
                        21 => run!(writer.record_storage(&fence, Ok(()))),
                        22 => run!(writer.request_saved(&id(99), true, Some(presentation().work), || id(501), None)),
                        23 => run!(writer.retry_saved(&id(99), || id(501), None)),
                        24 => run!(writer.request_progress(audio(), ExplicitProgress::MarkCompleted, Some(presentation()), 1000, || id(501))),
                        25 => run!(writer.request_progress(audio(), ExplicitProgress::Restart, None, 1000, || id(501))),
                        26 => run!(writer.resolve_progress_conflict(&audio().publication_id, true, || id(501), None)),
                        27 => run!(writer.retry_progress(&audio().publication_id, || id(501), None)),
                        28 => run!(writer.prompt_clear_history()),
                        29 => run!(writer.cancel_clear_history()),
                        30 => run!(writer.confirm_clear_history(|| id(501), None)),
                        31 => run!(writer.retry_history(|| id(501), None)),
                        32 => run!(writer.begin_resume(audio().publication_id, ResumeStatus::ExactAvailable, Some(presentation()))),
                        33 => run!(writer.begin_resume(audio().publication_id, ResumeStatus::ReleaseReplaced, None)),
                        34 => run!(writer.reset_resume()),
                        35 => run!(writer.ensure_saved_read(&id(99), Some(presentation().work))),
                        36 => run!(writer.ensure_progress_read(&audio().publication_id, Some(presentation()))),
                        37 => run!(writer.reread_saved(&fence, &id(99))),
                        38 => run!(writer.reread_progress(&fence, &audio().publication_id)),
                        39 => run!(writer.finish_saved_read(&fence, saved())),
                        40 => run!(writer.fail_saved_read(&fence, &id(99))),
                        41 => run!(writer.finish_progress_read(&fence, &audio().publication_id, Some(progress()), || id(501), None)),
                        42 => run!(writer.fail_progress_read(&fence, &audio().publication_id)),
                        43 => run!(writer.finish_resume(&fence, ticket, Ok(1000))),
                        44 => run!(writer.apply_receipt(&fence, &id(500), Receipt {operation_id:id(500),outcome:WriteOutcome::Accepted,resolution:None,state:MutationState::Saved {current:saved()}}, true, || id(501), None)),
                        45 => run!(writer.record_unknown_outcome(&fence, &id(500))),
                        46 => run!(writer.record_command_error(&fence, &id(500), CommandError::Unavailable, || id(101))),
                        47 => run!(writer.observe_playback(PlaybackObservation::SessionReset, Some(presentation()), 2000, || id(600), || id(501))),
                        48 => run!(writer.observe_playback(PlaybackObservation::PositionTick {source:source(),position_ms:2000}, Some(presentation()), 2000, || id(600), || id(501))),
                        49 => run!(writer.observe_playback(PlaybackObservation::SeekConfirmed {source:source(),position_ms:2000}, None, 2000, || id(600), || id(501))),
                        50 => run!(writer.observe_playback(PlaybackObservation::FlushCheckpoint {source:source(),position_ms:2000}, None, 2000, || id(600), || id(501))),
                        51 => run!(writer.observe_playback(PlaybackObservation::EngineStarted {identity:CallbackIdentity {generation:1,track:source().track()},source:source()}, Some(presentation()), 2000, || id(600), || id(501))),
                        _ => unreachable!(),
                    }
                    if action >= 47 {
                        let actual = state.with_untracked(Clone::clone);
                        let mut unconditional = actual.clone();
                        unconditional.seed_sync(|sync, library| { if let Some(library) = library { sync.sync_projection(library); } });
                        assert_eq!(actual.library(), unconditional.library(), "skipped projection must equal unconditional reconciliation, seed {seed} action {action}");
                    }
                }
            });
            }
        }
    }
}

#[test]
fn logout_failure_publishes_cleanup_change_then_its_duplicate_is_a_full_noop() {
    Owner::new().with(|| {
        let state = RwSignal::new(account());
        let writer = AccountWriter::new(state);
        let (generation, _) = checked(state, || writer.begin_logout()).unwrap();
        assert!(!checked(state, || writer
            .finish_logout(generation, Err(AccountFailure::Unavailable))));
        assert!(state.with_untracked(AccountViewState::can_retry_logout));
        let before = state.with_untracked(Clone::clone);
        assert_eq!(
            writer.finish_logout(generation, Err(AccountFailure::Unavailable)),
            Write::Done {
                value: false,
                published: Published::No
            }
        );
        assert_eq!(state.with_untracked(Clone::clone), before);
    });
}

#[test]
fn engine_bookkeeping_without_readers_waits_for_history_and_duplicates_do_not_replay() {
    Owner::new().with(|| {
        let state = RwSignal::new(account());
        let writer = AccountWriter::new(state);
        let observation = PlaybackObservation::EngineStarted {
            identity: CallbackIdentity {
                generation: 1,
                track: source().track(),
            },
            source: source(),
        };
        let before = state.with_untracked(Clone::clone);
        assert!(checked(state, || writer.observe_playback(
            observation,
            None,
            1000,
            || id(600),
            || panic!("history epoch not loaded")
        ))
        .unwrap()
        .value
        .is_none());
        let queued = state.with_untracked(Clone::clone);
        assert_ne!(before, queued);
        assert_eq!(reader_facts(&before), reader_facts(&queued));
        assert!(checked(state, || writer.observe_playback(
            observation,
            None,
            1000,
            || panic!("same engine session"),
            || panic!("duplicate must not mint")
        ))
        .unwrap()
        .value
        .is_none());
        assert_eq!(state.with_untracked(Clone::clone), queued);
        let request = checked(state, || writer.begin_library(Section::History, false)).unwrap();
        let (_, pending) = checked(state, || {
            writer.finish_page(
                &request,
                Ok(LibraryPage::History(PresentedHistoryPage {
                    items: Vec::new(),
                    next_cursor: None,
                    state: HistoryState {
                        revision: 0,
                        epoch: 0,
                    },
                })),
                || id(500),
            )
        })
        .unwrap();
        assert_eq!(pending.unwrap().operation_id, id(500));
        let settled = state.with_untracked(Clone::clone);
        assert_eq!(
            checked(state, || writer.finish_page(
                &request,
                Ok(page(Section::History)),
                || panic!("old page must not mint")
            ))
            .unwrap(),
            (LibraryCompletion::Ignored, None)
        );
        assert_eq!(state.with_untracked(Clone::clone), settled);
        assert!(checked(state, || writer.observe_playback(
            PlaybackObservation::SessionReset,
            None,
            1000,
            || id(601),
            || id(501)
        ))
        .unwrap()
        .value
        .is_none());
        let reset = state.with_untracked(Clone::clone);
        assert_ne!(reset, settled);
        assert_eq!(reader_facts(&reset), reader_facts(&settled));
    });
}

#[test]
fn progress_transient_tick_mint_seek_and_flush_keep_distinct_state_effect_and_publication() {
    for observation_kind in 0..3 {
        Owner::new().with(|| {
            let state = RwSignal::new(account());
            let writer = AccountWriter::new(state);
            let Admitted {
                fence,
                value: Some(first),
            } = checked(state, || {
                writer.observe_playback(
                    PlaybackObservation::PositionTick {
                        source: source(),
                        position_ms: 1000,
                    },
                    None,
                    1000,
                    || id(600),
                    || id(500),
                )
            })
            .unwrap()
            else {
                panic!()
            };
            let receipt = Receipt {
                operation_id: first.operation_id.clone(),
                outcome: WriteOutcome::Accepted,
                resolution: None,
                state: MutationState::Progress {
                    current: Some(progress()),
                },
            };
            checked(state, || {
                writer.apply_receipt(
                    &fence,
                    &first.operation_id,
                    receipt.clone(),
                    false,
                    || id(501),
                    None,
                )
            })
            .unwrap()
            .value
            .unwrap();
            let before_duplicate = state.with_untracked(Clone::clone);
            assert_eq!(
                checked(state, || writer.apply_receipt(
                    &fence,
                    &first.operation_id,
                    receipt,
                    false,
                    || panic!("duplicate cannot mint"),
                    None
                ))
                .unwrap()
                .value,
                Err(CommandError::Protocol)
            );
            assert_eq!(state.with_untracked(Clone::clone), before_duplicate);
            let before = state.with_untracked(Clone::clone);
            let tick = PlaybackObservation::PositionTick {
                source: source(),
                position_ms: 2000,
            };
            assert!(checked(state, || writer.observe_playback(
                tick,
                None,
                2000,
                || id(600),
                || panic!("throttled tick must not mint")
            ))
            .unwrap()
            .value
            .is_none());
            let after = state.with_untracked(Clone::clone);
            assert_ne!(before, after, "the transient intent really changed");
            assert_eq!(reader_facts(&before), reader_facts(&after));
            assert_eq!(
                before.sync().unwrap().to_journal(),
                after.sync().unwrap().to_journal()
            );
            assert_eq!(
                after
                    .sync()
                    .unwrap()
                    .progress_for(&audio().publication_id)
                    .unwrap()
                    .unsent
                    .as_ref()
                    .unwrap()
                    .position_ms,
                2000
            );
            let observation = match observation_kind {
                0 => PlaybackObservation::PositionTick {
                    source: source(),
                    position_ms: 3000,
                },
                1 => PlaybackObservation::SeekConfirmed {
                    source: source(),
                    position_ms: 3000,
                },
                _ => PlaybackObservation::FlushCheckpoint {
                    source: source(),
                    position_ms: 3000,
                },
            };
            let result = checked(state, || {
                writer.observe_playback(observation, None, 31000, || id(600), || id(501))
            })
            .unwrap()
            .value
            .unwrap();
            assert_eq!(result.operation_id, id(501));
            assert!(matches!(
                state.with_untracked(|s| s
                    .sync()
                    .unwrap()
                    .progress_for(&audio().publication_id)
                    .unwrap()
                    .command
                    .clone()),
                ProgressCommandPhase::Pending { .. }
            ));
        });
    }
}

#[test]
fn effects_only_resume_runs_once_after_release_and_dispose_cannot_restart_or_erase_operations() {
    let owner = Owner::new();
    owner.with(|| {
        let state = RwSignal::new(account());
        let writer = AccountWriter::new(state);
        let calls = RefCell::new(Vec::new());
        let mut tickets = Vec::new();
        for _ in 0..2 {
            let ticket = checked(state, || {
                writer.begin_resume(audio().publication_id, ResumeStatus::ExactAvailable, None)
            })
            .unwrap()
            .value
            .unwrap();
            // Shell interprets each admitted effect once, regardless of publication. This
            // reentry also fails if the production writer returns with its guard held.
            writer.record_storage(
                &state.with_untracked(AccountViewState::fence).unwrap(),
                Ok(()),
            );
            calls.borrow_mut().push(ticket);
            tickets.push(ticket);
        }
        assert_ne!(tickets[0], tickets[1]);
        assert_eq!(calls.borrow().len(), 2);
        let fence = state.with_untracked(AccountViewState::fence).unwrap();
        let before = state.with_untracked(Clone::clone);
        assert!(
            !checked(state, || writer.finish_resume(&fence, tickets[0], Ok(1000)))
                .unwrap()
                .value
        );
        assert_eq!(state.with_untracked(Clone::clone), before);
        assert!(
            checked(state, || writer.finish_resume(&fence, tickets[1], Ok(1000)))
                .unwrap()
                .value
        );
        let pending = checked(state, || {
            writer.request_saved(&id(99), true, None, || id(500), None)
        })
        .unwrap()
        .value
        .unwrap();
        let durable = RefCell::new(None);
        assert_eq!(
            writer.dispatch_after_persist(
                fence.clone(),
                pending.clone(),
                |journal| {
                    *durable.borrow_mut() = Some(journal.clone());
                    owner.cleanup();
                    Ok(())
                },
                |_, _| panic!("disposed during storage must not construct network work")
            ),
            None::<()>
        );
        assert_eq!(durable.borrow().as_ref().unwrap().pending[0], pending);
        assert_eq!(writer.reset_resume(), Write::Disposed);
        assert_eq!(
            writer.finish_logout(before.generation(), Ok(())),
            Write::Disposed
        );
        assert_eq!(
            writer.observe_playback(
                PlaybackObservation::PositionTick {
                    source: source(),
                    position_ms: 3000
                },
                None,
                31000,
                || panic!("disposed mint"),
                || panic!("disposed mint")
            ),
            Write::Disposed
        );
        assert_eq!(
            writer.dispatch_after_persist(
                fence,
                pending.clone(),
                |_| panic!("disposed persist"),
                |_, _| panic!("disposed send")
            ),
            None::<()>
        );
        let recovered = durable
            .into_inner()
            .unwrap()
            .restore_for_actor("a")
            .unwrap();
        assert_eq!(recovered.pending[0].operation_id, pending.operation_id);
        assert_eq!(recovered.pending[0].payload, pending.payload);
    });
}

#[test]
fn read_error_retry_and_duplicate_begin_do_not_refetch_from_account_notifications() {
    Owner::new().with(|| {
        let state = RwSignal::new(account());
        let writer = AccountWriter::new(state);
        let admission = Memo::new(move |_| state.with(AccountViewState::fence));
        let calls = Arc::new(AtomicUsize::new(0));
        let port = calls.clone();
        let scheduled = Memo::new(move |_| {
            if admission.get().is_some()
                && writer
                    .ensure_saved_read(&id(99), None)
                    .into_value()
                    .unwrap()
                    .unwrap()
                    .value
            {
                port.fetch_add(1, Ordering::SeqCst);
            }
        });
        scheduled.get();
        let fence = state.with_untracked(AccountViewState::fence).unwrap();
        for _ in 0..3 {
            assert!(
                !checked(state, || writer.ensure_saved_read(&id(99), None))
                    .unwrap()
                    .value
            );
            scheduled.get();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        checked(state, || writer.fail_saved_read(&fence, &id(99))).unwrap();
        scheduled.get();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "failure is not a new admission"
        );
        assert!(
            checked(state, || writer.reread_saved(&fence, &id(99)))
                .unwrap()
                .value
        );
        calls.fetch_add(1, Ordering::SeqCst); // explicit user retry's recording port
        checked(state, || writer.finish_saved_read(&fence, saved())).unwrap();
        scheduled.get();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(state.with_untracked(|s| s
            .sync()
            .unwrap()
            .saved_work(&id(99))
            .unwrap()
            .is_saved()));
    });
}

#[test]
fn playback_reconciles_confirmed_rows_after_a_paging_restart_or_failure() {
    for seed in 22..=24 {
        Owner::new().with(|| {
            let initial = fixture(seed);
            let mut expected = initial.clone();
            expected.seed_sync(|sync, library| {
                sync.sync_projection(library.unwrap());
            });
            assert_ne!(initial.library(), expected.library());
            let state = RwSignal::new(initial);
            checked(state, || {
                AccountWriter::new(state).observe_playback(
                    PlaybackObservation::SessionReset,
                    None,
                    1000,
                    || panic!("no session mint"),
                    || panic!("no operation mint"),
                )
            })
            .unwrap();
            assert_eq!(
                state.with_untracked(|s| s.library().cloned()),
                expected.library().cloned()
            );
        });
    }
}
