use super::super::tests::{populate, signed_in};
use super::*;
use crate::application::library_sync::{CommandError, LibrarySyncState};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

const WORK: &str = "00000000-0000-4000-8000-000000000001";
fn state() -> AccountViewState {
    let mut s = signed_in();
    populate(&mut s);
    s
}
fn edit_projection(s: &mut AccountViewState, edit: impl FnOnce(&mut LibraryProjection)) {
    s.with_sync_mut(|sync, projection| {
        // Isolate each presentation fact from reconciliation. Transition behavior has its
        // own application tests; these fixtures exercise the production row selectors.
        *sync = LibrarySyncState::new("listener-a", uuid::Uuid::from_u128(11).to_string()).unwrap();
        edit(projection.unwrap());
    })
    .unwrap();
}
fn audio_text(row: &AudioRowUi) -> String {
    format!(
        "{} {:?} {:?} {:?}",
        position_text(row, Locale::English),
        row.resume,
        row.presentation,
        resume_feedback(row, Locale::English)
    )
}
#[test]
fn every_audio_fact_updates_under_the_same_section_work_identity() {
    Owner::new().with(|| {
        for section in [Section::Continue, Section::History] {
            let signal = RwSignal::new(state());
            let ids = Memo::new(move |_| signal.with(|s| membership(s, section)));
            let row = Memo::new(move |_| signal.with(|s| audio_ui(s, section, WORK)));
            let before = row.get().unwrap();
            let initial_ids = ids.get();
            for field in 0..8 {
                signal.update(|s| {
                    edit_projection(s, |p| {
                        let (audio, pos, resume, pres) = match section {
                            Section::Continue => {
                                let i = &mut p.continuing[0];
                                (
                                    &mut i.progress.audio,
                                    &mut i.progress.position_ms,
                                    &mut i.resume,
                                    &mut i.presentation,
                                )
                            }
                            Section::History => {
                                let i = &mut p.history[0];
                                (
                                    &mut i.audio,
                                    &mut i.progress.as_mut().unwrap().position_ms,
                                    &mut i.resume,
                                    &mut i.presentation,
                                )
                            }
                            _ => unreachable!(),
                        };
                        match field {
                            0 => *pos = 25_000,
                            1 => audio.duration_ms = 90_000,
                            2 => *resume = ResumeStatus::ReleaseReplaced,
                            3 => pres.as_mut().unwrap().unit_title = "Tên tập đã đổi".into(),
                            4 => pres.as_mut().unwrap().work.title = "Tên truyện đã đổi".into(),
                            5 => pres.as_mut().unwrap().work.language = "en".into(),
                            6 => audio.publication_id = uuid::Uuid::from_u128(900).to_string(),
                            7 => *pres = None,
                            _ => unreachable!(),
                        }
                    })
                });
                let next = row.get().unwrap();
                assert_ne!(next, before, "{section:?} field {field}");
                assert_eq!(ids.get(), initial_ids);
                match field {
                    0 => assert!(audio_text(&next).contains("0:25")),
                    1 => assert!(audio_text(&next).contains("1:30")),
                    2 => assert_eq!(next.resume, ResumeStatus::ReleaseReplaced),
                    3 => assert_eq!(
                        next.presentation.as_ref().unwrap().unit_title,
                        "Tên tập đã đổi"
                    ),
                    4 => assert_eq!(
                        next.presentation.as_ref().unwrap().work.title,
                        "Tên truyện đã đổi"
                    ),
                    5 => assert_eq!(next.presentation.as_ref().unwrap().work.language, "en"),
                    6 => assert_eq!(
                        next.audio.publication_id,
                        uuid::Uuid::from_u128(900).to_string()
                    ),
                    7 => assert_eq!(next.presentation, None),
                    _ => unreachable!(),
                }
            }
        }
    });
}

