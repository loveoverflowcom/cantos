//! Pure Studio save state; a transport failure preserves both text and retry identity.
use cantos_contracts::{ApiError, ErrorCode, FieldIssue, RevisionResponse, SaveRevisionRequest};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Dirty,
    Loading,
    Saving,
    Loaded,
    Saved,
    DraftPreserved,
    SignedIn,
    Unauthenticated,
    NotFound,
    Forbidden,
    InvalidRequest,
    InvalidScript,
    EvidenceUnavailable,
    StaleRevision,
    OperationReused,
    ProposalAlreadySubmitted,
    Unavailable,
    CorruptRevision,
    StaleProductionInputs,
    ProductionBlocked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Intent {
    pub actor: String,
    pub script: String,
    pub request: SaveRevisionRequest,
}

#[derive(Clone, Debug)]
pub struct Editor {
    pub actor: String,
    pub script: String,
    pub draft: String,
    /// Local bytes acknowledged by the last save, before server normalization.
    pub baseline: String,
    pub draft_actor: String,
    pub base: u64,
    pub pending: Option<Intent>,
    pub ambiguous: bool,
    pub busy: bool,
    pub status: Status,
    pub remote: Option<RevisionResponse>,
    pub ticket: u64,
    pub read_actor: String,
    pub read_script: String,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            actor: String::new(),
            script: String::new(),
            draft: String::new(),
            baseline: String::new(),
            draft_actor: String::new(),
            base: 0,
            pending: None,
            ambiguous: false,
            busy: false,
            status: Status::Idle,
            remote: None,
            ticket: 0,
            read_actor: String::new(),
            read_script: String::new(),
        }
    }
}

impl Editor {
    pub fn is_dirty(&self) -> bool {
        self.draft != self.baseline
    }

    /// The shell also checks IME composition before navigation or browser unload.
    pub fn has_unsaved(&self) -> bool {
        self.is_dirty() || self.pending.is_some() || self.busy
    }

    pub fn can_edit(&self) -> bool {
        self.draft_actor.is_empty() || self.draft_actor == self.actor
    }

    pub fn can_save(&self) -> bool {
        !self.busy
            && self.pending.is_none()
            && !self.actor.is_empty()
            && !self.script.is_empty()
            && self.can_edit()
            && self.is_dirty()
    }

    /// Session changes preserve the local author's bytes and unresolved save identity.
    pub fn signed_in(&self, actor: String) -> Self {
        let mut next = self.clone();
        if next.actor != actor {
            next.ticket = next.ticket.saturating_add(1);
            next.read_actor.clear();
            next.read_script.clear();
            next.remote = None;
        }
        next.actor = actor;
        if next.draft_actor.is_empty() && !next.draft.is_empty() {
            next.draft_actor = next.actor.clone();
        }
        next.busy = false;
        next.status = if !next.can_edit()
            || next
                .pending
                .as_ref()
                .is_some_and(|intent| intent.actor != next.actor)
        {
            Status::Forbidden
        } else {
            Status::SignedIn
        };
        next
    }

    pub fn select_script(&self, script: String) -> Self {
        if self.has_unsaved() || self.script == script {
            return self.clone();
        }
        self.discard_and_select(script)
    }

    /// Only an explicit user discard may replace a dirty document. Pending saves must reconcile.
    pub fn discard_and_select(&self, script: String) -> Self {
        if self.busy || self.pending.is_some() {
            return self.clone();
        }
        Self {
            actor: self.actor.clone(),
            script,
            ticket: self.ticket.saturating_add(1),
            ..Self::default()
        }
    }

