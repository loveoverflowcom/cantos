use super::{
    mock::{MockPort, Scenario},
    model::*,
};

fn request(model: &mut Model, intent: Intent) -> Request {
    model.reduce(intent).unwrap().unwrap()
}
fn settle(model: &mut Model, intent: Intent, scenario: Scenario) -> Request {
    let request = request(model, intent);
    model
        .complete(MockPort::new(scenario).complete(&request))
        .unwrap();
    request
}
fn selected(model: &mut Model) -> u64 {
    settle(model, Intent::Generate, Scenario::Success);
    let take = model.snapshot().takes.last().unwrap().id;
    model.reduce(Intent::SelectTake(take)).unwrap();
    take
}
fn rejects(model: &mut Model, intent: Intent, expected: Rejection) {
    let before = model.clone();
    assert_eq!(model.reduce(intent), Err(expected));
    assert_eq!(*model, before, "rejection must be atomic");
}

#[test]
fn save_appends_revision_preserving_old_bytes_and_each_drafts_buffer() {
    let mut model = Model::new();
    let old = model.snapshot().current.unwrap().revisions[0].clone();
    let text = "Người dẫn chuyện: Ngày mai, mình vẫn diễn tiếp. Tiếng Việt còn nguyên dấu.";
    model.reduce(Intent::Edit(text.into())).unwrap();
    model.reduce(Intent::Open(2)).unwrap();
    model
        .reduce(Intent::Edit("Lá thư thứ hai, chưa lưu.".into()))
        .unwrap();
    model.reduce(Intent::Open(1)).unwrap();
    assert_eq!(model.snapshot().buffer, text);
    model.reduce(Intent::Save).unwrap();
    let snapshot = model.snapshot();
    assert!(!snapshot.dirty);
    assert_eq!(
        snapshot.current.unwrap().revisions,
        vec![
            old,
            Revision {
                number: 2,
                text: text.into()
            }
        ]
    );
    model.reduce(Intent::Open(2)).unwrap();
    assert_eq!(model.snapshot().buffer, "Lá thư thứ hai, chưa lưu.");
    assert!(model.snapshot().dirty);
}

#[test]
fn pending_request_rejects_duplicate_generate_and_edit_without_dispatch_or_text_loss() {
    let mut model = Model::new();
    let pending = request(&mut model, Intent::Generate);
    assert_eq!(model.snapshot().pending, Some(pending.clone()));
    rejects(&mut model, Intent::Generate, Rejection::Busy);
    rejects(
        &mut model,
        Intent::Edit("Không được thay snapshot đang chạy".into()),
        Rejection::Busy,
    );
    let completion = MockPort::new(Scenario::Success).complete(&pending);
    model.complete(completion.clone()).unwrap();
    let before = model.clone();
    assert_eq!(model.complete(completion), Err(Rejection::StaleCompletion));
    assert_eq!(model, before);
    assert_eq!(model.snapshot().takes.len(), 1);
    assert_eq!(
        model.snapshot().selected_take,
        None,
        "retention never selects"
    );
}

#[test]
fn generated_storage_failure_is_distinct_and_cannot_be_selected_or_synced() {
    let mut model = Model::new();
    settle(&mut model, Intent::Generate, Scenario::StorageFailure);
    let snapshot = model.snapshot();
    let take = &snapshot.takes[0];
    assert_eq!(take.execution, ExecutionState::Generated);
    assert_eq!(take.storage, StorageState::Failed);
    assert!(matches!(snapshot.run, RunState::StorageFailed { .. }));
    rejects(
        &mut model,
        Intent::SelectTake(take.id),
        Rejection::NotRetained,
    );
    rejects(&mut model, Intent::PreviewSync, Rejection::NoTake);
    settle(&mut model, Intent::Generate, Scenario::Failure);
    assert!(matches!(model.snapshot().run, RunState::Failed { .. }));
    assert_eq!(model.snapshot().takes.len(), 1);
    assert!(
        model.snapshot().can_generate,
        "explicit new decision remains available after known failure"
    );
}