#[test]
fn command_and_resume_feedback_changes_are_projected_without_changing_membership() {
    Owner::new().with(|| {
        let signal = RwSignal::new(state());
        let row = Memo::new(move |_| signal.with(|s| audio_ui(s, Section::Continue, WORK)));
        let saved = Memo::new(move |_| signal.with(|s| saved_ui(s, WORK)));
        let audio = row.get().unwrap().audio;
        let ids = signal.with(|s| membership(s, Section::Continue));
        for feedback in [
            ResumeAttemptState::Restored {
                publication_id: audio.publication_id.clone(),
                position_ms: 12_000,
            },
            ResumeAttemptState::Refused {
                publication_id: audio.publication_id.clone(),
                reason: ResumeRefusal::ReleaseReplaced,
            },
            ResumeAttemptState::Refused {
                publication_id: audio.publication_id.clone(),
                reason: ResumeRefusal::AccessUnavailable,
            },
            ResumeAttemptState::Refused {
                publication_id: audio.publication_id.clone(),
                reason: ResumeRefusal::Protocol,
            },
        ] {
            signal.update(|s| {
                s.with_sync_mut(|sync, _| sync.set_resume_attempt(feedback.clone()));
            });
            assert_eq!(row.get().unwrap().feedback, feedback);
            assert!(resume_feedback(&row.get().unwrap(), Locale::English).is_some());
        }
        let operation = uuid::Uuid::from_u128(80).to_string();
        signal.update(|s| {
            s.with_sync_mut(|sync, _| {
                assert!(sync
                    .request_saved(WORK, false, operation.clone(), None)
                    .is_some());
            });
        });
        assert!(matches!(
            saved.get().unwrap().command,
            SavedCommandPhase::Pending { .. }
        ));
        signal.update(|s| {
            s.with_sync_mut(|sync, _| {
                assert!(sync.record_unknown_outcome(&operation));
            });
        });
        assert_eq!(
            saved_status(&saved.get().unwrap().command),
            Some(Key::LiveSaveUnknown)
        );
        signal.update(|s| {
            s.with_sync_mut(|sync, _| {
                assert!(
                    sync.record_command_error(&operation, CommandError::Protocol, || {
                        uuid::Uuid::from_u128(81).to_string()
                    })
                );
            });
        });
        assert_eq!(
            saved_status(&saved.get().unwrap().command),
            Some(Key::LiveSaveError)
        );
        let operation = uuid::Uuid::from_u128(90).to_string();
        signal.update(|s| {
            s.with_sync_mut(|sync, _| {
                assert!(sync
                    .record_progress_intent(
                        audio.clone(),
                        30_000,
                        cantos_contracts::listener_library::ProgressKind::ExplicitSeek,
                        true,
                        10_000,
                        || operation.clone()
                    )
                    .is_some());
            });
        });
        assert!(matches!(
            row.get().unwrap().command,
            ProgressCommandPhase::Pending { .. }
        ));
        signal.update(|s| {
            s.with_sync_mut(|sync, _| {
                assert!(sync.record_unknown_outcome(&operation));
            });
        });
        assert!(matches!(
            row.get().unwrap().command,
            ProgressCommandPhase::UnknownOutcome { .. }
        ));
        signal.update(|s| {
            s.with_sync_mut(|sync, _| {
                assert!(
                    sync.record_command_error(&operation, CommandError::Protocol, || {
                        uuid::Uuid::from_u128(91).to_string()
                    })
                );
            });
        });
        assert!(matches!(
            row.get().unwrap().command,
            ProgressCommandPhase::Failed { .. }
        ));
        assert_eq!(signal.with(|s| membership(s, Section::Continue)), ids);
    });
}

