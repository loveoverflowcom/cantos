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
                    anchor,
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
                                .map(|(_, work)| dto::SavedItem {
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
                                    .map(|(_, work)| dto::ContinueItem {
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
                        PageKind::History => PageData::History {
                            state: state
                                .history_state
                                .unwrap_or(HistoryState::restore(0, 0).unwrap())
                                .raw(),
                            items: keys
                                .iter()
                                .map(|(order, work)| {
                                    let (audio, _) = &state.history[work];
                                    dto::HistoryItem {
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