#[test]
fn unknown_generation_reconciles_original_attempt_and_input_even_after_new_revision() {
    let mut model = Model::new();
    let original = settle(&mut model, Intent::Generate, Scenario::Unknown);
    rejects(&mut model, Intent::Generate, Rejection::Unresolved);
    model
        .reduce(Intent::Edit(
            "Le nouveau texte — văn bản mới không tạo audio tự động.".into(),
        ))
        .unwrap();
    model.reduce(Intent::Save).unwrap();
    let reconciliation = request(&mut model, Intent::Reconcile);
    assert_eq!(
        reconciliation.kind,
        RequestKind::ReconcileRun {
            attempt: original.ticket.sequence,
            revision: 1
        }
    );
    assert_ne!(reconciliation.ticket, original.ticket);
    model
        .complete(MockPort::new(Scenario::Success).complete(&reconciliation))
        .unwrap();
    let take = model.snapshot().takes[0].clone();
    assert_eq!(take.revision, 1);
    assert!(take.stale);
    rejects(&mut model, Intent::SelectTake(take.id), Rejection::Stale);
}

#[test]
fn saved_edit_keeps_selected_take_but_blocks_stale_sync_and_invalidates_preview() {
    let mut model = Model::new();
    let take = selected(&mut model);
    model.reduce(Intent::PreviewSync).unwrap();
    assert_eq!(
        model.snapshot().sync,
        SyncState::Preview(SyncPlan {
            draft: 1,
            revision: 1,
            take
        })
    );
    model
        .reduce(Intent::Edit(
            "Một câu mới, vẫn cần được đọc và duyệt riêng.".into(),
        ))
        .unwrap();
    rejects(&mut model, Intent::ConfirmSync, Rejection::Unsaved);
    model.reduce(Intent::Save).unwrap();
    let snapshot = model.snapshot();
    assert_eq!(snapshot.selected_take, Some(take));
    assert_eq!(snapshot.sync, SyncState::Idle);
    assert!(snapshot.takes[0].stale);
    assert!(!snapshot.can_sync);
    rejects(&mut model, Intent::PreviewSync, Rejection::Stale);
    rejects(&mut model, Intent::ConfirmSync, Rejection::MissingPlan);
}

#[test]
fn sync_requires_separate_plan_and_confirm_partial_and_unknown_reconcile_same_operation() {
    for outcome in [Scenario::Partial, Scenario::Unknown] {
        let mut model = Model::new();
        let take = selected(&mut model);
        rejects(&mut model, Intent::ConfirmSync, Rejection::MissingPlan);
        model.reduce(Intent::PreviewSync).unwrap();
        assert!(
            model.snapshot().pending.is_none(),
            "preview creates no request"
        );
        model.reduce(Intent::CancelSync).unwrap();
        assert_eq!(model.snapshot().sync, SyncState::Idle);
        model.reduce(Intent::PreviewSync).unwrap();
        let original = settle(&mut model, Intent::ConfirmSync, outcome);
        let plan = SyncPlan {
            draft: 1,
            revision: 1,
            take,
        };
        assert_eq!(
            model.snapshot().sync,
            if outcome == Scenario::Partial {
                SyncState::Partial {
                    operation: original.ticket.sequence,
                    plan: plan.clone(),
                }
            } else {
                SyncState::Unknown {
                    operation: original.ticket.sequence,
                    plan: plan.clone(),
                }
            }
        );
        rejects(&mut model, Intent::PreviewSync, Rejection::Unresolved);
        let reconciliation = request(&mut model, Intent::Reconcile);
        assert_eq!(
            reconciliation.kind,
            RequestKind::ReconcileSync {
                operation: original.ticket.sequence,
                plan: plan.clone()
            }
        );
        model
            .complete(MockPort::new(Scenario::Success).complete(&reconciliation))
            .unwrap();
        assert_eq!(
            model.snapshot().sync,
            SyncState::Receipt {
                operation: original.ticket.sequence,
                plan
            }
        );
        assert_eq!(model.snapshot().selected_take, Some(take));
        assert_eq!(
            model.snapshot().takes.len(),
            1,
            "sync receipt is not another take"
        );
    }
}