#[test]
fn content_notifications_run_selectors_but_not_equal_membership_consumers() {
    Owner::new().with(|| {
            let state = RwSignal::new(state());
            let selects = Arc::new(AtomicUsize::new(0));
            let count = selects.clone();
            let ids = Memo::new(move |_| { count.fetch_add(1, Ordering::SeqCst); state.with(|s| membership(s, Section::Continue)) });
            let reconciles = Arc::new(AtomicUsize::new(0));
            let count = reconciles.clone();
            let consumer = Memo::new(move |_| { let ids = ids.get(); count.fetch_add(1, Ordering::SeqCst); ids });
            assert_eq!(consumer.get(), vec![WORK.to_owned()]);
            for pos in [20_000, 21_000, 22_000] {
                state.update(|s| edit_projection(s, |p| p.continuing[0].progress.position_ms = pos));
                assert_eq!(consumer.get(), vec![WORK.to_owned()]);
            }
            assert_eq!(selects.load(Ordering::SeqCst), 4);
            assert_eq!(reconciles.load(Ordering::SeqCst), 1);
            println!("#359 host: membership selector computations=4 (3 after mount); membership consumer reconciliations=1 (0 after mount); row/component construction and DOM mutations measured separately in browser");
        });
}

#[test]
fn membership_tracks_insert_remove_reorder_and_removed_lookup_is_none_in_each_section() {
    Owner::new().with(|| {
        for section in Section::ALL {
            let s = RwSignal::new(state());
            let ids = Memo::new(move |_| s.with(|s| membership(s, section)));
            let a = uuid::Uuid::from_u128(70).to_string();
            s.update(|s| {
                edit_projection(s, |p| match section {
                    Section::Continue => {
                        let mut r = p.continuing[0].clone();
                        r.progress.audio.work_id = a.clone();
                        p.continuing.insert(0, r);
                    }
                    Section::History => {
                        let mut r = p.history[0].clone();
                        r.audio.work_id = a.clone();
                        p.history.insert(0, r);
                    }
                    Section::Saved => {
                        let mut r = p.saved[0].clone();
                        r.saved.work_id = a.clone();
                        p.saved.insert(0, r);
                    }
                })
            });
            assert_eq!(ids.get(), vec![a.clone(), WORK.to_owned()]);
            s.update(|s| {
                edit_projection(s, |p| match section {
                    Section::Continue => p.continuing.reverse(),
                    Section::Saved => p.saved.reverse(),
                    Section::History => p.history.reverse(),
                })
            });
            assert_eq!(ids.get(), vec![WORK.to_owned(), a.clone()]);
            s.update(|s| {
                edit_projection(s, |p| match section {
                    Section::Continue => p.continuing.clear(),
                    Section::Saved => p.saved.clear(),
                    Section::History => p.history.clear(),
                })
            });
            // Force the row lookup before asking the membership memo to reconcile removal.
            assert!(s.with(|s| audio_ui(s, section, WORK)).is_none());
            assert!(s.with(|s| saved_ui(s, WORK)).is_none() || section != Section::Saved);
            assert_eq!(ids.get(), Vec::<String>::new());
        }
    });
}

