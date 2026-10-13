//! Production use cases with a named transactional-memory persistence double, never account-sync acceptance.
use super::*;
use crate::contexts::{
    catalog::domain::OwnerId,
    listening::domain::{
        reconcile_progress, reconcile_saved, AudioIdentity, HistoryState, Revision,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};
#[derive(Default, Clone)]
struct Account {
    order: u64,
    progress: BTreeMap<String, ProgressRecord>,
    saved: BTreeMap<String, SavedRecord>,
    history: BTreeMap<String, (dto::AudioIdentity, u64)>,
    history_state: Option<HistoryState>,
    sessions: BTreeSet<String>,
    receipts: BTreeMap<String, (Vec<u8>, dto::Receipt)>,
    sequences: BTreeMap<(String, String), u64>,
}
#[derive(Default)]
struct World {
    accounts: BTreeMap<String, Account>,
    withdrawn: bool,
    fail_before_commit: bool,
}
#[derive(Clone, Default)]
struct MemoryRepository(Arc<Mutex<World>>);
impl LibraryRepository for MemoryRepository {
    fn mutate(&self, listener: ListenerId, request: Mutation) -> WriteFuture {
        let world = self.0.clone();
        Box::pin(async move {
            let mut w = world.lock().unwrap();
            let mut state = w
                .accounts
                .get(listener.as_str())
                .cloned()
                .unwrap_or_default();
            let intent = request.command.intent();
            let operation = intent.operation().as_str().to_owned();
            if let Some((fingerprint, receipt)) = state.receipts.get(&operation) {
                return if fingerprint == &request.fingerprint {
                    Ok(receipt.clone())
                } else {
                    Err(LibraryError::OperationReused)
                };
            }
            let seq_key = (
                intent.device().as_str().to_owned(),
                request.command.sequence_key(),
            );
            if state
                .sequences
                .get(&seq_key)
                .is_some_and(|v| *v >= intent.sequence())
            {
                return Err(LibraryError::StaleSequence);
            }
            let order = Revision::new(state.order).unwrap().next()?;
            let h = state
                .history_state
                .unwrap_or(HistoryState::restore(0, 0).unwrap());
            let receipt = match &request.command {
                Command::Progress(c) => {
                    let key = c.audio().publication().as_str().to_owned();
                    if w.withdrawn && !state.progress.contains_key(&key) {
                        return Err(LibraryError::NotFound);
                    }
                    let out = reconcile_progress(state.progress.get(&key), c, 1000, order)?;
                    if let ProgressOutcome::Accepted(record) = &out {
                        state.progress.insert(key, record.clone());
                        state.order = order.value();
                    }
                    progress_receipt(intent.operation(), &out)
                }
                Command::Saved(c) => {
                    let old = state
                        .saved
                        .get(c.work().as_str())
                        .cloned()
                        .unwrap_or(SavedRecord::empty(c.work()));
                    let out = reconcile_saved(&old, c, 1000, order)?;
                    if let SavedOutcome::Accepted(record) = &out {
                        state.saved.insert(c.work().as_str().into(), record.clone());
                        state.order = order.value();
                    }
                    saved_receipt(intent.operation(), &out)
                }
                Command::Start(c) => {
                    if w.withdrawn {
                        return Err(LibraryError::NotFound);
                    }
                    let session = format!("{}:{}", intent.device().as_str(), c.session().as_str());
                    let out = h.start(c, state.sessions.contains(&session))?;
                    if let HistoryOutcome::Accepted(next) = out {
                        state.history_state = Some(next);
                        state.sessions.insert(session);
                        state.history.insert(
                            c.audio().work().as_str().into(),
                            (c.audio().raw(), order.value()),
                        );
                        state.order = order.value();
                    }
                    history_receipt(intent.operation(), out)
                }
                Command::Clear(c) => {
                    let out = h.clear(c)?;
                    if let HistoryOutcome::Accepted(next) = out {
                        state.history_state = Some(next);
                        state.history.clear();
                        state.order = order.value();
                    }
                    history_receipt(intent.operation(), out)
                }
            };
            state.sequences.insert(seq_key, intent.sequence());
            state
                .receipts
                .insert(operation, (request.fingerprint, receipt.clone()));
            if w.fail_before_commit {
                return Err(LibraryError::Unavailable);
            }
            w.accounts.insert(listener.as_str().into(), state);
            Ok(receipt)
        })
    }
    fn read(&self, listener: ListenerId, request: ReadRequest) -> ReadFuture {
        let world = self.0.clone();
        Box::pin(async move {
            let w = world.lock().unwrap();
            let state = w
                .accounts
                .get(listener.as_str())
                .cloned()
                .unwrap_or_default();
            Ok(match request {
                ReadRequest::Progress(id) => ReadResult::Progress(state.progress.get(&id).cloned()),
                ReadRequest::Saved(work) => ReadResult::Saved(
                    state
                        .saved
                        .get(work.as_str())
                        .cloned()
                        .unwrap_or(SavedRecord::empty(&work)),
                ),
                ReadRequest::Receipt(id) => ReadResult::Receipt(
                    state
                        .receipts
                        .get(id.as_str())
                        .ok_or(LibraryError::NotFound)?
                        .1
                        .clone(),
                ),
                ReadRequest::Page {
                    kind,
                    limit,
                    anchor: _,
                } => {
                    let data = match kind {
                        PageKind::Saved => PageData::Saved(
                            state
                                .saved
                                .values()
                                .filter(|v| v.raw().saved)
                                .take(limit)
                                .map(|v| dto::SavedItem {
                                    saved: v.raw().clone(),
                                })
                                .collect(),
                        ),
                        PageKind::Continue => PageData::Continue(
                            state
                                .progress
                                .values()
                                .filter(|v| v.raw().completion == dto::Completion::InProgress)
                                .take(limit)
                                .map(|v| dto::ContinueItem {
                                    progress: v.raw().clone(),
                                    resume: if w.withdrawn {
                                        dto::ResumeStatus::AccessUnavailable
                                    } else {
                                        dto::ResumeStatus::ExactAvailable
                                    },
                                })
                                .collect(),
                        ),
                        PageKind::History => PageData::History {
                            state: state
                                .history_state
                                .unwrap_or(HistoryState::restore(0, 0).unwrap())
                                .raw(),
                            items: state
                                .history
                                .values()
                                .take(limit)
                                .map(|(audio, order)| dto::HistoryItem {
                                    audio: audio.clone(),
                                    started_at_ms: 1000,
                                    listener_order: *order,
                                    progress: state
                                        .progress
                                        .get(&audio.publication_id)
                                        .map(|v| v.raw().clone()),
                                    resume: if w.withdrawn {
                                        dto::ResumeStatus::AccessUnavailable
                                    } else {
                                        dto::ResumeStatus::ExactAvailable
                                    },
                                })
                                .collect(),
                        },
                    };
                    ReadResult::Page(PageSnapshot { data, next: None })
                }
            })
        })
    }
}
fn uid(v: u64) -> String {
    format!("00000000-0000-4000-8000-{v:012x}")
}
fn intent(n: u64) -> dto::Operation {
    dto::Operation {
        operation_id: uid(n + 100),
        device_id: uid(90),
        device_sequence: n,
        device_reported_at_ms: None,
    }
}
fn principal(actor: &str) -> AuthenticatedListener {
    AuthenticatedListener::from_verified_actor(OwnerId::parse(actor).unwrap())
}
fn audio() -> dto::AudioIdentity {
    dto::AudioIdentity {
        work_id: uid(1),
        unit_id: uid(2),
        publication_id: uid(3),
        rendition_id: uid(4),
        sha256: "a".repeat(64),
        duration_ms: 60000,
    }
}
fn write(n: u64, base: u64, pos: u64, kind: dto::ProgressKind) -> dto::ProgressWrite {
    dto::ProgressWrite {
        operation: intent(n),
        audio: audio(),
        base_revision: base,
        position_ms: pos,
        kind,
    }
}
fn service(repository: &MemoryRepository) -> LibraryService {
    LibraryService {
        repository: Arc::new(repository.clone()),
        cursors: Arc::new(CursorVault::default()),
    }
}
#[tokio::test]
async fn ten_identical_retries_have_one_record_one_revision_and_one_receipt_after_restart() {
    let repo = MemoryRepository::default();
    let svc = service(&repo);
    let a = principal("alice");
    let request = write(1, 0, 15000, dto::ProgressKind::Checkpoint);
    let first = svc
        .put_progress(&a, &uid(3), request.clone())
        .await
        .unwrap();
    for _ in 1..10 {
        assert_eq!(
            svc.put_progress(&a, &uid(3), request.clone()).await,
            Ok(first.clone())
        );
    }
    let restarted = service(&repo);
    assert_eq!(
        restarted.receipt(&a, &request.operation.operation_id).await,
        Ok(first)
    );
    let w = repo.0.lock().unwrap();
    let account = &w.accounts["alice"];
    assert_eq!(
        (
            account.progress.len(),
            account.receipts.len(),
            account.order
        ),
        (1, 1, 1)
    );
}
#[tokio::test]
async fn changed_intent_key_and_old_sequence_are_exact_atomic_rejections() {
    let repo = MemoryRepository::default();
    let svc = service(&repo);
    let a = principal("alice");
    let original = write(1, 0, 1000, dto::ProgressKind::Checkpoint);
    svc.put_progress(&a, &uid(3), original.clone())
        .await
        .unwrap();
    let before = svc.progress(&a, &uid(3)).await.unwrap();
    let mut changed = original.clone();
    changed.position_ms = 2000;
    assert_eq!(
        svc.put_progress(&a, &uid(3), changed).await,
        Err(LibraryError::OperationReused)
    );
    let mut stale = original;
    stale.operation.operation_id = uid(999);
    stale.base_revision = 1;
    assert_eq!(
        svc.put_progress(&a, &uid(3), stale).await,
        Err(LibraryError::StaleSequence)
    );
    assert_eq!(svc.progress(&a, &uid(3)).await.unwrap(), before);
}
#[tokio::test]
async fn save_unsave_race_and_reversed_receipts_never_flip_desired_server_state() {
    let repo = MemoryRepository::default();
    let svc = service(&repo);
    let a = principal("alice");
    let save = dto::SavedWrite {
        operation: intent(1),
        base_revision: 0,
        saved: true,
    };
    let older = svc.set_saved(&a, &uid(1), save.clone()).await.unwrap();
    let remove = dto::SavedWrite {
        operation: intent(2),
        base_revision: 1,
        saved: false,
    };
    let newer = svc.set_saved(&a, &uid(1), remove).await.unwrap();
    assert_eq!(svc.set_saved(&a, &uid(1), save).await, Ok(older));
    assert_eq!(svc.saved(&a, &uid(1)).await.unwrap().revision, 2);
    assert!(!svc.saved(&a, &uid(1)).await.unwrap().saved);
    let mut device_b = intent(3);
    device_b.device_id = uid(91);
    let conflict = svc
        .set_saved(
            &a,
            &uid(1),
            dto::SavedWrite {
                operation: device_b,
                base_revision: 1,
                saved: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(conflict.outcome, dto::WriteOutcome::RevisionConflict);
    assert_eq!(conflict.state, newer.state);
}
#[tokio::test]
async fn two_devices_delayed_checkpoint_and_explicit_resolution_preserve_rewind() {
    let repo = MemoryRepository::default();
    let svc = service(&repo);
    let a = principal("alice");
    svc.put_progress(
        &a,
        &uid(3),
        write(1, 0, 50000, dto::ProgressKind::Checkpoint),
    )
    .await
    .unwrap();
    svc.put_progress(
        &a,
        &uid(3),
        write(2, 1, 1000, dto::ProgressKind::ExplicitSeek),
    )
    .await
    .unwrap();
    let mut delayed = write(1, 1, 55000, dto::ProgressKind::Checkpoint);
    delayed.operation.device_id = uid(91);
    delayed.operation.operation_id = uid(1001);
    let conflict = svc.put_progress(&a, &uid(3), delayed).await.unwrap();
    assert_eq!(conflict.resolution, Some(dto::Resolution::KeepServer));
    let mut chosen = write(2, 2, 55000, dto::ProgressKind::ResolveHere);
    chosen.operation.device_id = uid(91);
    chosen.operation.operation_id = uid(1002);
    assert_eq!(
        svc.put_progress(&a, &uid(3), chosen).await.unwrap().outcome,
        dto::WriteOutcome::Accepted
    );
    assert_eq!(
        svc.progress(&a, &uid(3)).await.unwrap().unwrap().revision,
        3
    );
}
#[tokio::test]
async fn only_confirmed_start_creates_history_and_clear_preserves_saved_progress_and_session_tombstones(
) {
    let repo = MemoryRepository::default();
    let svc = service(&repo);
    let a = principal("alice");
    svc.put_progress(
        &a,
        &uid(3),
        write(1, 0, 60000, dto::ProgressKind::MarkCompleted),
    )
    .await
    .unwrap();
    svc.set_saved(
        &a,
        &uid(1),
        dto::SavedWrite {
            operation: intent(2),
            base_revision: 0,
            saved: true,
        },
    )
    .await
    .unwrap();
    assert!(svc.history(&a, 20, None).await.unwrap().items.is_empty());
    let start = dto::HistoryStart {
        operation: intent(3),
        audio: audio(),
        engine_session_id: uid(9),
        event: dto::PlaybackEvent::PlaybackStarted,
        history_epoch: 0,
    };
    let receipt = svc.playback_started(&a, start.clone()).await.unwrap();
    let history = svc.history(&a, 20, None).await.unwrap();
    assert_eq!(history.items.len(), 1);
    assert_eq!(
        history.items[0].progress.as_ref().unwrap().completion,
        dto::Completion::Completed
    );
    svc.clear_history(
        &a,
        dto::HistoryClear {
            operation: intent(4),
            base_revision: 1,
            history_epoch: 0,
            confirmation: dto::ClearConfirmation::ClearHistory,
        },
    )
    .await
    .unwrap();
    assert_eq!(svc.playback_started(&a, start.clone()).await, Ok(receipt));
    assert!(svc.history(&a, 20, None).await.unwrap().items.is_empty());
    let mut delayed = start;
    delayed.operation = intent(5);
    delayed.engine_session_id = uid(10);
    assert_eq!(
        svc.playback_started(&a, delayed).await.unwrap().outcome,
        dto::WriteOutcome::HistoryEpochConflict
    );
    assert!(svc.saved(&a, &uid(1)).await.unwrap().saved);
    assert!(svc.progress(&a, &uid(3)).await.unwrap().is_some());
    assert!(svc
        .continue_listening(&a, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
}
#[tokio::test]
async fn account_switch_and_withdrawal_do_not_leak_or_delete_owned_state() {
    let repo = MemoryRepository::default();
    let svc = service(&repo);
    let a = principal("alice");
    let b = principal("bob");
    let request = write(1, 0, 1000, dto::ProgressKind::Checkpoint);
    svc.put_progress(&a, &uid(3), request.clone())
        .await
        .unwrap();
    assert_eq!(svc.progress(&b, &uid(3)).await.unwrap(), None);
    assert_eq!(
        svc.receipt(&b, &request.operation.operation_id).await,
        Err(LibraryError::NotFound)
    );
    assert!(svc
        .continue_listening(&b, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
    repo.0.lock().unwrap().withdrawn = true;
    assert_eq!(
        svc.continue_listening(&a, 20, None).await.unwrap().items[0].resume,
        dto::ResumeStatus::AccessUnavailable
    );
    assert_eq!(
        svc.put_progress(
            &a,
            &uid(3),
            write(2, 1, 500, dto::ProgressKind::ExplicitSeek)
        )
        .await
        .unwrap()
        .outcome,
        dto::WriteOutcome::Accepted
    );
    assert_eq!(
        svc.put_progress(&b, &uid(3), request).await,
        Err(LibraryError::NotFound)
    );
}
#[tokio::test]
async fn uncertain_precommit_failure_never_commits_effect_or_receipt_and_retry_is_same_intent() {
    let repo = MemoryRepository::default();
    let svc = service(&repo);
    let a = principal("alice");
    repo.0.lock().unwrap().fail_before_commit = true;
    let request = write(1, 0, 1000, dto::ProgressKind::Checkpoint);
    assert_eq!(
        svc.put_progress(&a, &uid(3), request.clone()).await,
        Err(LibraryError::Unavailable)
    );
    assert_eq!(svc.progress(&a, &uid(3)).await.unwrap(), None);
    assert_eq!(
        svc.receipt(&a, &request.operation.operation_id).await,
        Err(LibraryError::NotFound)
    );
    repo.0.lock().unwrap().fail_before_commit = false;
    assert_eq!(
        svc.put_progress(&a, &uid(3), request)
            .await
            .unwrap()
            .outcome,
        dto::WriteOutcome::Accepted
    );
}
#[test]
fn exact_audio_identity_constructor_preserves_full_timing_binding() {
    assert_eq!(AudioIdentity::new(&audio()).unwrap().raw(), audio());
}