    pub fn edit(&self, text: String) -> Self {
        if !self.can_edit() || text == self.draft {
            return self.clone();
        }
        let mut next = self.clone();
        next.draft = text;
        if next.draft_actor.is_empty() {
            next.draft_actor = self.actor.clone();
        }
        if !self.busy
            && self.pending.is_none()
            && matches!(
                self.status,
                Status::Idle
                    | Status::SignedIn
                    | Status::Loaded
                    | Status::Saved
                    | Status::DraftPreserved
                    | Status::Dirty
            )
        {
            next.status = if next.is_dirty() {
                Status::Dirty
            } else if next.base == 0 {
                Status::Idle
            } else {
                Status::Saved
            };
        }
        next
    }
    /// A new intent cannot abandon an ambiguous save. Reconcile the pending snapshot first.
    pub fn start_save(&self, operation: String) -> Option<(Self, Intent)> {
        if !self.can_save() {
            return None;
        }
        let intent = Intent {
            actor: self.actor.clone(),
            script: self.script.clone(),
            request: SaveRevisionRequest {
                expected_revision: self.base,
                operation_id: operation,
                script_json: self.draft.clone(),
            },
        };
        let mut next = self.clone();
        next.busy = true;
        next.status = Status::Saving;
        next.pending = Some(intent.clone());
        next.draft_actor = intent.actor.clone();
        next.ambiguous = false;
        Some((next, intent))
    }

    pub fn retry(&self) -> Option<(Self, Intent)> {
        if self.busy {
            return None;
        }
        let intent = self.pending.clone()?;
        if intent.actor != self.actor || intent.script != self.script {
            return None;
        }
        let mut next = self.clone();
        next.busy = true;
        next.status = Status::Saving;
        Some((next, intent))
    }

    pub fn saved(&self, intent: &Intent, response: RevisionResponse) -> Self {
        if self.pending.as_ref() != Some(intent)
            || self.script != intent.script
            || self.script != response.script_id
            || self.actor != intent.actor
        {
            return self.clone();
        }
        let mut next = self.clone();
        if response.accepted_by != intent.actor {
            // A different tab may have replaced the session cookie during this request.
            next.busy = false;
            next.ambiguous = true;
            next.actor.clear();
            next.status = Status::Unauthenticated;
            return next;
        }
        next.base = response.revision;
        next.baseline = intent.request.script_json.clone();
        next.remote = Some(response);
        next.pending = None;
        next.ambiguous = false;
        next.busy = false;
        // Never rewrite the textarea on acknowledgement (including while IME composing).
        next.status = if self.draft == intent.request.script_json {
            Status::Saved
        } else {
            Status::DraftPreserved
        };
        next
    }

    pub fn failed(&self, error: &ApiError) -> Self {
        let mut next = self.clone();
        next.busy = false;
        next.status = error_status(&error.code);
        // A later authorization denial says nothing about an earlier ambiguous commit.
        if matches!(
            error.code,
            ErrorCode::Unavailable | ErrorCode::CorruptRevision
        ) {
            next.ambiguous = next.pending.is_some();
        } else if self.ambiguous
            && matches!(
                error.code,
                ErrorCode::Unauthenticated | ErrorCode::Forbidden | ErrorCode::NotFound
            )
        {
            // Reauthenticate as the submitting actor, then reconcile the original operation.
        } else {
            next.pending = None;
            next.ambiguous = false;
        }
        next
    }

    pub fn save_failed(&self, intent: &Intent, error: &ApiError) -> Self {
        if self.pending.as_ref() != Some(intent)
            || self.actor != intent.actor
            || self.script != intent.script
        {
            return self.clone();
        }
        self.failed(error)
    }

    pub fn session_failed(&self, error: &ApiError) -> Self {
        let mut next = self.clone();
        next.busy = false;
        next.status = error_status(&error.code);
        next
    }

    pub fn start_read(&self) -> Option<Self> {
        if self.busy
            || self.pending.is_some()
            || self.actor.is_empty()
            || self.script.is_empty()
            || !self.can_edit()
        {
            return None;
        }
        let mut next = self.clone();
        next.busy = true;
        next.status = Status::Loading;
        next.ticket = self.ticket.checked_add(1)?;
        next.read_actor = self.actor.clone();
        next.read_script = self.script.clone();
        Some(next)
    }

    pub fn loaded(&self, ticket: u64, response: RevisionResponse) -> Self {
        self.loaded_with_buffer(ticket, response, false)
    }