#[test]
fn retained_resume_handler_reads_current_release_position_and_rejects_absence_and_logout() {
    use std::sync::Mutex;
    Owner::new().with(|| {
        let s = RwSignal::new(state());
        let row = Memo::new(move |_| s.with(|s| audio_ui(s, Section::Continue, WORK)));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let record = calls.clone();
        let act = LibraryActions {
            on_resume: Callback::new(move |target| record.lock().unwrap().push(target)),
            on_unsave: Callback::new(|_| ()),
            on_retry_saved: Callback::new(|_| ()),
            on_prompt_clear_history: Callback::new(|_| ()),
            on_confirm_clear_history: Callback::new(|_| ()),
            on_cancel_clear_history: Callback::new(|_| ()),
            on_retry_history: Callback::new(|_| ()),
            on_resolve_progress: Callback::new(|_| ()),
            on_retry_progress: Callback::new(|_| ()),
        };
        let _ = row.get();
        s.update(|s| {
            edit_projection(s, |p| {
                p.continuing[0].progress.audio.publication_id =
                    uuid::Uuid::from_u128(92).to_string();
                p.continuing[0].progress.position_ms = 32_000;
            })
        });
        resume_current(row, act);
        assert_eq!(
            calls.lock().unwrap()[0].0.publication_id,
            uuid::Uuid::from_u128(92).to_string()
        );
        assert_eq!(calls.lock().unwrap()[0].1, 32_000);
        s.update(|s| {
            edit_projection(s, |p| {
                p.continuing[0].resume = ResumeStatus::AccessUnavailable
            })
        });
        resume_current(row, act);
        s.update(|s| {
            s.begin_logout().unwrap();
        });
        resume_current(row, act);
        assert_eq!(calls.lock().unwrap().len(), 1);
        assert_eq!(row.get(), None);
        for section in Section::ALL {
            assert!(s.with(|s| membership(s, section)).is_empty());
        }
    });
}
#[test]
fn saved_presentation_and_history_completion_are_reactive_under_stable_ids() {
    Owner::new().with(|| {
        let s = RwSignal::new(state());
        let saved = Memo::new(move |_| s.with(|s| saved_ui(s, WORK)));
        let history = Memo::new(move |_| s.with(|s| audio_ui(s, Section::History, WORK)));
        for field in 0..4 {
            s.update(|s| {
                edit_projection(s, |p| match field {
                    0 => p.saved[0].presentation.as_mut().unwrap().title = "Bản tên mới".into(),
                    1 => p.saved[0].presentation.as_mut().unwrap().language = "en".into(),
                    2 => {
                        p.saved[0].presentation.as_mut().unwrap().publication_id =
                            uuid::Uuid::from_u128(800).to_string()
                    }
                    3 => p.history[0].progress.as_mut().unwrap().completion = Completion::Completed,
                    _ => unreachable!(),
                })
            });
            match field {
                0 => assert_eq!(
                    saved.get().unwrap().presentation.unwrap().title,
                    "Bản tên mới"
                ),
                1 => assert_eq!(saved.get().unwrap().presentation.unwrap().language, "en"),
                2 => assert_eq!(
                    saved.get().unwrap().presentation.unwrap().publication_id,
                    uuid::Uuid::from_u128(800).to_string()
                ),
                3 => assert!(position_text(&history.get().unwrap(), Locale::English)
                    .contains(messages::text(Key::LiveResumeCompleted, Locale::English))),
                _ => unreachable!(),
            }
            assert_eq!(
                s.with(|s| membership(s, Section::Saved)),
                vec![WORK.to_owned()]
            );
        }
    });
}

