//! Production use cases with a named transactional-memory persistence double, never account-sync acceptance.
use super::*;
use crate::{
    contexts::listening::domain::{
        reconcile_bookmark_create, reconcile_bookmark_delete, reconcile_progress, reconcile_saved,
        AudioIdentity, BookmarkRecord, HistoryState, Revision,
    },
    platform::identity::domain::ActorId,
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
    bookmarks: BTreeMap<String, BookmarkRecord>,
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
    fn mutate(&self, access: Access, request: Mutation) -> WriteFuture {
        let listener = access.listener;
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
                Command::Bookmark(c) => {
                    let key = c.id().as_str().to_owned();
                    if w.withdrawn && !state.bookmarks.contains_key(&key) {
                        return Err(LibraryError::NotFound);
                    }
                    let out = reconcile_bookmark_create(state.bookmarks.get(&key), c, 1000, order)?;
                    if let BookmarkCreateOutcome::Accepted(record) = &out {
                        state.bookmarks.insert(key, record.clone());
                        state.order = order.value();
                    }
                    bookmark_create_receipt(intent.operation(), &out)
                }
                Command::BookmarkDelete(c) => {
                    let key = c.id().as_str().to_owned();
                    let out = reconcile_bookmark_delete(state.bookmarks.get(&key), c, 1000, order)?;
                    if let BookmarkDeleteOutcome::Accepted(record) = &out {
                        state.bookmarks.insert(key, record.clone());
                        state.order = order.value();
                    }
                    let Some(receipt) = bookmark_delete_receipt(intent.operation(), &out) else {
                        return Err(LibraryError::NotFound);
                    };
                    receipt
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
    fn read(&self, access: Access, request: ReadRequest) -> ReadFuture {
        let listener = access.listener;
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
                    anchor,
                    presentation: _,
                } => {
                    let epoch = state
                        .history_state
                        .unwrap_or(HistoryState::restore(0, 0).unwrap())
                        .raw()
                        .epoch;
                    if kind == PageKind::History
                        && anchor.as_ref().is_some_and(|a| a.history_epoch != epoch)
                    {
                        return Err(LibraryError::InvalidCursor);
                    }
                    let upper = anchor.as_ref().map_or(state.order, |a| a.upper_order);
                    let mut keys: Vec<(u64, String)> = match kind {
                        PageKind::Saved => state
                            .saved
                            .values()
                            .filter(|v| v.raw().saved)
                            .map(|v| (v.raw().listener_order, v.raw().work_id.clone()))
                            .collect(),
                        PageKind::Continue => super::super::domain::projection::continue_records(
                            state.progress.values().cloned(),
                        )
                        .into_iter()
                        .map(|v| (v.raw().listener_order, v.raw().audio.work_id.clone()))
                        .collect(),
                        PageKind::History => state
                            .history
                            .iter()
                            .map(|(work, (_, order))| (*order, work.clone()))
                            .collect(),
                        PageKind::Bookmarks => state
                            .bookmarks
                            .values()
                            .filter(|v| !v.deleted())
                            .map(|v| (v.raw().listener_order, v.id().to_owned()))
                            .collect(),
                    };
                    keys.retain(|(order, work)| {
                        *order <= upper
                            && anchor.as_ref().is_none_or(|a| {
                                *order < a.after_order
                                    || (*order == a.after_order && work > &a.after_work)
                            })
                    });
                    keys.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
                    let more = keys.len() > limit;
                    keys.truncate(limit);
                    let next = if more {
                        keys.last().map(|(order, work)| PageAnchor {
                            upper_order: upper,
                            after_order: *order,
                            after_work: work.clone(),
                            history_epoch: epoch,
                            visibility_scope: String::new(),
                        })
                    } else {
                        None
                    };
                    let resume = if w.withdrawn {
                        dto::ResumeStatus::AccessUnavailable
                    } else {
                        dto::ResumeStatus::ExactAvailable
                    };
                    let data = match kind {
                        PageKind::Saved => PageData::Saved(
                            keys.iter()
                                .map(|(_, work)| dto::PresentedSavedItem {
                                    presentation: None,
                                    saved: state.saved[work].raw().clone(),
                                })
                                .collect(),
                        ),
                        PageKind::Continue => {
                            let latest = super::super::domain::projection::continue_records(
                                state.progress.values().cloned(),
                            );
                            PageData::Continue(
                                keys.iter()
                                    .map(|(_, work)| dto::PresentedContinueItem {
                                        presentation: None,
                                        progress: latest
                                            .iter()
                                            .find(|r| &r.raw().audio.work_id == work)
                                            .unwrap()
                                            .raw()
                                            .clone(),
                                        resume,
                                    })
                                    .collect(),
                            )
                        }
                        PageKind::Bookmarks => PageData::Bookmarks(
                            keys.iter()
                                .map(|(_, id)| dto::BookmarkItem {
                                    presentation: None,
                                    bookmark: state.bookmarks[id].raw().clone(),
                                    resume,
                                })
                                .collect(),
                        ),
                        PageKind::History => PageData::History {
                            state: state
                                .history_state
                                .unwrap_or(HistoryState::restore(0, 0).unwrap())
                                .raw(),
                            items: keys
                                .iter()
                                .map(|(order, work)| {
                                    let (audio, _) = &state.history[work];
                                    dto::PresentedHistoryItem {
                                        presentation: None,
                                        audio: audio.clone(),
                                        started_at_ms: 1000,
                                        listener_order: *order,
                                        progress: state
                                            .progress
                                            .get(&audio.publication_id)
                                            .map(|v| v.raw().clone()),
                                        resume,
                                    }
                                })
                                .collect(),
                        },
                    };
                    ReadResult::Page(PageSnapshot { data, next })
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
    AuthenticatedListener::from_verified_actor(ActorId::new(actor).unwrap())
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
        presented_cursors: Arc::new(CursorVault::default()),
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

#[tokio::test]
async fn presentation_is_explicit_and_cursors_cannot_cross_response_contracts() {
    let repo = MemoryRepository::default();
    let svc = service(&repo);
    let listener = principal("alice");
    for n in 1..=2 {
        svc.set_saved(
            &listener,
            &uid(n),
            dto::SavedWrite {
                operation: intent(n),
                base_revision: 0,
                saved: true,
            },
        )
        .await
        .unwrap();
    }
    let legacy = svc.saved_works(&listener, 1, None).await.unwrap();
    let presented = svc.presented_saved_works(&listener, 1, None).await.unwrap();
    let legacy_json = serde_json::to_value(&legacy).unwrap();
    let presented_json = serde_json::to_value(&presented).unwrap();
    assert!(legacy_json["items"][0].get("presentation").is_none());
    // This repository double supplies no public metadata; absence is explicit, never invented.
    assert_eq!(
        presented_json["items"][0]["presentation"],
        serde_json::Value::Null
    );
    assert_eq!(legacy.items[0].saved, presented.items[0].saved);
    assert_eq!(
        svc.presented_saved_works(&listener, 1, legacy.next_cursor.as_deref())
            .await,
        Err(LibraryError::InvalidCursor)
    );
    assert_eq!(
        svc.saved_works(&listener, 1, presented.next_cursor.as_deref())
            .await,
        Err(LibraryError::InvalidCursor)
    );
    assert_eq!(
        svc.presented_saved_works(&listener, 1, presented.next_cursor.as_deref())
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        svc.presented_saved_works(&principal("bob"), 1, presented.next_cursor.as_deref())
            .await,
        Err(LibraryError::InvalidCursor)
    );
}

#[tokio::test]
async fn continue_never_resurrects_old_unfinished_release_and_keysets_are_account_bound() {
    let repo = MemoryRepository::default();
    let svc = service(&repo);
    let a = principal("alice");
    svc.put_progress(&a, &uid(3), write(1, 0, 100, dto::ProgressKind::Checkpoint))
        .await
        .unwrap();
    let mut completed = write(2, 0, 60000, dto::ProgressKind::MarkCompleted);
    completed.audio.publication_id = uid(30);
    completed.audio.rendition_id = uid(31);
    svc.put_progress(&a, &uid(30), completed).await.unwrap();
    assert!(svc
        .continue_listening(&a, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
    for n in 10..14 {
        svc.set_saved(
            &a,
            &uid(n),
            dto::SavedWrite {
                operation: intent(n),
                base_revision: 0,
                saved: true,
            },
        )
        .await
        .unwrap();
    }
    let first = svc.saved_works(&a, 1, None).await.unwrap();
    assert_eq!(first.items[0].saved.work_id, uid(13));
    let token = first.next_cursor.unwrap();
    assert_eq!(
        svc.saved_works(&principal("bob"), 1, Some(&token))
            .await
            .err(),
        Some(LibraryError::InvalidCursor)
    );
    assert_eq!(
        svc.history(&a, 1, Some(&token)).await.err(),
        Some(LibraryError::InvalidCursor)
    );
    assert_eq!(
        svc.saved_works(&a, 2, Some(&token)).await.err(),
        Some(LibraryError::InvalidCursor)
    );
    svc.set_saved(
        &a,
        &uid(12),
        dto::SavedWrite {
            operation: intent(20),
            base_revision: 1,
            saved: false,
        },
    )
    .await
    .unwrap();
    svc.set_saved(
        &a,
        &uid(14),
        dto::SavedWrite {
            operation: intent(21),
            base_revision: 0,
            saved: true,
        },
    )
    .await
    .unwrap();
    let second = svc.saved_works(&a, 1, Some(&token)).await.unwrap();
    assert_eq!(second.items[0].saved.work_id, uid(11));
    let third = svc
        .saved_works(&a, 1, second.next_cursor.as_deref())
        .await
        .unwrap();
    assert_eq!(third.items[0].saved.work_id, uid(10));
    assert!(third.next_cursor.is_none());
    assert_eq!(
        service(&repo).saved_works(&a, 1, Some(&token)).await.err(),
        Some(LibraryError::InvalidCursor)
    );
}

proptest::proptest! {
    #[test]
    fn saved_protocol_schedule_matches_independent_desire_revision_oracle(events in proptest::collection::vec((proptest::bool::ANY,proptest::bool::ANY,proptest::bool::ANY,proptest::bool::ANY),1..65)) {
        let rt=tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let repo=MemoryRepository::default(); let svc=service(&repo); let a=principal("alice");
            let (mut revision,mut desired)=(0_u64, false);
            for (index,(want,stale,device,rollback)) in events.into_iter().enumerate() {
                let n=index as u64+1; let mut operation=intent(n); operation.device_id=uid(if device {90}else{91});
                let base=if stale {revision.saturating_sub(1)}else{revision};
                let command=dto::SavedWrite {operation,base_revision:base,saved:want};
                repo.0.lock().unwrap().fail_before_commit=rollback;
                let result=svc.set_saved(&a,&uid(1),command.clone()).await;
                if rollback { assert_eq!(result,Err(LibraryError::Unavailable)); }
                else {
                    let outcome=if want==desired {dto::WriteOutcome::AlreadyCurrent} else if base!=revision {dto::WriteOutcome::RevisionConflict}else{desired=want;revision+=1;dto::WriteOutcome::Accepted};
                    let receipt=result.unwrap(); assert_eq!(receipt.outcome,outcome);
                    assert_eq!(svc.set_saved(&a,&uid(1),command).await,Ok(receipt));
                }
                let current=svc.saved(&a,&uid(1)).await.unwrap(); assert_eq!((current.revision,current.saved),(revision,desired));
                assert_eq!(svc.saved(&principal("bob"),&uid(1)).await.unwrap().revision,0);
                repo.0.lock().unwrap().fail_before_commit=false;
            }
        });
    }
}

mod bookmarks {
    //! Bookmark rows B1-B7 of the sync reference over the same transactional-memory double:
    //! `mock-integration-tested` at most, never account-sync acceptance.
    use super::*;
    fn bm(n: u64) -> String {
        uid(0xb00 + n)
    }
    fn create(id: u64, op: u64, seq: u64, pos: u64) -> dto::BookmarkWrite {
        dto::BookmarkWrite {
            operation: dto::Operation {
                operation_id: uid(0x400 + op),
                device_id: uid(90),
                device_sequence: seq,
                device_reported_at_ms: None,
            },
            bookmark_id: bm(id),
            audio: audio(),
            position_ms: pos,
        }
    }
    fn delete(op: u64, seq: u64) -> dto::BookmarkDelete {
        dto::BookmarkDelete {
            operation: dto::Operation {
                operation_id: uid(0x400 + op),
                device_id: uid(90),
                device_sequence: seq,
                device_reported_at_ms: None,
            },
        }
    }
    fn current(r: &dto::Receipt) -> &dto::BookmarkRecord {
        match &r.state {
            dto::MutationState::Bookmark { current } => current,
            other => panic!("{other:?}"),
        }
    }
    #[tokio::test]
    async fn create_is_idempotent_by_operation_and_by_identical_value_and_never_duplicates() {
        let repo = MemoryRepository::default();
        let (svc, a) = (service(&repo), principal("alice"));
        let first = svc
            .create_bookmark(&a, &bm(1), create(1, 1, 1, 15000))
            .await
            .unwrap();
        assert_eq!(first.outcome, dto::WriteOutcome::Accepted);
        assert_eq!(current(&first).position_ms, 15000);
        for _ in 0..5 {
            assert_eq!(
                svc.create_bookmark(&a, &bm(1), create(1, 1, 1, 15000))
                    .await,
                Ok(first.clone()),
                "B2: the same operation replays its receipt"
            );
        }
        let again = svc
            .create_bookmark(&a, &bm(1), create(1, 2, 2, 15000))
            .await
            .unwrap();
        assert_eq!(again.outcome, dto::WriteOutcome::AlreadyCurrent);
        assert_eq!(current(&again), current(&first));
        let page = svc.bookmarks(&a, 20, None).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].bookmark, *current(&first));
    }
    #[tokio::test]
    async fn one_id_names_one_immutable_value_and_the_path_must_name_the_body() {
        let repo = MemoryRepository::default();
        let (svc, a) = (service(&repo), principal("alice"));
        svc.create_bookmark(&a, &bm(1), create(1, 1, 1, 15000))
            .await
            .unwrap();
        assert_eq!(
            svc.create_bookmark(&a, &bm(1), create(1, 2, 2, 16000))
                .await,
            Err(LibraryError::OperationReused),
            "a different value under a used ID is never silently rewritten"
        );
        assert_eq!(
            svc.create_bookmark(&a, &bm(2), create(1, 3, 3, 15000))
                .await,
            Err(LibraryError::InvalidRequest),
            "path and body IDs must agree"
        );
        assert_eq!(svc.bookmarks(&a, 20, None).await.unwrap().items.len(), 1);
    }
    #[tokio::test]
    async fn position_is_bounded_by_the_verified_duration_and_malformed_ids_are_refused() {
        let repo = MemoryRepository::default();
        let (svc, a) = (service(&repo), principal("alice"));
        assert_eq!(
            svc.create_bookmark(&a, &bm(1), create(1, 1, 1, 60_001))
                .await,
            Err(LibraryError::InvalidPosition)
        );
        let end = svc
            .create_bookmark(&a, &bm(1), create(1, 2, 1, 60_000))
            .await
            .unwrap();
        assert_eq!(end.outcome, dto::WriteOutcome::Accepted);
        let start = svc
            .create_bookmark(&a, &bm(2), create(2, 3, 1, 0))
            .await
            .unwrap();
        assert_eq!(start.outcome, dto::WriteOutcome::Accepted);
        let mut bad = create(3, 4, 1, 1);
        bad.bookmark_id = "NOT-A-UUID".into();
        assert_eq!(
            svc.create_bookmark(&a, "NOT-A-UUID", bad).await,
            Err(LibraryError::InvalidRequest)
        );
    }
    #[tokio::test]
    async fn tombstones_are_final_for_replays_restores_and_repeated_deletes() {
        let repo = MemoryRepository::default();
        let (svc, a) = (service(&repo), principal("alice"));
        let made = svc
            .create_bookmark(&a, &bm(1), create(1, 1, 1, 15000))
            .await
            .unwrap();
        let gone = svc.delete_bookmark(&a, &bm(1), delete(2, 2)).await.unwrap();
        assert_eq!(gone.outcome, dto::WriteOutcome::Accepted);
        assert!(current(&gone).deleted);
        assert!(current(&gone).listener_order > current(&made).listener_order);
        assert_eq!(current(&gone).audio, current(&made).audio);
        assert!(svc.bookmarks(&a, 20, None).await.unwrap().items.is_empty());
        // B4: the old outbox replay returns the historical receipt and creates nothing.
        assert_eq!(
            svc.create_bookmark(&a, &bm(1), create(1, 1, 1, 15000))
                .await,
            Ok(made.clone())
        );
        assert!(svc.bookmarks(&a, 20, None).await.unwrap().items.is_empty());
        // B5: a local restore with a new operation is refused, naming the tombstone.
        let refused = svc
            .create_bookmark(&a, &bm(1), create(1, 3, 3, 15000))
            .await
            .unwrap();
        assert_eq!(refused.outcome, dto::WriteOutcome::BookmarkDeleted);
        assert_eq!(refused.resolution, Some(dto::Resolution::KeepServer));
        assert_eq!(current(&refused), current(&gone));
        // B7: deleting again changes nothing.
        let repeat = svc.delete_bookmark(&a, &bm(1), delete(4, 4)).await.unwrap();
        assert_eq!(repeat.outcome, dto::WriteOutcome::AlreadyCurrent);
        assert_eq!(current(&repeat), current(&gone));
        assert!(svc.bookmarks(&a, 20, None).await.unwrap().items.is_empty());
    }
    #[tokio::test]
    async fn deleting_an_id_the_account_never_created_is_not_found_and_leaves_no_effect() {
        let repo = MemoryRepository::default();
        let (svc, a) = (service(&repo), principal("alice"));
        assert_eq!(
            svc.delete_bookmark(&a, &bm(9), delete(1, 1)).await,
            Err(LibraryError::NotFound)
        );
        assert_eq!(
            svc.receipt(&a, &uid(0x401)).await,
            Err(LibraryError::NotFound),
            "a 404 writes no receipt"
        );
        let made = svc
            .create_bookmark(&a, &bm(9), create(9, 2, 1, 5))
            .await
            .unwrap();
        assert_eq!(made.outcome, dto::WriteOutcome::Accepted);
    }
    #[tokio::test]
    async fn bookmarks_are_account_scoped_and_one_actor_cannot_read_or_delete_anothers() {
        let repo = MemoryRepository::default();
        let (svc, alice, bob) = (service(&repo), principal("alice"), principal("bob"));
        let made = svc
            .create_bookmark(&alice, &bm(1), create(1, 1, 1, 15000))
            .await
            .unwrap();
        assert!(svc
            .bookmarks(&bob, 20, None)
            .await
            .unwrap()
            .items
            .is_empty());
        assert_eq!(
            svc.delete_bookmark(&bob, &bm(1), delete(2, 1)).await,
            Err(LibraryError::NotFound)
        );
        assert_eq!(
            svc.receipt(&bob, &uid(0x401)).await,
            Err(LibraryError::NotFound)
        );
        assert_eq!(
            svc.bookmarks(&alice, 20, None).await.unwrap().items.len(),
            1
        );
        // The same client-minted ID is independent per account.
        let theirs = svc
            .create_bookmark(&bob, &bm(1), create(1, 3, 1, 7))
            .await
            .unwrap();
        assert_eq!(theirs.outcome, dto::WriteOutcome::Accepted);
        assert_eq!(current(&theirs).position_ms, 7);
        assert_eq!(
            svc.bookmarks(&alice, 20, None).await.unwrap().items[0].bookmark,
            *current(&made)
        );
    }
    #[tokio::test]
    async fn pages_are_newest_first_bounded_and_cursors_bind_account_and_route() {
        let repo = MemoryRepository::default();
        let (svc, a) = (service(&repo), principal("alice"));
        for n in 1..=5 {
            svc.create_bookmark(&a, &bm(n), create(n, n, 1, n * 1000))
                .await
                .unwrap();
        }
        svc.delete_bookmark(&a, &bm(3), delete(30, 2))
            .await
            .unwrap();
        let (mut seen, mut cursor) = (Vec::new(), None);
        loop {
            let page = svc.bookmarks(&a, 2, cursor.as_deref()).await.unwrap();
            assert!(page.items.len() <= 2);
            seen.extend(page.items.iter().map(|i| i.bookmark.bookmark_id.clone()));
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(
            seen,
            vec![bm(5), bm(4), bm(2), bm(1)],
            "live only, descending acceptance"
        );
        let first = svc.bookmarks(&a, 2, None).await.unwrap();
        let token = first.next_cursor.as_deref();
        assert_eq!(
            svc.bookmarks(&principal("bob"), 2, token).await,
            Err(LibraryError::InvalidCursor)
        );
        assert_eq!(
            svc.saved_works(&a, 2, token).await,
            Err(LibraryError::InvalidCursor)
        );
        assert_eq!(
            svc.bookmarks(&a, 3, token).await,
            Err(LibraryError::InvalidCursor)
        );
        assert_eq!(
            svc.bookmarks(&a, 0, None).await,
            Err(LibraryError::InvalidRequest)
        );
        assert_eq!(
            svc.bookmarks(&a, 51, None).await,
            Err(LibraryError::InvalidRequest)
        );
    }
    #[tokio::test]
    async fn saved_progress_history_and_clear_never_create_or_delete_a_bookmark() {
        let repo = MemoryRepository::default();
        let (svc, a) = (service(&repo), principal("alice"));
        let made = svc
            .create_bookmark(&a, &bm(1), create(1, 1, 1, 15000))
            .await
            .unwrap();
        svc.put_progress(
            &a,
            &uid(3),
            write(2, 0, 20000, dto::ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
        for (n, saved, base) in [(3, true, 0), (4, false, 1)] {
            svc.set_saved(
                &a,
                &uid(1),
                dto::SavedWrite {
                    operation: intent(n),
                    base_revision: base,
                    saved,
                },
            )
            .await
            .unwrap();
        }
        svc.clear_history(
            &a,
            dto::HistoryClear {
                operation: intent(5),
                base_revision: 0,
                history_epoch: 0,
                confirmation: dto::ClearConfirmation::ClearHistory,
            },
        )
        .await
        .unwrap();
        let page = svc.bookmarks(&a, 20, None).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].bookmark, *current(&made));
        assert_eq!(
            svc.progress(&a, &uid(3))
                .await
                .unwrap()
                .unwrap()
                .position_ms,
            20000
        );
        // ... and the converse: deleting a bookmark leaves Saved and progress alone.
        svc.delete_bookmark(&a, &bm(1), delete(6, 2)).await.unwrap();
        assert_eq!(
            svc.progress(&a, &uid(3))
                .await
                .unwrap()
                .unwrap()
                .position_ms,
            20000
        );
    }
    #[tokio::test]
    async fn withdrawn_audio_keeps_the_bookmark_unavailable_and_blocks_only_new_ones() {
        let repo = MemoryRepository::default();
        let (svc, a) = (service(&repo), principal("alice"));
        svc.create_bookmark(&a, &bm(1), create(1, 1, 1, 15000))
            .await
            .unwrap();
        repo.0.lock().unwrap().withdrawn = true;
        let page = svc.bookmarks(&a, 20, None).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].resume, dto::ResumeStatus::AccessUnavailable);
        assert_eq!(page.items[0].presentation, None);
        assert_eq!(
            page.items[0].bookmark.audio,
            audio(),
            "no remap to another release"
        );
        assert_eq!(
            svc.create_bookmark(&a, &bm(2), create(2, 2, 1, 1)).await,
            Err(LibraryError::NotFound)
        );
        assert_eq!(
            svc.create_bookmark(&a, &bm(1), create(1, 3, 2, 15000))
                .await
                .unwrap()
                .outcome,
            dto::WriteOutcome::AlreadyCurrent,
            "an owned value is decided from its record, not from current access"
        );
    }
    #[tokio::test]
    async fn uncertain_precommit_failure_commits_neither_effect_nor_receipt_and_retry_is_the_same_intent(
    ) {
        let repo = MemoryRepository::default();
        let (svc, a) = (service(&repo), principal("alice"));
        repo.0.lock().unwrap().fail_before_commit = true;
        let request = create(1, 1, 1, 15000);
        assert_eq!(
            svc.create_bookmark(&a, &bm(1), request.clone()).await,
            Err(LibraryError::Unavailable)
        );
        assert!(svc.bookmarks(&a, 20, None).await.unwrap().items.is_empty());
        assert_eq!(
            svc.receipt(&a, &request.operation.operation_id).await,
            Err(LibraryError::NotFound)
        );
        repo.0.lock().unwrap().fail_before_commit = false;
        assert_eq!(
            svc.create_bookmark(&a, &bm(1), request.clone())
                .await
                .unwrap()
                .outcome,
            dto::WriteOutcome::Accepted
        );
        // Lost response: the receipt is readable and equals the original answer.
        let receipt = svc
            .receipt(&a, &request.operation.operation_id)
            .await
            .unwrap();
        assert_eq!(receipt.outcome, dto::WriteOutcome::Accepted);
        assert_eq!(svc.bookmarks(&a, 20, None).await.unwrap().items.len(), 1);
    }
    #[tokio::test]
    async fn changed_body_under_one_operation_and_a_reused_sequence_are_exact_rejections() {
        let repo = MemoryRepository::default();
        let (svc, a) = (service(&repo), principal("alice"));
        svc.create_bookmark(&a, &bm(1), create(1, 1, 1, 15000))
            .await
            .unwrap();
        assert_eq!(
            svc.create_bookmark(&a, &bm(1), create(1, 1, 1, 16000))
                .await,
            Err(LibraryError::OperationReused)
        );
        assert_eq!(
            svc.create_bookmark(&a, &bm(1), create(1, 9, 1, 15000))
                .await,
            Err(LibraryError::StaleSequence)
        );
        assert_eq!(svc.bookmarks(&a, 20, None).await.unwrap().items.len(), 1);
    }
}