    /// Native text may be newer than the reducer until blur/composition commits it.
    /// The shell keeps its buffer/navigation guard active until those bytes are committed.
    pub fn loaded_with_buffer(
        &self,
        ticket: u64,
        response: RevisionResponse,
        buffer_pending: bool,
    ) -> Self {
        if !self.accepts_read(ticket) || response.script_id != self.script {
            return self.clone();
        }
        let mut next = self.clone();
        next.busy = false;
        next.read_actor.clear();
        next.read_script.clear();
        if !self.is_dirty() && !buffer_pending {
            next.draft = response.script_json.clone();
            next.baseline = response.script_json.clone();
            next.draft_actor = self.actor.clone();
            next.base = response.revision;
            next.status = Status::Loaded;
        } else {
            next.status = Status::DraftPreserved;
        }
        next.remote = Some(response);
        next
    }

    fn accepts_read(&self, ticket: u64) -> bool {
        self.busy
            && self.status == Status::Loading
            && ticket == self.ticket
            && self.read_actor == self.actor
            && self.read_script == self.script
    }

    pub fn read_failed(&self, ticket: u64, error: &ApiError) -> Self {
        if !self.accepts_read(ticket) {
            return self.clone();
        }
        let mut next = self.failed(error);
        next.read_actor.clear();
        next.read_script.clear();
        next
    }

    /// Explicit history/discard action. The shell confirms any dirty draft before calling this.
    pub fn open_revision(&self, response: RevisionResponse) -> Self {
        if self.busy || self.pending.is_some() || self.actor.is_empty() {
            return self.clone();
        }
        Self {
            actor: self.actor.clone(),
            script: response.script_id.clone(),
            draft: response.script_json.clone(),
            baseline: response.script_json.clone(),
            draft_actor: self.actor.clone(),
            base: response.revision,
            status: Status::Loaded,
            remote: Some(response),
            ticket: self.ticket.saturating_add(1),
            ..Self::default()
        }
    }

    /// Explicit user reconciliation: keep local text and adopt the displayed remote head.
    pub fn rebase(&self) -> Self {
        let mut next = self.clone();
        if !self.busy && self.pending.is_none() && self.can_edit() {
            if let Some(remote) = &self.remote {
                next.base = remote.revision;
                next.status = Status::DraftPreserved;
            }
        }
        next
    }
}

/// Validation is tied to exact actor and text bytes; it never owns or rewrites a draft.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationPreview {
    pub snapshot: String,
    pub actor: String,
    pub busy: bool,
    pub ticket: u64,
    pub valid: bool,
    pub issues: Vec<FieldIssue>,
    pub error: Option<ErrorCode>,
}

impl ValidationPreview {
    pub fn start(&self, actor: String, snapshot: String) -> Self {
        let Some(ticket) = self.ticket.checked_add(1) else {
            return Self {
                actor,
                snapshot,
                ticket: self.ticket,
                error: Some(ErrorCode::Unavailable),
                ..Self::default()
            };
        };
        Self {
            snapshot,
            actor,
            busy: true,
            ticket,
            ..Self::default()
        }
    }

    pub fn finish(
        &self,
        ticket: u64,
        actor: &str,
        snapshot: &str,
        result: Result<Vec<FieldIssue>, ApiError>,
    ) -> Self {
        if !self.busy || ticket != self.ticket || actor != self.actor || snapshot != self.snapshot {
            return self.clone();
        }
        let mut next = self.clone();
        next.busy = false;
        match result {
            Ok(issues) => {
                next.valid = issues.is_empty();
                next.issues = issues;
                next.error = None;
            }
            Err(error) => {
                next.valid = false;
                next.issues = error.issues;
                next.error = Some(error.code);
            }
        }
        next
    }

    pub fn is_current(&self, actor: &str, snapshot: &str) -> bool {
        self.valid && !self.busy && self.actor == actor && self.snapshot == snapshot
    }
}