#[test]
fn conflict_projections_keep_both_positions_and_saved_feedback_then_return_to_idle() {
    use cantos_contracts::listener_library::{
        MutationState, ProgressKind, Receipt, Resolution, WriteOutcome,
    };
    Owner::new().with(|| {
        for resolution in [Resolution::ListenerChooses, Resolution::KeepServer] {
            let s = RwSignal::new(state());
            let row = Memo::new(move |_| s.with(|s| audio_ui(s, Section::Continue, WORK)));
            let audio = row.get().unwrap().audio;
            let operation = uuid::Uuid::from_u128(101).to_string();
            s.update(|s| {
                s.with_sync_mut(|sync, _| {
                    sync.record_progress_intent(
                        audio.clone(),
                        35_000,
                        ProgressKind::ExplicitSeek,
                        true,
                        5000,
                        || operation.clone(),
                    )
                    .unwrap();
                    let mut current = sync
                        .progress_for(&audio.publication_id)
                        .unwrap()
                        .record
                        .clone()
                        .unwrap();
                    current.position_ms = 20_000;
                    current.revision += 1;
                    sync.apply_receipt(
                        &operation,
                        Receipt {
                            operation_id: operation.clone(),
                            outcome: WriteOutcome::RevisionConflict,
                            resolution: Some(resolution),
                            state: MutationState::Progress {
                                current: Some(current),
                            },
                        },
                        false,
                        || uuid::Uuid::from_u128(102).to_string(),
                        None,
                    )
                    .unwrap();
                });
            });
            let command = row.get().unwrap().command;
            match (&command, resolution) {
                (
                    ProgressCommandPhase::Conflict(ProgressConflict::ListenerChooses {
                        local_position_ms,
                        server,
                        ..
                    }),
                    Resolution::ListenerChooses,
                )
                | (
                    ProgressCommandPhase::Conflict(ProgressConflict::KeepServer {
                        local_position_ms,
                        server,
                    }),
                    Resolution::KeepServer,
                ) => {
                    assert_eq!(*local_position_ms, 35_000);
                    assert_eq!(server.position_ms, 20_000);
                }
                _ => panic!("{command:?}"),
            }
            let html = progress_feedback(
                Section::Continue,
                audio.publication_id,
                command,
                None,
                Locale::English,
            )
            .to_html();
            assert!(html.contains("0:20"));
        }
        let s = RwSignal::new(state());
        let saved = Memo::new(move |_| s.with(|s| saved_ui(s, WORK)));
        for (outcome, expected) in [
            (WriteOutcome::RevisionConflict, Some(Key::LiveSaveConflict)),
            (WriteOutcome::Accepted, None),
        ] {
            let operation =
                uuid::Uuid::from_u128(if expected.is_some() { 103 } else { 104 }).to_string();
            s.update(|s| {
                s.with_sync_mut(|sync, _| {
                    sync.request_saved(WORK, false, operation.clone(), None)
                        .unwrap();
                    let mut current = sync.saved_work(WORK).unwrap().record.clone().unwrap();
                    current.revision += 1;
                    sync.apply_receipt(
                        &operation,
                        Receipt {
                            operation_id: operation.clone(),
                            outcome,
                            resolution: expected.map(|_| Resolution::KeepServer),
                            state: MutationState::Saved { current },
                        },
                        false,
                        || uuid::Uuid::from_u128(105).to_string(),
                        None,
                    )
                    .unwrap();
                });
            });
            assert_eq!(saved_status(&saved.get().unwrap().command), expected);
        }
    });
}

#[test]
fn account_switch_clears_all_old_selectors_and_ignores_the_old_page_completion() {
    use crate::application::{
        account::AccountScope,
        account_view::{LibraryCompletion, SessionCompletion},
    };
    Owner::new().with(|| {
        let s = RwSignal::new(state());
        let row = Memo::new(move |_| s.with(|s| audio_ui(s, Section::Continue, WORK)));
        let saved = Memo::new(move |_| s.with(|s| saved_ui(s, WORK)));
        assert!(row.get().is_some() && saved.get().is_some());
        let stale = s
            .try_update(|s| s.begin_library(Section::Saved, true))
            .unwrap()
            .unwrap();
        let issued = s
            .try_update(AccountViewState::begin_refresh)
            .unwrap()
            .unwrap();
        assert_eq!(row.get(), None);
        assert_eq!(saved.get(), None);
        assert_eq!(s.with(super::super::history_ui), None);
        s.update(|s| {
            assert!(matches!(
                s.finish_session(
                    issued,
                    Ok(AccountScope::from_session(
                        "listener-b".into(),
                        uuid::Uuid::from_u128(300).to_string()
                    )
                    .unwrap()),
                    || uuid::Uuid::from_u128(100).to_string()
                ),
                Ok(SessionCompletion::Authenticated(_))
            ));
        });
        assert_eq!(row.get(), None);
        assert_eq!(saved.get(), None);
        assert_eq!(
            s.try_update(|s| s.finish_page(&stale, Err(AccountFailure::Unavailable)))
                .unwrap(),
            Ok(LibraryCompletion::Ignored)
        );
        for section in Section::ALL {
            assert!(s.with(|s| membership(s, section)).is_empty());
        }
    });
}
