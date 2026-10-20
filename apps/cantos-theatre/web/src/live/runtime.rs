//! Leptos/browser shell for the pure listener account and library sync owners.
use crate::{
    application::{
        account::{AccountGeneration, RequestFence},
        account_view::{
            AccountFailure, AccountViewState, CleanupIntent, LibraryCompletion, LibraryRequest,
            SessionCompletion, SessionPhase,
        },
        library::Section,
        library_sync::{
            audio_identity_from_source, decode_journal, generate_uuid_string,
            record_started_in_projection, release_matches_audio_identity, AccountSyncJournal,
            LibrarySyncState, PendingMutation, RecoveryMode, RereadTarget, ResumeAttemptState,
            ResumeRefusal,
        },
        player_session::{PlaybackObservation, PlayerSession},
        public_listen::{ListenLabel, NowPlaying},
        public_work::CatalogReadError,
    },
    core::{
        playback::{ResumeSnapshot, Source, ValidatedResumeSnapshot},
        public_ids::PublicWorkId,
        public_track::{PinnedRelease, ReleaseRequest},
    },
};
use cantos_contracts::listener_library::{
    AudioIdentity, AudioPresentation, ProgressKind, Receipt, ResumeStatus, WorkPresentation,
};
use leptos::prelude::*;
use web_sys::wasm_bindgen::{closure::Closure, JsCast};

const SYNC_STORAGE_PREFIX: &str = "cantos.theatre.sync.v1.";
const MAX_JOURNAL_BYTES: usize = 65_536;

fn storage_key(actor: &str) -> String {
    format!("{SYNC_STORAGE_PREFIX}{actor}")
}

fn current_speed_permille(player: &PlayerSession) -> u16 {
    player
        .state
        .get_untracked()
        .session()
        .map_or(1000, |s| s.speed_permille())
}

fn build_resume_snapshot(
    source: Source,
    position_ms: u32,
    speed_permille: u16,
) -> Option<ValidatedResumeSnapshot> {
    ValidatedResumeSnapshot::try_from(ResumeSnapshot {
        source,
        position_ms,
        speed_permille,
    })
    .ok()
}

/// Monotonic relative timestamp in milliseconds for checkpoint throttling.
fn now_ms() -> i64 {
    web_sys::Event::new("cantos-clock")
        .ok()
        .map(|e| e.time_stamp() as i64)
        .filter(|t| *t >= 0)
        .unwrap_or(0)
}

fn load_journal_from_storage(
    actor: &str,
) -> Result<Option<AccountSyncJournal>, crate::application::account_view::JournalStorageFault> {
    use crate::application::account_view::JournalStorageFault;
    let storage = web_sys::window()
        .ok_or(JournalStorageFault::Unavailable)?
        .local_storage()
        .map_err(|_| JournalStorageFault::Unavailable)?
        .ok_or(JournalStorageFault::Unavailable)?;
    let Some(raw) = storage
        .get_item(&storage_key(actor))
        .map_err(|_| JournalStorageFault::Unavailable)?
    else {
        return Ok(None);
    };
    decode_journal(&raw, actor).map(Some)
}

/// Persist before any mutation send. A corrupt/unread journal must never be overwritten with
/// an empty replacement, losing operation IDs that could already have reached the API.
pub fn persist_sync_journal(state: RwSignal<AccountViewState>) -> bool {
    use crate::application::account_view::JournalStorageFault;
    let Some(Some((fence, journal))) = state.try_with_untracked(|s| {
        if !s.can_persist_journal() {
            return None;
        }
        Some((s.fence()?, s.sync()?.to_journal()))
    }) else {
        return false;
    };
    let result = (|| {
        let json = journal
            .to_json_string()
            .filter(|json| json.len() <= MAX_JOURNAL_BYTES)
            .ok_or(JournalStorageFault::Unavailable)?;
        let storage = web_sys::window()
            .ok_or(JournalStorageFault::Unavailable)?
            .local_storage()
            .map_err(|_| JournalStorageFault::Unavailable)?
            .ok_or(JournalStorageFault::Unavailable)?;
        storage
            .set_item(&storage_key(&journal.actor_id), &json)
            .map_err(|_| JournalStorageFault::Unavailable)
    })();
    let _ = state.try_update(|s| s.record_storage(&fence, result));
    result.is_ok()
}

fn presentation_from_now_playing(
    source: Source,
    now_playing: RwSignal<Option<NowPlaying>>,
) -> Option<AudioPresentation> {
    let facts = now_playing
        .try_get_untracked()
        .flatten()
        .filter(|facts| facts.describes(source))?;
    let track = facts.track();
    Some(AudioPresentation {
        work: WorkPresentation {
            work_id: track.work().to_string(),
            publication_id: track.publication().to_string(),
            title: facts.label().work_title.clone(),
            kind: cantos_contracts::theatre::WorkKind::Story,
            language: facts.label().language.clone(),
        },
        unit_id: track.unit().to_string(),
        unit_title: facts.label().unit_title.clone(),
    })
}