pub fn error_status(code: &ErrorCode) -> Status {
    match code {
        ErrorCode::Unauthenticated => Status::Unauthenticated,
        ErrorCode::NotFound => Status::NotFound,
        ErrorCode::Forbidden => Status::Forbidden,
        ErrorCode::InvalidRequest | ErrorCode::InvalidCatalog => Status::InvalidRequest,
        ErrorCode::InvalidScript => Status::InvalidScript,
        ErrorCode::EvidenceUnavailable => Status::EvidenceUnavailable,
        ErrorCode::StaleRevision | ErrorCode::CatalogConflict | ErrorCode::StaleCursor => {
            Status::StaleRevision
        }
        ErrorCode::OperationReused => Status::OperationReused,
        ErrorCode::ProposalAlreadySubmitted => Status::ProposalAlreadySubmitted,
        ErrorCode::Unavailable => Status::Unavailable,
        ErrorCode::CorruptRevision => Status::CorruptRevision,
        ErrorCode::StaleProductionInputs => Status::StaleProductionInputs,
        ErrorCode::ProductionBlocked => Status::ProductionBlocked,
    }
}

#[cfg(test)]
mod tests {
    use super::{Editor, Status, ValidationPreview};
    use cantos_contracts::{ApiError, ErrorCode, FieldIssue, RevisionResponse};