#[test]
fn a_to_b_to_a_reset_and_dispose_reject_old_callbacks_atomically() {
    let mut model = Model::new();
    let old = request(&mut model, Intent::Generate);
    let completion = MockPort::new(Scenario::Success).complete(&old);
    model.reduce(Intent::Open(2)).unwrap();
    model.reduce(Intent::Open(1)).unwrap();
    assert!(matches!(model.snapshot().run, RunState::Uncertain { .. }));
    let before = model.clone();
    assert_eq!(
        model.complete(completion.clone()),
        Err(Rejection::StaleCompletion)
    );
    assert_eq!(model, before);
    let reconcile = request(&mut model, Intent::Reconcile);
    assert_ne!(reconcile.ticket.epoch, old.ticket.epoch);
    assert_eq!(model.complete(completion), Err(Rejection::StaleCompletion));
    model.reset(LibraryState::Ready);
    assert_eq!(
        model.complete(MockPort::new(Scenario::Success).complete(&reconcile)),
        Err(Rejection::StaleCompletion)
    );
    let pending = request(&mut model, Intent::Generate);
    model.dispose();
    let before = model.clone();
    assert_eq!(
        model.complete(MockPort::new(Scenario::Success).complete(&pending)),
        Err(Rejection::Disposed)
    );
    assert_eq!(model, before);
    rejects(&mut model, Intent::NewDraft, Rejection::Disposed);
}

#[test]
fn malformed_completion_never_changes_pending_owner_or_partial_state() {
    let mut model = Model::new();
    let request = request(&mut model, Intent::Generate);
    let before = model.clone();
    assert_eq!(
        model.complete(Completion {
            request: request.clone(),
            outcome: Outcome::Synced(SyncOutcome::Receipt)
        }),
        Err(Rejection::WrongCompletion)
    );
    assert_eq!(model, before);
    let mut forged = request.clone();
    forged.kind = RequestKind::Generate {
        attempt: 900,
        revision: 77,
    };
    assert_eq!(
        model.complete(Completion {
            request: forged,
            outcome: Outcome::Generated(GenerationOutcome::Retained)
        }),
        Err(Rejection::StaleCompletion)
    );
    assert_eq!(model, before);
    model
        .complete(MockPort::new(Scenario::Success).complete(&request))
        .unwrap();
}

#[test]
fn library_connection_and_utf8_bounds_are_explicit_and_rejections_preserve_state() {
    let mut model = Model::new();
    for state in [
        LibraryState::Loading,
        LibraryState::Error,
        LibraryState::Locked,
    ] {
        model.reset(state);
        assert!(model.snapshot().drafts.is_empty());
        rejects(&mut model, Intent::NewDraft, Rejection::LibraryUnavailable);
    }
    model.reset(LibraryState::Empty);
    model.reduce(Intent::NewDraft).unwrap();
    assert_eq!(model.snapshot().library, LibraryState::Ready);
    assert_eq!(model.snapshot().drafts.len(), 1);
    rejects(&mut model, Intent::Generate, Rejection::EmptyText);
    model
        .reduce(Intent::Edit("á".repeat(MAX_TEXT_BYTES / 2)))
        .unwrap();
    rejects(
        &mut model,
        Intent::Edit("á".repeat(MAX_TEXT_BYTES / 2 + 1)),
        Rejection::TextTooLong,
    );
    model.reduce(Intent::Save).unwrap();
    model
        .reduce(Intent::SetConnection(ConnectionState::Unavailable))
        .unwrap();
    rejects(
        &mut model,
        Intent::Generate,
        Rejection::ConnectionUnavailable,
    );
    model
        .reduce(Intent::SetConnection(ConnectionState::Connected))
        .unwrap();
    assert!(model.snapshot().can_generate);
    rejects(&mut model, Intent::Open(999), Rejection::UnknownDraft);
}