async fn cleanup(
    state: RwSignal<AccountViewState>,
    issued: AccountGeneration,
    intent: CleanupIntent,
    session_change_budget: u8,
) {
    let result = super::account::delete_session(&intent).await;
    let session_changed = result == Err(AccountFailure::SessionChanged);
    let mut current_result = false;
    let _ = state.try_update(|current| {
        current_result =
            current.generation() == issued && current.phase() == SessionPhase::LogoutPending;
        current.finish_logout(issued, result);
    });
    if current_result && session_changed && session_change_budget > 0 {
        refresh_with_budget(state, session_change_budget - 1);
    }
}

pub fn refresh(state: RwSignal<AccountViewState>) {
    refresh_with_budget(state, 1);
}

fn refresh_with_budget(state: RwSignal<AccountViewState>, session_change_budget: u8) {
    let current = state.get_untracked();
    if matches!(
        current.phase(),
        SessionPhase::Checking | SessionPhase::LogoutPending
    ) {
        return;
    }
    let mut current = current;
    let Ok(issued) = current.begin_refresh() else {
        return;
    };
    state.set(current);
    leptos::task::spawn_local(async move {
        let result = super::account::session().await;
        finish_lookup(state, issued, result, session_change_budget).await;
    });
}

async fn finish_lookup(
    state: RwSignal<AccountViewState>,
    issued: AccountGeneration,
    result: Result<crate::application::account::AccountScope, AccountFailure>,
    session_change_budget: u8,
) {
    let mut completion = None;
    let mut cleanup_intent = CleanupIntent::StaleOnly;
    let mut actor = None;
    if state
        .try_update(|current| {
            completion = Some(current.finish_session(issued, result));
            cleanup_intent = current.cleanup_intent().clone();
            actor = current.actor().map(str::to_owned);
        })
        .is_none()
    {
        return;
    }
    match completion {
        Some(Ok(SessionCompletion::Authenticated(fence))) => {
            if let Some(actor_id) = actor {
                match load_journal_from_storage(&actor_id) {
                    Ok(Some(journal)) => {
                        let _ = state.try_update(|current| {
                            current.restore_sync_journal(&fence, journal);
                        });
                    }
                    Ok(None) => (),
                    Err(fault) => {
                        let _ = state.try_update(|current| {
                            current.record_journal_restore(&fence, Err(fault))
                        });
                    }
                }
            }
            persist_sync_journal(state);
            let recoverable = state
                .try_with_untracked(|s| {
                    if s.accepts(&fence) {
                        s.sync()
                            .map(LibrarySyncState::pending_mutations)
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                })
                .unwrap_or_default();
            for section in Section::ALL {
                load_library(state, section, false, session_change_budget, false);
            }
            for pending in recoverable {
                let fence = fence.clone();
                leptos::task::spawn_local(execute_mutation(
                    state,
                    fence,
                    pending,
                    session_change_budget,
                ));
            }
        }
        Some(Ok(SessionCompletion::Cleanup(issued))) => {
            cleanup(state, issued, cleanup_intent, session_change_budget).await
        }
        Some(Ok(SessionCompletion::Ignored)) | Some(Err(_)) | None => (),
    }
}

pub fn load_library(
    state: RwSignal<AccountViewState>,
    section: Section,
    restart: bool,
    session_change_budget: u8,
    focus: bool,
) {
    let Some(Some(request)) = state.try_update(|s| s.begin_library(section, restart)) else {
        return;
    };
    leptos::task::spawn_local(finish_page(state, request, session_change_budget, focus));
}

async fn finish_page(
    state: RwSignal<AccountViewState>,
    request: LibraryRequest,
    session_change_budget: u8,
    focus: bool,
) {
    let result = super::library::load(&request).await;
    let section = request.page.section;
    let changed = result == Err(AccountFailure::SessionChanged);
    let Some(completion) =
        state.try_update(|s| s.finish_page_with_followup(&request, result, generate_uuid_string))
    else {
        return;
    };
    let completion = completion.map(|(completion, pending)| {
        if let Some(pending) = pending {
            persist_sync_journal(state);
            leptos::task::spawn_local(execute_mutation(
                state,
                request.fence().clone(),
                pending,
                session_change_budget,
            ));
        }
        completion
    });
    match completion {
        Ok(LibraryCompletion::Cleanup(issued)) => {
            let intent = state.with_untracked(|s| s.cleanup_intent().clone());
            cleanup(state, issued, intent, session_change_budget).await;
        }
        Ok(LibraryCompletion::Applied) if changed && session_change_budget > 0 => {
            refresh_with_budget(state, session_change_budget - 1)
        }
        Ok(LibraryCompletion::Applied) if focus => {
            crate::platform::focus_by_id(crate::features::library::live::status_id(section))
        }
        Ok(LibraryCompletion::Applied | LibraryCompletion::Ignored) | Err(_) => (),
    }
}

async fn handle_command_auth_failure(
    state: RwSignal<AccountViewState>,
    fence: &RequestFence,
    failure: AccountFailure,
    session_change_budget: u8,
) {
    let changed = failure == AccountFailure::SessionChanged;
    let Some(completion) = state.try_update(|s| s.fail_fenced_command(fence, failure)) else {
        return;
    };
    match completion {
        Ok(LibraryCompletion::Cleanup(issued)) => {
            let intent = state.with_untracked(|s| s.cleanup_intent().clone());
            cleanup(state, issued, intent, session_change_budget).await;
        }
        Ok(LibraryCompletion::Applied) if changed && session_change_budget > 0 => {
            refresh_with_budget(state, session_change_budget - 1);
        }
        Ok(LibraryCompletion::Applied | LibraryCompletion::Ignored) | Err(_) => (),
    }
}

pub fn ensure_saved_loaded(
    state: RwSignal<AccountViewState>,
    work_id: String,
    presentation: Option<WorkPresentation>,
) {
    let work_for_closure = work_id.clone();
    let Some(Some((fence, true))) = state.try_update(|s| {
        s.with_sync_mut(|sync, _| {
            if let Some(w) = presentation {
                sync.remember_work_presentation(w);
            }
            if sync
                .saved_work(&work_for_closure)
                .is_some_and(|e| e.record.is_some() || e.loading)
            {
                false
            } else {
                sync.begin_saved_read(&work_for_closure)
            }
        })
    }) else {
        return;
    };
    leptos::task::spawn_local(run_saved_point_read(state, fence, work_id, 1));
}

async fn run_saved_point_read(
    state: RwSignal<AccountViewState>,
    fence: RequestFence,
    work_id: String,
    session_change_budget: u8,
) {
    let result = super::library::load_saved_point(fence.session(), &work_id).await;
    match result {
        Ok(record) => {
            let _ = state.try_update(|s| {
                s.with_fenced_sync_mut(&fence, |sync, _| {
                    sync.observe_saved_record(record);
                })
            });
        }
        Err(error) => {
            if let Some(failure) = error.to_account_failure() {
                handle_command_auth_failure(state, &fence, failure, session_change_budget).await;
                return;
            }
            let _ = state.try_update(|s| {
                s.with_fenced_sync_mut(&fence, |sync, _| {
                    sync.fail_saved_read(&work_id);
                })
            });
        }
    }
}

pub fn ensure_progress_loaded(
    state: RwSignal<AccountViewState>,
    publication_id: String,
    presentation: Option<AudioPresentation>,
) {
    let pub_for_closure = publication_id.clone();
    let Some(Some((fence, true))) = state.try_update(|s| {
        s.with_sync_mut(|sync, _| {
            if let Some(p) = presentation {
                sync.remember_audio_presentation(p);
            }
            if sync
                .progress_for(&pub_for_closure)
                .is_some_and(|e| e.record.is_some() || e.point_read_in_flight)
            {
                false
            } else {
                sync.begin_progress_read(&pub_for_closure)
            }
        })
    }) else {
        return;
    };
    leptos::task::spawn_local(run_progress_point_read(state, fence, publication_id, 1));
}

async fn run_progress_point_read(
    state: RwSignal<AccountViewState>,
    fence: RequestFence,
    publication_id: String,
    session_change_budget: u8,
) {
    let result = super::library::load_progress_point(fence.session(), &publication_id).await;
    match result {
        Ok(record) => {
            let mut next_dispatch = None;
            let _ = state.try_update(|s| {
                s.with_fenced_sync_mut(&fence, |sync, _| {
                    next_dispatch = sync.finish_progress_read(
                        &publication_id,
                        record,
                        generate_uuid_string,
                        None,
                    );
                })
            });
            persist_sync_journal(state);
            if let Some(pending) = next_dispatch {
                leptos::task::spawn_local(execute_mutation(
                    state,
                    fence,
                    pending,
                    session_change_budget,
                ));
            }
        }
        Err(error) => {
            if let Some(failure) = error.to_account_failure() {
                handle_command_auth_failure(state, &fence, failure, session_change_budget).await;
                return;
            }
            let _ = state.try_update(|s| {
                s.with_fenced_sync_mut(&fence, |sync, _| {
                    sync.fail_progress_read(&publication_id);
                })
            });
        }
    }
}

fn trigger_reread(
    state: RwSignal<AccountViewState>,
    fence: RequestFence,
    target: RereadTarget,
    session_change_budget: u8,
) {
    match target {
        RereadTarget::Saved { work_id } => {
            let w = work_id.clone();
            let started = state
                .try_update(|s| s.with_fenced_sync_mut(&fence, |sync, _| sync.begin_saved_read(&w)))
                .flatten()
                .unwrap_or(false);
            if started {
                leptos::task::spawn_local(run_saved_point_read(
                    state,
                    fence,
                    work_id,
                    session_change_budget,
                ));
            }
        }
        RereadTarget::Progress { publication_id } => {
            let p = publication_id.clone();
            let started = state
                .try_update(|s| {
                    s.with_fenced_sync_mut(&fence, |sync, _| sync.begin_progress_read(&p))
                })
                .flatten()
                .unwrap_or(false);
            if started {
                leptos::task::spawn_local(run_progress_point_read(
                    state,
                    fence,
                    publication_id,
                    session_change_budget,
                ));
            }
        }
        RereadTarget::History => {
            load_library(state, Section::History, true, session_change_budget, false);
        }
    }
}

async fn apply_and_follow_receipt(
    state: RwSignal<AccountViewState>,
    fence: RequestFence,
    operation_id: String,
    receipt: Receipt,
    recovered_via_lookup: bool,
    session_change_budget: u8,
) {
    let op_for_closure = operation_id.clone();
    let applied = state
        .try_update(|s| {
            s.with_fenced_sync_mut(&fence, |sync, library| {
                let res = sync.apply_receipt(
                    &op_for_closure,
                    receipt,
                    recovered_via_lookup,
                    generate_uuid_string,
                    None,
                );
                if let (Ok(applied), Some(lib)) = (&res, library) {
                    if let Some((audio, started_at_ms)) = &applied.accepted_history_start {
                        record_started_in_projection(lib, sync, audio, *started_at_ms);
                    }
                }
                res
            })
        })
        .flatten();
    persist_sync_journal(state);
    match applied {
        Some(Ok(applied)) => {
            if let Some(reread) = applied.reread {
                trigger_reread(state, fence.clone(), reread, session_change_budget);
            }
            if let Some(next) = applied.next_dispatch {
                leptos::task::spawn_local(execute_mutation(
                    state,
                    fence,
                    next,
                    session_change_budget,
                ));
            }
        }
        Some(Err(error)) => {
            let _ = state.try_update(|s| {
                s.with_fenced_sync_mut(&fence, |sync, _| {
                    sync.record_command_error(&operation_id, error, generate_uuid_string);
                })
            });
            persist_sync_journal(state);
        }
        None => (),
    }
}

async fn execute_mutation(
    state: RwSignal<AccountViewState>,
    fence: RequestFence,
    pending: PendingMutation,
    session_change_budget: u8,
) {
    if !state
        .try_with_untracked(|s| s.accepts(&fence))
        .unwrap_or(false)
    {
        return;
    }
    if !persist_sync_journal(state) {
        let _ = state.try_update(|s| {
            s.with_fenced_sync_mut(&fence, |sync, _| {
                sync.record_unknown_outcome(&pending.operation_id)
            });
        });
        return;
    }
    if pending.recovery == RecoveryMode::ReadReceiptFirst {
        match super::library::load_operation_receipt(fence.session(), &pending.operation_id).await {
            super::library::OperationLookupOutcome::Found(receipt) => {
                apply_and_follow_receipt(
                    state,
                    fence,
                    pending.operation_id,
                    *receipt,
                    true,
                    session_change_budget,
                )
                .await;
                return;
            }
            super::library::OperationLookupOutcome::NotFound => (),
            super::library::OperationLookupOutcome::Error(error) => {
                if let Some(failure) = error.to_account_failure() {
                    handle_command_auth_failure(state, &fence, failure, session_change_budget)
                        .await;
                    return;
                }
                let _ = state.try_update(|s| {
                    s.with_fenced_sync_mut(&fence, |sync, _| {
                        sync.record_unknown_outcome(&pending.operation_id);
                    })
                });
                persist_sync_journal(state);
                return;
            }
            super::library::OperationLookupOutcome::UnknownOutcome => {
                let _ = state.try_update(|s| {
                    s.with_fenced_sync_mut(&fence, |sync, _| {
                        sync.record_unknown_outcome(&pending.operation_id);
                    })
                });
                persist_sync_journal(state);
                return;
            }
        }
    }

    // Receipt lookup awaited I/O: account B must never inherit account A's replay callback.
    if !state
        .try_with_untracked(|s| s.accepts(&fence))
        .unwrap_or(false)
    {
        return;
    }
    match super::library::send_mutation(fence.session(), &pending.payload).await {
        super::library::WriteHttpOutcome::Receipt(receipt) => {
            apply_and_follow_receipt(
                state,
                fence,
                pending.operation_id,
                *receipt,
                false,
                session_change_budget,
            )
            .await;
        }
        super::library::WriteHttpOutcome::Error(error) => {
            if let Some(failure) = error.to_account_failure() {
                handle_command_auth_failure(state, &fence, failure, session_change_budget).await;
                return;
            }
            let _ = state.try_update(|s| {
                s.with_fenced_sync_mut(&fence, |sync, _| {
                    sync.record_command_error(&pending.operation_id, error, generate_uuid_string);
                })
            });
            persist_sync_journal(state);
        }
        super::library::WriteHttpOutcome::UnknownOutcome => {
            let _ = state.try_update(|s| {
                s.with_fenced_sync_mut(&fence, |sync, _| {
                    sync.record_unknown_outcome(&pending.operation_id);
                })
            });
            persist_sync_journal(state);
        }
    }
}

pub fn set_saved(
    state: RwSignal<AccountViewState>,
    work_id: String,
    desired: bool,
    presentation: Option<WorkPresentation>,
) {
    let Some(Some((fence, pending))) = state.try_update(|s| {
        s.with_sync_mut(|sync, _| {
            if let Some(w) = presentation {
                sync.remember_work_presentation(w);
            }
            sync.request_saved(&work_id, desired, generate_uuid_string(), None)
        })
    }) else {
        return;
    };
    persist_sync_journal(state);
    if let Some(pending) = pending {
        leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
    }
}

pub fn retry_saved(state: RwSignal<AccountViewState>, work_id: String) {
    let Some(Some((fence, Some(pending)))) = state.try_update(|s| {
        s.with_sync_mut(|sync, _| sync.retry_saved(&work_id, generate_uuid_string, None))
    }) else {
        return;
    };
    persist_sync_journal(state);
    leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
}

pub fn handle_playback_observation(
    state: RwSignal<AccountViewState>,
    now_playing: RwSignal<Option<NowPlaying>>,
    observation: PlaybackObservation,
) {
    if state
        .try_with_untracked(|s| s.phase())
        .unwrap_or(SessionPhase::Idle)
        != SessionPhase::SignedIn
    {
        return;
    }
    let now = now_ms();
    match observation {
        PlaybackObservation::SessionReset => {
            let _ = state.try_update(|s| {
                s.with_sync_mut(|sync, _| {
                    sync.reset_engine_session();
                })
            });
        }
        PlaybackObservation::EngineStarted { identity, source } => {
            let pres = presentation_from_now_playing(source, now_playing);
            let pub_id = audio_identity_from_source(source).map(|a| a.publication_id);
            let dispatched = state
                .try_update(|s| {
                    s.with_sync_mut(|sync, _| {
                        if let Some(p) = pres.clone() {
                            sync.remember_audio_presentation(p);
                        }
                        sync.record_engine_started(
                            identity,
                            source,
                            generate_uuid_string,
                            generate_uuid_string,
                            None,
                        )
                    })
                })
                .flatten();
            if let Some((fence, Some(pending))) = dispatched {
                persist_sync_journal(state);
                leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
            }
            if let Some(publication_id) = pub_id {
                ensure_progress_loaded(state, publication_id, pres);
            }
        }
        PlaybackObservation::PositionTick {
            source,
            position_ms,
        } => {
            let Some(audio) = audio_identity_from_source(source) else {
                return;
            };
            let pres = presentation_from_now_playing(source, now_playing);
            let dispatched = state
                .try_update(|s| {
                    s.with_sync_mut(|sync, _| {
                        if let Some(p) = pres {
                            sync.remember_audio_presentation(p);
                        }
                        sync.record_progress_intent(
                            audio,
                            u64::from(position_ms),
                            ProgressKind::Checkpoint,
                            false,
                            now,
                            generate_uuid_string,
                        )
                    })
                })
                .flatten();
            if let Some((fence, Some(pending))) = dispatched {
                persist_sync_journal(state);
                leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
            }
        }
        PlaybackObservation::SeekConfirmed {
            source,
            position_ms,
        } => {
            let Some(audio) = audio_identity_from_source(source) else {
                return;
            };
            let pres = presentation_from_now_playing(source, now_playing);
            let dispatched = state
                .try_update(|s| {
                    s.with_sync_mut(|sync, _| {
                        if let Some(p) = pres {
                            sync.remember_audio_presentation(p);
                        }
                        sync.record_progress_intent(
                            audio,
                            u64::from(position_ms),
                            ProgressKind::ExplicitSeek,
                            true,
                            now,
                            generate_uuid_string,
                        )
                    })
                })
                .flatten();
            if let Some((fence, Some(pending))) = dispatched {
                persist_sync_journal(state);
                leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
            }
        }
        PlaybackObservation::FlushCheckpoint {
            source,
            position_ms,
        } => {
            let Some(audio) = audio_identity_from_source(source) else {
                return;
            };
            let pres = presentation_from_now_playing(source, now_playing);
            let dispatched = state
                .try_update(|s| {
                    s.with_sync_mut(|sync, _| {
                        if let Some(p) = pres {
                            sync.remember_audio_presentation(p);
                        }
                        sync.record_progress_intent(
                            audio,
                            u64::from(position_ms),
                            ProgressKind::Checkpoint,
                            true,
                            now,
                            generate_uuid_string,
                        )
                    })
                })
                .flatten();
            if let Some((fence, Some(pending))) = dispatched {
                persist_sync_journal(state);
                leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
            }
        }
    }
}

pub fn mark_completed(
    state: RwSignal<AccountViewState>,
    player: PlayerSession,
    now_playing: RwSignal<Option<NowPlaying>>,
) {
    let Some(source) = player.source.get_untracked() else {
        return;
    };
    let Some(audio) = audio_identity_from_source(source) else {
        return;
    };
    let pres = presentation_from_now_playing(source, now_playing);
    let now = now_ms();
    let dispatched = state
        .try_update(|s| {
            s.with_sync_mut(|sync, _| {
                if let Some(p) = pres {
                    sync.remember_audio_presentation(p);
                }
                sync.record_progress_intent(
                    audio.clone(),
                    audio.duration_ms,
                    ProgressKind::MarkCompleted,
                    true,
                    now,
                    generate_uuid_string,
                )
            })
        })
        .flatten();
    if let Some((fence, Some(pending))) = dispatched {
        persist_sync_journal(state);
        leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
    }
}

pub fn restart_progress(
    state: RwSignal<AccountViewState>,
    player: PlayerSession,
    now_playing: RwSignal<Option<NowPlaying>>,
) {
    let Some(source) = player.source.get_untracked() else {
        return;
    };
    let Some(audio) = audio_identity_from_source(source) else {
        return;
    };
    let pres = presentation_from_now_playing(source, now_playing);
    let now = now_ms();
    let dispatched = state
        .try_update(|s| {
            s.with_sync_mut(|sync, _| {
                if let Some(p) = pres {
                    sync.remember_audio_presentation(p);
                }
                sync.record_progress_intent(
                    audio,
                    0,
                    ProgressKind::Restart,
                    true,
                    now,
                    generate_uuid_string,
                )
            })
        })
        .flatten();
    if let Some((fence, Some(pending))) = dispatched {
        persist_sync_journal(state);
        leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
    }
    let speed = current_speed_permille(&player);
    if let Some(snapshot) = build_resume_snapshot(source, 0, speed) {
        player.restore(snapshot);
    }
}

pub fn resolve_progress_conflict(
    state: RwSignal<AccountViewState>,
    player: PlayerSession,
    publication_id: String,
    keep_local: bool,
) {
    let pub_for_closure = publication_id.clone();
    let resolved = state
        .try_update(|s| {
            s.with_sync_mut(|sync, _| {
                sync.resolve_progress_conflict(
                    &pub_for_closure,
                    keep_local,
                    generate_uuid_string,
                    None,
                )
            })
        })
        .flatten();
    let Some((fence, Some((pending, resolved_pos_ms)))) = resolved else {
        return;
    };
    persist_sync_journal(state);
    if !keep_local {
        if let Some(source) = player.source.get_untracked() {
            if audio_identity_from_source(source)
                .is_some_and(|a| a.publication_id == publication_id)
            {
                let clamped = u32::try_from(resolved_pos_ms)
                    .unwrap_or(u32::MAX)
                    .min(source.duration_ms());
                let speed = current_speed_permille(&player);
                if let Some(snapshot) = build_resume_snapshot(source, clamped, speed) {
                    player.restore(snapshot);
                }
            }
        }
    }
    leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
}

pub fn retry_progress(state: RwSignal<AccountViewState>, publication_id: String) {
    let Some(Some((fence, Some(pending)))) = state.try_update(|s| {
        s.with_sync_mut(|sync, _| sync.retry_progress(&publication_id, generate_uuid_string, None))
    }) else {
        return;
    };
    persist_sync_journal(state);
    leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
}

pub fn prompt_clear_history(state: RwSignal<AccountViewState>) {
    let _ = state.try_update(|s| {
        s.with_sync_mut(|sync, _| {
            sync.prompt_clear_history();
        })
    });
}

pub fn cancel_clear_history(state: RwSignal<AccountViewState>) {
    let _ = state.try_update(|s| {
        s.with_sync_mut(|sync, _| {
            sync.cancel_clear_history();
        })
    });
}

pub fn confirm_clear_history(state: RwSignal<AccountViewState>) {
    let Some(Some((fence, Some(pending)))) = state.try_update(|s| {
        s.with_sync_mut(|sync, _| sync.confirm_clear_history(generate_uuid_string, None))
    }) else {
        return;
    };
    persist_sync_journal(state);
    leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
}

pub fn retry_history(state: RwSignal<AccountViewState>) {
    let Some(Some((fence, Some(pending)))) = state
        .try_update(|s| s.with_sync_mut(|sync, _| sync.retry_history(generate_uuid_string, None)))
    else {
        return;
    };
    persist_sync_journal(state);
    leptos::task::spawn_local(execute_mutation(state, fence, pending, 1));
}

pub fn resume_exact_release(
    state: RwSignal<AccountViewState>,
    player: PlayerSession,
    now_playing: RwSignal<Option<NowPlaying>>,
    audio: AudioIdentity,
    position_ms: u64,
    resume: ResumeStatus,
    presentation: Option<AudioPresentation>,
) {
    let pub_for_closure = audio.publication_id.clone();
    let pres_for_closure = presentation.clone();
    let Some(Some((fence, ticket))) = state.try_update(|s| {
        s.with_sync_mut(|sync, _| {
            if let Some(p) = pres_for_closure {
                sync.remember_audio_presentation(p);
            }
            match resume {
                ResumeStatus::ReleaseReplaced => {
                    sync.set_resume_attempt(ResumeAttemptState::Refused {
                        publication_id: pub_for_closure,
                        reason: ResumeRefusal::ReleaseReplaced,
                    });
                    None
                }
                ResumeStatus::AccessUnavailable => {
                    sync.set_resume_attempt(ResumeAttemptState::Refused {
                        publication_id: pub_for_closure,
                        reason: ResumeRefusal::AccessUnavailable,
                    });
                    None
                }
                ResumeStatus::ExactAvailable => sync.begin_resume(pub_for_closure),
            }
        })
    }) else {
        return;
    };
    let Some(ticket) = ticket else {
        return;
    };
    let player_generation = player.state.get_untracked().generation();

    // If the exact release is already loaded in the player, restore the position paused directly.
    if let Some(source) = player.source.get_untracked() {
        if audio_identity_from_source(source).as_ref() == Some(&audio) {
            let clamped = u32::try_from(position_ms.min(audio.duration_ms))
                .unwrap_or(0)
                .min(source.duration_ms());
            let speed = current_speed_permille(&player);
            if let Some(snapshot) = build_resume_snapshot(source, clamped, speed) {
                player.restore(snapshot);
                let _ = state.try_update(|s| {
                    s.with_fenced_sync_mut(&fence, |sync, _| {
                        sync.set_resume_attempt(ResumeAttemptState::Restored {
                            publication_id: audio.publication_id.clone(),
                            position_ms: clamped,
                        });
                    })
                });
                return;
            }
        }
    }

    let Some(work) = PublicWorkId::parse(&audio.work_id) else {
        return;
    };
    let Some(request) = ReleaseRequest::parse(work, &audio.unit_id, &audio.publication_id) else {
        return;
    };

    leptos::task::spawn_local(async move {
        let result = match web_sys::AbortController::new() {
            Ok(controller) => {
                super::public_media::load_manifest(request.publication(), controller).await
            }
            Err(_) => Err(CatalogReadError::Unavailable),
        };
        let Some(accepted) = state.try_with_untracked(|s| {
            s.accepts(&fence) && s.sync().is_some_and(|sync| sync.accepts_resume(ticket))
        }) else {
            return;
        };
        if !accepted
            || player.state.try_with_untracked(|s| s.generation()) != Some(player_generation)
        {
            return;
        }
        match result {
            Ok(manifest) => {
                let Ok(release) = PinnedRelease::admit(&manifest, request) else {
                    let _ = state.try_update(|s| {
                        s.with_fenced_sync_mut(&fence, |sync, _| {
                            sync.set_resume_attempt(ResumeAttemptState::Refused {
                                publication_id: audio.publication_id.clone(),
                                reason: ResumeRefusal::ReleaseReplaced,
                            });
                        })
                    });
                    return;
                };
                if !release_matches_audio_identity(&release, &audio) {
                    let _ = state.try_update(|s| {
                        s.with_fenced_sync_mut(&fence, |sync, _| {
                            sync.set_resume_attempt(ResumeAttemptState::Refused {
                                publication_id: audio.publication_id.clone(),
                                reason: ResumeRefusal::ReleaseReplaced,
                            });
                        })
                    });
                    return;
                }
                let Some(source) = Source::public(release.track(), release.duration_ms()) else {
                    return;
                };
                let clamped = u32::try_from(position_ms.min(audio.duration_ms))
                    .unwrap_or(0)
                    .min(source.duration_ms());
                let speed = current_speed_permille(&player);
                let Some(snapshot) = build_resume_snapshot(source, clamped, speed) else {
                    return;
                };
                if let Some(pres) = presentation {
                    now_playing.set(Some(NowPlaying::new(
                        &release,
                        ListenLabel {
                            work_title: pres.work.title,
                            unit_title: pres.unit_title,
                            language: pres.work.language,
                        },
                    )));
                }
                player.restore(snapshot);
                let _ = state.try_update(|s| {
                    s.with_fenced_sync_mut(&fence, |sync, _| {
                        sync.set_resume_attempt(ResumeAttemptState::Restored {
                            publication_id: audio.publication_id,
                            position_ms: clamped,
                        });
                    })
                });
            }
            Err(CatalogReadError::NotFound) => {
                let _ = state.try_update(|s| {
                    s.with_fenced_sync_mut(&fence, |sync, _| {
                        sync.set_resume_attempt(ResumeAttemptState::Refused {
                            publication_id: audio.publication_id,
                            reason: ResumeRefusal::AccessUnavailable,
                        });
                    })
                });
            }
            Err(
                CatalogReadError::Diagnostic(_)
                | CatalogReadError::Unavailable
                | CatalogReadError::Protocol
                | CatalogReadError::InvalidCursor,
            ) => {
                let _ = state.try_update(|s| {
                    s.with_fenced_sync_mut(&fence, |sync, _| {
                        sync.set_resume_attempt(ResumeAttemptState::Refused {
                            publication_id: audio.publication_id,
                            reason: ResumeRefusal::LoadFailed,
                        });
                    })
                });
            }
        }
    });
}

pub fn logout(state: RwSignal<AccountViewState>) {
    let current = state.get_untracked();
    if current.cleanup_in_flight() {
        return;
    }
    let mut current = current;
    let Ok(issued) = current.begin_logout() else {
        return;
    };
    let intent = current.cleanup_intent().clone();
    state.set(current);
    let expected = match &intent {
        CleanupIntent::ExpectedSession(session) => Some(session.as_str()),
        CleanupIntent::StaleOnly => None,
    };
    if let Ok(client) = crate::api_endpoint::auth() {
        let _ = client.begin_logout(expected);
    }
    leptos::task::spawn_local(cleanup(state, issued, intent, 1));
}

pub fn login(
    state: RwSignal<AccountViewState>,
    configuration: crate::application::registration::RegistrationState,
) {
    if !state.get_untracked().can_login(configuration) {
        return;
    }
    let login = match super::account::reserve_login() {
        Ok(login) => login,
        Err(error) => {
            state.update(|s| s.navigation_failed(error));
            return;
        }
    };
    let mut current = state.get_untracked();
    let Ok(issued) = current.begin_refresh() else {
        return;
    };
    state.set(current);
    leptos::task::spawn_local(async move {
        let result = match login.run().await {
            Ok(()) => super::account::session().await,
            Err(cantos_web_auth::Failure::Cancelled | cantos_web_auth::Failure::Changed) => {
                Err(AccountFailure::SignedOut)
            }
            Err(error) => Err(super::account::auth_error(error)),
        };
        finish_lookup(state, issued, result, 1).await;
    });
}

pub fn cancel_login() {
    if let Ok(client) = crate::api_endpoint::auth() {
        if client.cancel_authorization().is_ok() {
            let _ = client.invalidate();
            leptos::task::spawn_local(async move {
                let _ = client.settled_cleanup().await;
            });
        }
    }
}

fn credentials_lost(state: RwSignal<AccountViewState>) {
    let Some(mut current) = state.try_get_untracked() else {
        return;
    };
    if current.phase() != SessionPhase::SignedIn {
        return;
    }
    let Ok(issued) = current.begin_logout() else {
        return;
    };
    state.set(current);
    leptos::task::spawn_local(async move {
        let result = match crate::api_endpoint::auth() {
            Ok(client) => client
                .settled_cleanup()
                .await
                .map_err(super::account::auth_error),
            Err(error) => Err(super::account::auth_error(error)),
        };
        let _ = state.try_update(|current| {
            current.finish_logout(issued, result);
        });
    });
}

pub fn mount(
    state: RwSignal<AccountViewState>,
    player: PlayerSession,
    now_playing: RwSignal<Option<NowPlaying>>,
) {
    use leptos::leptos_dom::helpers::window_event_listener_untyped;

    let credential_loss = window_event_listener_untyped(
        cantos_web_auth::browser::invalidation_event(cantos_web_auth::Surface::Theatre),
        move |_| credentials_lost(state),
    );
    let focus = window_event_listener_untyped("focus", move |_| refresh(state));
    let visibility = window_event_listener_untyped("visibilitychange", move |_| {
        let hidden = web_sys::window()
            .and_then(|w| w.document())
            .is_some_and(|doc| doc.hidden());
        if hidden {
            if let (Some(source), Some(position_ms)) = (
                player.source.try_get_untracked().flatten(),
                player.position_ms.try_get_untracked().flatten(),
            ) {
                handle_playback_observation(
                    state,
                    now_playing,
                    PlaybackObservation::FlushCheckpoint {
                        source,
                        position_ms,
                    },
                );
            }
        }
    });
    let timer = StoredValue::new_local(None::<(web_sys::Window, i32, Closure<dyn FnMut()>)>);
    if let Some(window) = web_sys::window() {
        let observed_window = window.clone();
        let callback = Closure::wrap(Box::new(move || {
            if observed_window
                .document()
                .is_some_and(|document| document.hidden())
            {
                return;
            }
            if state.get_untracked().phase() == SessionPhase::SignedIn {
                refresh(state);
            }
        }) as Box<dyn FnMut()>);
        if let Ok(handle) = window.set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            60_000,
        ) {
            timer.set_value(Some((window, handle, callback)));
        }
    }
    on_cleanup(move || {
        focus.remove();
        visibility.remove();
        credential_loss.remove();
        timer.update_value(|timer| {
            if let Some((window, handle, callback)) = timer.take() {
                window.clear_interval_with_handle(handle);
                drop(callback);
            }
        });
    });
    refresh(state);
}

/// Explicit storage recovery. An unread journal is reloaded through account admission; a write
/// failure retries persistence in this mount, then resumes only the exact retained receipt plans.
pub fn retry_journal_storage(state: RwSignal<AccountViewState>) {
    if state.with_untracked(AccountViewState::journal_restore_blocked) {
        refresh(state);
        return;
    }
    if !persist_sync_journal(state) {
        return;
    }
    let Some(Some((fence, pending))) =
        state.try_with_untracked(|s| Some((s.fence()?, s.sync()?.pending_mutations())))
    else {
        return;
    };
    for operation in pending {
        leptos::task::spawn_local(execute_mutation(state, fence.clone(), operation, 1));
    }
}