    #[test]
    fn typing_after_an_acknowledged_save_is_visibly_unsaved() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "original".into(),
            ..Editor::default()
        };
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let saved = saving.saved(&intent, response(1));
        assert_eq!(saved.status, Status::Saved);
        assert_eq!(saved.edit("new text".into()).status, Status::Dirty);
        assert_eq!(saved.edit("new text".into()).base, 1);
    }

    #[test]
    fn changing_a_clean_save_target_clears_the_previous_document() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "original".into(),
            ..Editor::default()
        };
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let saved = saving.saved(&intent, response(1));
        let selected = saved.select_script("other".into());
        assert_eq!(selected.script, "other");
        assert_eq!(selected.draft, "");
        assert_eq!(selected.base, 0);
        assert!(selected.remote.is_none());
        assert_eq!(selected.status, Status::Idle);
        assert_eq!(saved.select_script("script".into()).base, 1);
        assert_eq!(saving.select_script("other".into()).script, "script");
    }

    #[test]
    fn reverting_an_edit_to_the_acknowledged_snapshot_is_clean() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "Ngày mai, mình có diễn tiếp không?".into(),
            ..Editor::default()
        };
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let saved = saving.saved(&intent, response(1));
        let reverted = saved.edit("changed".into()).edit(initial.draft);
        assert_eq!(reverted.status, Status::Saved);
        assert!(reverted.start_save("duplicate".into()).is_none());
    }

    #[test]
    fn a_clean_refresh_replaces_the_previous_saved_document() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "original".into(),
            ..Editor::default()
        };
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let reading = saving.saved(&intent, response(1)).start_read().unwrap();
        let loaded = reading.loaded(reading.ticket, response(2));
        assert_eq!(loaded.draft, "canonical");
        assert_eq!(loaded.base, 2);
        assert_eq!(loaded.status, Status::Loaded);
    }

    #[test]
    fn selecting_another_script_cannot_move_an_unsaved_draft() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "my draft".into(),
            base: 3,
            ..Editor::default()
        };
        let selected = initial.select_script("other".into());
        assert_eq!(selected.script, "script");
        assert_eq!(selected.draft, "my draft");
        assert_eq!(selected.base, 3);
    }

    #[test]
    fn server_normalization_does_not_make_acknowledged_local_bytes_dirty() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "Ngo\u{31b}o\u{31b}i da\u{302}n chuye\u{323}\u{302}n".into(),
            ..Editor::default()
        };
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let saved = saving.saved(&intent, response(1));
        assert_eq!(saved.baseline, initial.draft);
        assert_eq!(saved.draft, initial.draft);
        assert_eq!(saved.remote.as_ref().unwrap().script_json, "canonical");
        assert!(!saved.is_dirty());
        assert!(!saved.has_unsaved());
        assert!(!saved.can_save());
        assert_eq!(saved.draft_actor, "alice");
    }

    #[test]
    fn explicit_discard_selects_a_new_target_without_reusing_the_old_draft() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "my draft".into(),
            draft_actor: "alice".into(),
            base: 3,
            ticket: 4,
            ..Editor::default()
        };
        let selected = initial.discard_and_select("other".into());
        assert_eq!(selected.script, "other");
        assert_eq!(selected.draft, "");
        assert_eq!(selected.baseline, "");
        assert_eq!(selected.draft_actor, "");
        assert_eq!(selected.actor, "alice");
        assert_eq!(selected.base, 0);
        assert_eq!(selected.ticket, 5);
        assert_eq!(selected.status, Status::Idle);
        assert!(!selected.has_unsaved());

        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let rejected = saving.discard_and_select("other".into());
        assert_eq!(rejected.script, "script");
        assert_eq!(rejected.draft, "my draft");
        assert_eq!(rejected.pending, Some(intent.clone()));
        let ambiguous = saving.failed(&error(ErrorCode::Unavailable));
        let rejected = ambiguous.discard_and_select("other".into());
        assert_eq!(rejected.script, "script");
        assert_eq!(rejected.pending, Some(intent));
        assert!(rejected.ambiguous);
    }

    #[test]
    fn edit_and_read_during_an_earlier_read_preserve_the_new_local_text() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            ..Editor::default()
        }
        .open_revision(response(1));
        let reading = initial.start_read().unwrap();
        let edited = reading.edit("new dialogue".into());
        let loaded = edited.loaded(reading.ticket, response(2));
        assert_eq!(loaded.draft, "new dialogue");
        assert_eq!(loaded.baseline, "canonical");
        assert_eq!(loaded.base, 1);
        assert_eq!(loaded.remote.as_ref().unwrap().revision, 2);
        assert_eq!(loaded.status, Status::DraftPreserved);
        assert!(!loaded.busy);
        assert!(loaded.has_unsaved());
    }

    #[test]
    fn a_read_cannot_replace_clean_reducer_rows_with_native_text_still_buffered() {
        let original = include_str!(
            "../../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json"
        );
        let mut initial_response = response(1);
        initial_response.script_json = original.into();
        let opened = Editor {
            actor: "alice".into(),
            ..Editor::default()
        }
        .open_revision(initial_response);
        assert!(!opened.is_dirty());
        for remove_active_row in [false, true] {
            let mut document: serde_json::Value = serde_json::from_str(original).unwrap();
            let dialogues = document["episode"]["acts"][0]["scenes"][0]["dialogues"]
                .as_array_mut()
                .unwrap();
            if remove_active_row {
                dialogues.remove(1);
            } else {
                dialogues[1]["text"] = "An: Mình sẽ diễn tiếp ở đây.".into();
            }
            let mut remote = response(2);
            remote.script_json = serde_json::to_string(&document).unwrap();
            let reading = opened.start_read().unwrap();
            let preserved = reading.loaded_with_buffer(reading.ticket, remote.clone(), true);
            assert_eq!(preserved.draft, original);
            assert_eq!(preserved.baseline, original);
            assert_eq!(preserved.base, 1);
            assert_eq!(preserved.draft_actor, "alice");
            assert_eq!(preserved.status, Status::DraftPreserved);
            assert_eq!(
                preserved.remote.as_ref().unwrap().script_json,
                remote.script_json
            );
            assert_eq!(preserved.remote.as_ref().unwrap().revision, 2);
            assert!(!preserved.busy);
            // The shell owns the actual native buffer; committing it still marks this base dirty.
            let committed = preserved.edit("An: Chữ đang nhập trong ô vẫn còn.".into());
            assert!(committed.is_dirty());
            assert_eq!(committed.base, 1);
            let replaced = reading.loaded_with_buffer(reading.ticket, remote.clone(), false);
            assert_eq!(replaced.draft, remote.script_json);
            assert_eq!(replaced.base, 2);
            assert_eq!(replaced.status, Status::Loaded);
        }
    }

    #[test]
    fn session_changes_preserve_the_original_author_and_invalidate_reads() {
        let reading = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "Alice's draft".into(),
            draft_actor: "alice".into(),
            ..Editor::default()
        }
        .start_read()
        .unwrap();
        let changed = reading.signed_in("bob".into());
        assert_eq!(changed.draft_actor, "alice");
        assert_eq!(changed.draft, "Alice's draft");
        assert_eq!(changed.status, Status::Forbidden);
        assert_eq!(changed.ticket, reading.ticket + 1);
        assert!(!changed.busy);
        assert!(!changed.can_edit());
        assert!(!changed.can_save());
        assert!(changed.start_read().is_none());
        assert_eq!(changed.edit("Bob's draft".into()).draft, "Alice's draft");
        assert!(changed.loaded(reading.ticket, response(2)).remote.is_none());
        assert_eq!(
            changed
                .read_failed(reading.ticket, &error(ErrorCode::NotFound))
                .status,
            Status::Forbidden
        );
        let restored = changed.signed_in("alice".into());
        assert_eq!(restored.draft, "Alice's draft");
        assert_eq!(restored.status, Status::SignedIn);
        assert!(restored.can_edit());
        assert!(restored.can_save());
    }

    #[test]
    fn a_late_save_response_under_another_actor_cannot_acknowledge_or_fail_the_draft() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "Alice's draft".into(),
            ..Editor::default()
        };
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let changed = saving.signed_in("bob".into());
        let late = changed.saved(&intent, response(1));
        assert_eq!(late.actor, "bob");
        assert_eq!(late.status, Status::Forbidden);
        assert_eq!(late.draft_actor, "alice");
        assert_eq!(late.pending, Some(intent.clone()));
        assert_eq!(late.base, 0);
        assert_eq!(late.baseline, "");
        let failed = changed.save_failed(&intent, &error(ErrorCode::InvalidScript));
        assert_eq!(failed.pending, Some(intent));
        assert_eq!(failed.status, Status::Forbidden);
        assert_eq!(failed.draft, "Alice's draft");
    }

    #[test]
    fn a_consumed_read_ticket_cannot_reapply_success_or_failure() {
        let reading = Editor {
            actor: "alice".into(),
            script: "script".into(),
            ..Editor::default()
        }
        .start_read()
        .unwrap();
        let loaded = reading.loaded(reading.ticket, response(1));
        let late = loaded.loaded(reading.ticket, response(2));
        assert_eq!(late.base, 1);
        assert_eq!(late.remote.as_ref().unwrap().revision, 1);
        assert_eq!(
            loaded
                .read_failed(reading.ticket, &error(ErrorCode::Unavailable))
                .status,
            Status::Loaded
        );
    }

    #[test]
    fn explicit_history_open_is_clean_and_cannot_discard_an_unresolved_save() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "newer local draft".into(),
            base: 3,
            ..Editor::default()
        };
        let opened = initial.open_revision(response(1));
        assert_eq!(opened.draft, "canonical");
        assert_eq!(opened.baseline, "canonical");
        assert_eq!(opened.draft_actor, "alice");
        assert_eq!(opened.base, 1);
        assert_eq!(opened.status, Status::Loaded);
        assert!(!opened.has_unsaved());
        assert!(!opened.can_save());
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let rejected = saving.open_revision(response(1));
        assert_eq!(rejected.draft, "newer local draft");
        assert_eq!(rejected.base, 3);
        assert_eq!(rejected.pending, Some(intent));
    }

    #[test]
    fn browser_exit_remains_guarded_during_loading_saving_and_ambiguous_retry() {
        let clean = Editor {
            actor: "alice".into(),
            script: "script".into(),
            ..Editor::default()
        };
        assert!(!clean.has_unsaved());
        assert!(clean.start_read().unwrap().has_unsaved());
        let dirty = clean.edit("new dialogue".into());
        let (saving, intent) = dirty.start_save("op".into()).unwrap();
        assert!(saving.has_unsaved());
        let reverted = saving.edit("".into());
        assert!(!reverted.is_dirty());
        assert!(reverted.has_unsaved());
        let ambiguous = reverted.save_failed(&intent, &error(ErrorCode::Unavailable));
        assert!(!ambiguous.busy);
        assert!(ambiguous.has_unsaved());
        assert_eq!(ambiguous.pending, Some(intent));
    }

    #[test]
    fn successful_validation_applies_only_to_the_exact_actor_and_snapshot() {
        let checking = ValidationPreview::default().start("alice".into(), "script text".into());
        assert!(checking.busy);
        assert!(!checking.is_current("alice", "script text"));
        let checked = checking.finish(checking.ticket, "alice", "script text", Ok(vec![]));
        assert_eq!(checked.ticket, 1);
        assert_eq!(checked.actor, "alice");
        assert_eq!(checked.snapshot, "script text");
        assert_eq!(checked.error, None);
        assert_eq!(checked.issues, vec![]);
        assert!(!checked.busy);
        assert!(checked.valid);
        assert!(checked.is_current("alice", "script text"));
        assert!(!checked.is_current("bob", "script text"));
        assert!(!checked.is_current("alice", "changed text"));
    }

    #[test]
    fn late_or_mismatched_validation_cannot_replace_a_newer_result() {
        let first = ValidationPreview::default().start("alice".into(), "first".into());
        let second = first.start("alice".into(), "second".into());
        assert_eq!(second.ticket, 2);
        for (ticket, actor, snapshot) in [
            (first.ticket, "alice", "first"),
            (second.ticket, "bob", "second"),
            (second.ticket, "alice", "first"),
        ] {
            assert_eq!(second.finish(ticket, actor, snapshot, Ok(vec![])), second);
            assert_eq!(
                second.finish(ticket, actor, snapshot, Err(error(ErrorCode::Unavailable))),
                second
            );
        }
        let ready = second.finish(second.ticket, "alice", "second", Ok(vec![]));
        assert!(ready.is_current("alice", "second"));
        assert_eq!(
            ready.finish(
                second.ticket,
                "alice",
                "second",
                Err(error(ErrorCode::Unavailable))
            ),
            ready
        );
    }

    #[test]
    fn findings_or_failed_validation_never_claim_a_valid_snapshot() {
        let checking = ValidationPreview::default().start("alice".into(), "text".into());
        let issues = vec![FieldIssue {
            path: "/acts/0/scenes/0/dialogues/0/text".into(),
            rule: "empty_spoken_text".into(),
        }];
        let findings = checking.finish(checking.ticket, "alice", "text", Ok(issues.clone()));
        assert_eq!(findings.issues, issues);
        assert_eq!(findings.error, None);
        assert!(!findings.is_current("alice", "text"));
        let failed = checking.finish(
            checking.ticket,
            "alice",
            "text",
            Err(ApiError {
                code: ErrorCode::InvalidScript,
                current_revision: None,
                issues: issues.clone(),
            }),
        );
        assert_eq!(failed.error, Some(ErrorCode::InvalidScript));
        assert_eq!(failed.issues, issues);
        assert_eq!(failed.snapshot, "text");
        assert!(!failed.valid);
        assert!(!failed.busy);
        assert!(!failed.is_current("alice", "text"));
    }

    #[test]
    fn validation_ticket_exhaustion_cannot_reuse_a_completed_request() {
        let exhausted = ValidationPreview {
            ticket: u64::MAX,
            valid: true,
            ..ValidationPreview::default()
        };
        let refused = exhausted.start("alice".into(), "text".into());
        assert_eq!(refused.ticket, u64::MAX);
        assert_eq!(refused.error, Some(ErrorCode::Unavailable));
        assert!(!refused.busy);
        assert!(!refused.valid);
    }

    fn error(code: ErrorCode) -> ApiError {
        ApiError {
            code,
            current_revision: None,
            issues: vec![],
        }
    }

    #[test]
    fn expired_session_after_ambiguous_save_keeps_the_original_retry_identity() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "text".into(),
            ..Editor::default()
        };
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let unavailable = saving.failed(&ApiError {
            code: ErrorCode::Unavailable,
            current_revision: None,
            issues: vec![],
        });
        let expired = unavailable.failed(&ApiError {
            code: ErrorCode::Unauthenticated,
            current_revision: None,
            issues: vec![],
        });
        assert_eq!(expired.pending, Some(intent));
        assert_eq!(expired.draft, "text");
        let login_denied = expired.session_failed(&ApiError {
            code: ErrorCode::Unauthenticated,
            current_revision: None,
            issues: vec![],
        });
        assert_eq!(login_denied.pending, expired.pending);
        let mut different_actor = login_denied.clone();
        different_actor.actor = "bob".into();
        assert!(different_actor.retry().is_none());
        different_actor.actor = "alice".into();
        assert_eq!(different_actor.retry().unwrap().1, expired.pending.unwrap());
    }

    #[test]
    fn acknowledgement_from_a_changed_cookie_does_not_clear_the_original_intent() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "text".into(),
            ..Editor::default()
        };
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let mut unexpected = response(1);
        unexpected.accepted_by = "bob".into();
        let result = saving.saved(&intent, unexpected);
        assert_eq!(result.pending, Some(intent));
        assert!(result.ambiguous);
        assert!(!result.busy);
        assert_eq!(result.base, 0);
        assert_eq!(result.draft, "text");
        assert!(result.retry().is_none());
    }

    fn response(revision: u64) -> RevisionResponse {
        RevisionResponse {
            script_id: "script".into(),
            revision,
            accepted_by: "alice".into(),
            accepted_at: "2026-10-10T00:00:00.000000Z".into(),
            content_digest: "content".into(),
            export_digest: "export".into(),
            script_json: "canonical".into(),
        }
    }
    #[test]
    fn timeout_retry_reuses_snapshot_and_edit_during_save_survives_ack() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "Ngày mai, mình có diễn tiếp không?".into(),
            ..Editor::default()
        };
        let (saving, intent) = initial.start_save("operation".into()).unwrap();
        assert!(saving.start_save("duplicate".into()).is_none());
        let mut failed = saving.failed(&ApiError {
            code: ErrorCode::Unavailable,
            current_revision: None,
            issues: vec![],
        });
        failed.draft.push_str(" Nhé?");
        assert!(failed.start_save("new".into()).is_none());
        let (retrying, retried) = failed.retry().unwrap();
        assert_eq!(retried, intent);
        let saved = retrying.saved(&retried, response(1));
        assert_eq!(saved.draft, failed.draft);
        assert_eq!(saved.base, 1);
        assert_eq!(saved.status, Status::DraftPreserved);
        assert!(saved.pending.is_none());
    }
    #[test]
    fn conflict_and_reload_preserve_text_until_explicit_rebase() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            draft: "my edit".into(),
            base: 1,
            ..Editor::default()
        };
        let (saving, _) = initial.start_save("op".into()).unwrap();
        let conflict = saving.failed(&ApiError {
            code: ErrorCode::StaleRevision,
            current_revision: Some(2),
            issues: vec![],
        });
        assert_eq!(conflict.draft, "my edit");
        assert_eq!(conflict.base, 1);
        let reading = conflict.start_read().unwrap();
        let loaded = reading.loaded(reading.ticket, response(2));
        assert_eq!(loaded.draft, "my edit");
        assert_eq!(loaded.base, 1);
        assert_eq!(loaded.rebase().base, 2);
    }
    #[test]
    fn late_completed_read_cannot_replace_newer_selection_or_ticket() {
        let initial = Editor {
            actor: "alice".into(),
            script: "script".into(),
            ..Editor::default()
        };
        let mut reading = initial.start_read().unwrap();
        let old = reading.ticket;
        reading.ticket += 1;
        let late = reading.loaded(old, response(1));
        assert!(late.remote.is_none());
        reading.script = "other".into();
        assert!(reading.loaded(reading.ticket, response(1)).remote.is_none());
    }
}
