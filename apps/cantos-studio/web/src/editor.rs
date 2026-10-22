//! Pure Studio save state; a transport failure preserves both text and retry identity.
//!
//! One closed [`Phase`] names the only request the editor owns, so a read, a save and a session
//! check can never overlap and an unresolved save always carries its exact intent. Text
//! ownership and the last settled outcome are separate facts; "dirty" is derived from the bytes.
use cantos_contracts::{ApiError, ErrorCode, FieldIssue, RevisionResponse, SaveRevisionRequest};

/// What the Studio shows. `Loading` and `Saving` are derived from the phase, never stored.
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

/// One in-flight revision read, accepted only for the exact actor, script and sequence that
/// issued it. Only [`Editor::start_read`] creates one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadTicket {
    sequence: u64,
    actor: String,
    script: String,
}

impl ReadTicket {
    pub fn script(&self) -> &str {
        &self.script
    }
}

/// A submitted save whose outcome the editor has not observed.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingSave {
    intent: Intent,
    /// An earlier attempt may have committed (timeout, corrupt answer or an acknowledgement for
    /// another actor), so a later authorization denial cannot discard this retry identity.
    ambiguous: bool,
}

/// Work held while the shell confirms the session cookie.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Held {
    Nothing,
    Read(ReadTicket),
    Save(PendingSave),
}

/// The one request the editor owns; any other combination is unrepresentable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Phase {
    /// No request in flight and no unresolved save.
    #[default]
    Idle,
    Reading(ReadTicket),
    /// The save request is in flight.
    Saving(PendingSave),
    /// The save is not in flight and its outcome is unknown or retryable. Only an exact retry
    /// or a late acknowledgement of the same intent may settle it.
    ReconciliationRequired(PendingSave),
    /// The shell is confirming the session. A held read or save still settles under its own
    /// guard; the check itself ends in `signed_in`, `session_failed` or `release_session_check`.
    CheckingSession(Held),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editor {
    actor: String,
    script: String,
    draft: String,
    /// Local bytes acknowledged by the last save, before server normalization.
    baseline: String,
    /// The author who owns `draft`; empty until someone types or a revision is opened.
    draft_actor: String,
    base: u64,
    remote: Option<RevisionResponse>,
    /// Monotonic read sequence. A new target, account or read invalidates every older ticket.
    sequence: u64,
    phase: Phase,
    /// The last settled outcome, shown while no request is in flight.
    notice: Status,
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
            remote: None,
            sequence: 0,
            phase: Phase::Idle,
            notice: Status::Idle,
        }
    }
}

impl Editor {
    pub fn actor(&self) -> &str {
        &self.actor
    }

    pub fn script(&self) -> &str {
        &self.script
    }

    pub fn draft(&self) -> &str {
        &self.draft
    }

    pub fn base(&self) -> u64 {
        self.base
    }

    pub fn remote(&self) -> Option<&RevisionResponse> {
        self.remote.as_ref()
    }

    pub fn status(&self) -> Status {
        match self.phase {
            Phase::Reading(_) | Phase::CheckingSession(_) => Status::Loading,
            Phase::Saving(_) => Status::Saving,
            Phase::Idle | Phase::ReconciliationRequired(_) => self.notice,
        }
    }

    /// A read, save or session check is in flight.
    pub fn busy(&self) -> bool {
        match self.phase {
            Phase::Reading(_) | Phase::Saving(_) | Phase::CheckingSession(_) => true,
            Phase::Idle | Phase::ReconciliationRequired(_) => false,
        }
    }

    /// The unresolved save, whether in flight, held or awaiting reconciliation.
    pub fn pending(&self) -> Option<&Intent> {
        self.pending_save().map(|save| &save.intent)
    }

    /// An unresolved save is not in flight: retrying the same intent is the only way forward.
    pub fn awaits_reconciliation(&self) -> bool {
        matches!(self.phase, Phase::ReconciliationRequired(_))
    }

    fn pending_save(&self) -> Option<&PendingSave> {
        match &self.phase {
            Phase::Saving(save)
            | Phase::ReconciliationRequired(save)
            | Phase::CheckingSession(Held::Save(save)) => Some(save),
            Phase::Idle
            | Phase::Reading(_)
            | Phase::CheckingSession(Held::Nothing | Held::Read(_)) => None,
        }
    }

    /// The phase once nothing is in flight: a read is abandoned, an unresolved save survives.
    fn settled_phase(&self) -> Phase {
        match self.pending_save() {
            Some(save) => Phase::ReconciliationRequired(save.clone()),
            None => Phase::Idle,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.draft != self.baseline
    }

    /// The shell also checks IME composition before navigation or browser unload.
    pub fn has_unsaved(&self) -> bool {
        self.is_dirty() || self.phase != Phase::Idle
    }

    pub fn can_edit(&self) -> bool {
        self.draft_actor.is_empty() || self.draft_actor == self.actor
    }

    pub fn can_save(&self) -> bool {
        self.phase == Phase::Idle
            && !self.actor.is_empty()
            && !self.script.is_empty()
            && self.can_edit()
            && self.is_dirty()
    }

    /// Session changes preserve the local author's bytes and unresolved save identity.
    pub fn signed_in(&self, actor: String) -> Self {
        if actor != self.actor && !self.is_dirty() && self.pending_save().is_none() {
            // A clean private document has no local work to retain across account ownership.
            // Also invalidate any outstanding read; no A source identity may be projected as B.
            return Self {
                actor,
                sequence: self.sequence.saturating_add(1),
                notice: Status::SignedIn,
                ..Self::default()
            };
        }
        let mut next = self.clone();
        if next.actor != actor {
            next.sequence = next.sequence.saturating_add(1);
            next.remote = None;
        }
        next.actor = actor;
        if next.draft_actor.is_empty() && !next.draft.is_empty() {
            next.draft_actor = next.actor.clone();
        }
        next.phase = self.settled_phase();
        next.notice = if !next.can_edit()
            || next
                .pending()
                .is_some_and(|intent| intent.actor != next.actor)
        {
            Status::Forbidden
        } else {
            Status::SignedIn
        };
        next
    }

    /// The shell is confirming the session cookie. New reads, saves, retries and discards wait;
    /// work already in flight is held and still settles under its own guard.
    pub fn checking_session(&self) -> Self {
        let held = match &self.phase {
            Phase::Idle => Held::Nothing,
            Phase::Reading(ticket) => Held::Read(ticket.clone()),
            Phase::Saving(save) | Phase::ReconciliationRequired(save) => Held::Save(save.clone()),
            Phase::CheckingSession(_) => return self.clone(),
        };
        Self {
            phase: Phase::CheckingSession(held),
            ..self.clone()
        }
    }

    /// The person cancels the session check (the shell fences the pending login/read). The check
    /// stops guarding unload, while an unresolved save keeps its retry identity and the draft
    /// keeps its own guard.
    pub fn release_session_check(&self) -> Self {
        if !matches!(self.phase, Phase::CheckingSession(_)) {
            return self.clone();
        }
        Self {
            phase: self.settled_phase(),
            ..self.clone()
        }
    }

    pub fn select_script(&self, script: String) -> Self {
        if self.has_unsaved() || self.script == script {
            return self.clone();
        }
        self.discard_and_select(script)
    }

    /// Only an explicit user discard may replace a dirty document. Pending saves must reconcile.
    pub fn discard_and_select(&self, script: String) -> Self {
        if self.phase != Phase::Idle {
            return self.clone();
        }
        Self {
            actor: self.actor.clone(),
            script,
            sequence: self.sequence.saturating_add(1),
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
        if self.phase == Phase::Idle
            && matches!(
                self.notice,
                Status::Idle
                    | Status::SignedIn
                    | Status::Loaded
                    | Status::Saved
                    | Status::DraftPreserved
                    | Status::Dirty
            )
        {
            next.notice = if next.is_dirty() {
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
        next.phase = Phase::Saving(PendingSave {
            intent: intent.clone(),
            ambiguous: false,
        });
        next.draft_actor = intent.actor.clone();
        Some((next, intent))
    }

    pub fn retry(&self) -> Option<(Self, Intent)> {
        let Phase::ReconciliationRequired(save) = &self.phase else {
            return None;
        };
        if save.intent.actor != self.actor || save.intent.script != self.script {
            return None;
        }
        let mut next = self.clone();
        next.phase = Phase::Saving(save.clone());
        Some((next, save.intent.clone()))
    }

    pub fn saved(&self, intent: &Intent, response: RevisionResponse) -> Self {
        if self.pending() != Some(intent)
            || self.script != intent.script
            || self.script != response.script_id
            || self.actor != intent.actor
        {
            return self.clone();
        }
        let mut next = self.clone();
        if response.accepted_by != intent.actor {
            // A different tab may have replaced the session cookie during this request.
            next.phase = Phase::ReconciliationRequired(PendingSave {
                intent: intent.clone(),
                ambiguous: true,
            });
            next.actor.clear();
            next.notice = Status::Unauthenticated;
            return next;
        }
        next.base = response.revision;
        next.baseline = intent.request.script_json.clone();
        next.remote = Some(response);
        next.phase = Phase::Idle;
        // Never rewrite the textarea on acknowledgement (including while IME composing).
        next.notice = if self.draft == intent.request.script_json {
            Status::Saved
        } else {
            Status::DraftPreserved
        };
        next
    }

    /// Settle whatever request the editor owns with a failure. An unresolved save survives
    /// unless the failure definitively rejects it.
    pub fn failed(&self, error: &ApiError) -> Self {
        let phase = match self.pending_save() {
            None => Phase::Idle,
            Some(save) => match error.code {
                ErrorCode::Unavailable | ErrorCode::CorruptRevision => {
                    Phase::ReconciliationRequired(PendingSave {
                        intent: save.intent.clone(),
                        ambiguous: true,
                    })
                }
                // A later authorization denial says nothing about an earlier ambiguous commit.
                // Reauthenticate as the submitting actor, then reconcile the original operation.
                ErrorCode::Unauthenticated | ErrorCode::Forbidden | ErrorCode::NotFound => {
                    if save.ambiguous {
                        Phase::ReconciliationRequired(save.clone())
                    } else {
                        Phase::Idle
                    }
                }
                ErrorCode::InvalidRequest
                | ErrorCode::InvalidScript
                | ErrorCode::EvidenceUnavailable
                | ErrorCode::StaleRevision
                | ErrorCode::OperationReused
                | ErrorCode::ProposalAlreadySubmitted
                | ErrorCode::StaleProductionInputs
                | ErrorCode::ProductionBlocked
                | ErrorCode::InvalidCatalog
                | ErrorCode::CatalogConflict
                | ErrorCode::StaleCursor
                | ErrorCode::AudioRejected => Phase::Idle,
            },
        };
        Self {
            phase,
            notice: error_status(&error.code),
            ..self.clone()
        }
    }

    pub fn save_failed(&self, intent: &Intent, error: &ApiError) -> Self {
        if self.pending() != Some(intent)
            || self.actor != intent.actor
            || self.script != intent.script
        {
            return self.clone();
        }
        self.failed(error)
    }

    /// Findings describe the submitted bytes, never a newer draft or another context.
    pub fn accepts_save_findings(&self, intent: &Intent) -> bool {
        self.pending() == Some(intent)
            && self.actor == intent.actor
            && self.script == intent.script
            && self.draft == intent.request.script_json
    }

    pub fn session_failed(&self, error: &ApiError) -> Self {
        Self {
            phase: self.settled_phase(),
            notice: error_status(&error.code),
            ..self.clone()
        }
    }

    pub fn start_read(&self) -> Option<(Self, ReadTicket)> {
        if self.phase != Phase::Idle
            || self.actor.is_empty()
            || self.script.is_empty()
            || !self.can_edit()
        {
            return None;
        }
        let ticket = ReadTicket {
            sequence: self.sequence.checked_add(1)?,
            actor: self.actor.clone(),
            script: self.script.clone(),
        };
        let mut next = self.clone();
        next.sequence = ticket.sequence;
        next.phase = Phase::Reading(ticket.clone());
        Some((next, ticket))
    }

    pub fn loaded(&self, ticket: &ReadTicket, response: RevisionResponse) -> Self {
        self.loaded_with_buffer(ticket, response, false)
    }

    /// Native text may be newer than the reducer until blur/composition commits it.
    /// The shell keeps its buffer/navigation guard active until those bytes are committed.
    pub fn loaded_with_buffer(
        &self,
        ticket: &ReadTicket,
        response: RevisionResponse,
        buffer_pending: bool,
    ) -> Self {
        if !self.accepts_read(ticket) || response.script_id != self.script {
            return self.clone();
        }
        let mut next = self.clone();
        next.phase = Phase::Idle;
        if !self.is_dirty() && !buffer_pending {
            next.draft = response.script_json.clone();
            next.baseline = response.script_json.clone();
            next.draft_actor = self.actor.clone();
            next.base = response.revision;
            next.notice = Status::Loaded;
        } else {
            next.notice = Status::DraftPreserved;
        }
        next.remote = Some(response);
        next
    }

    fn accepts_read(&self, ticket: &ReadTicket) -> bool {
        let held = match &self.phase {
            Phase::Reading(held) | Phase::CheckingSession(Held::Read(held)) => held,
            Phase::Idle
            | Phase::Saving(_)
            | Phase::ReconciliationRequired(_)
            | Phase::CheckingSession(Held::Nothing | Held::Save(_)) => return false,
        };
        held == ticket
            && ticket.sequence == self.sequence
            && ticket.actor == self.actor
            && ticket.script == self.script
    }

    pub fn read_failed(&self, ticket: &ReadTicket, error: &ApiError) -> Self {
        if !self.accepts_read(ticket) {
            return self.clone();
        }
        self.failed(error)
    }

    /// Explicit history/discard action. The shell confirms any dirty draft before calling this.
    pub fn open_revision(&self, response: RevisionResponse) -> Self {
        if self.phase != Phase::Idle || self.actor.is_empty() {
            return self.clone();
        }
        Self {
            actor: self.actor.clone(),
            script: response.script_id.clone(),
            draft: response.script_json.clone(),
            baseline: response.script_json.clone(),
            draft_actor: self.actor.clone(),
            base: response.revision,
            notice: Status::Loaded,
            remote: Some(response),
            sequence: self.sequence.saturating_add(1),
            phase: Phase::Idle,
        }
    }

    /// Explicit user reconciliation: keep local text and adopt the displayed remote head.
    pub fn rebase(&self) -> Self {
        let mut next = self.clone();
        if self.phase == Phase::Idle && self.can_edit() {
            if let Some(remote) = &self.remote {
                next.base = remote.revision;
                next.notice = Status::DraftPreserved;
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
        ErrorCode::InvalidRequest | ErrorCode::InvalidCatalog | ErrorCode::AudioRejected => {
            Status::InvalidRequest
        }
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
    use super::{Editor, Phase, ReadTicket, Status, ValidationPreview};
    use cantos_contracts::{ApiError, ErrorCode, FieldIssue, RevisionResponse};

    /// A signed-in author typing into a selected script, built only from public transitions.
    fn editing(actor: &str, script: &str, text: &str) -> Editor {
        Editor::default()
            .signed_in(actor.into())
            .select_script(script.into())
            .edit(text.into())
    }

    /// An author editing an opened revision `revision` of `"script"`.
    fn editing_revision(revision: u64, text: &str) -> Editor {
        Editor::default()
            .signed_in("alice".into())
            .open_revision(response(revision))
            .edit(text.into())
    }

    fn reading(editor: &Editor) -> (Editor, ReadTicket) {
        editor.start_read().unwrap()
    }

    #[test]
    fn late_save_findings_do_not_attach_to_newer_editor_text() {
        let initial = editing("alice", "script", "Ngày mai, mình có diễn tiếp không?");
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        assert!(saving.accepts_save_findings(&intent));
        let changed = saving.edit("Ngày mai, mình vẫn diễn tiếp.".into());
        assert!(!changed.accepts_save_findings(&intent));
        assert_eq!(changed.draft(), "Ngày mai, mình vẫn diễn tiếp.");
        assert_eq!(changed.pending(), Some(&intent));
    }

    #[test]
    fn save_findings_require_the_pending_actor_script_and_exact_snapshot() {
        let (saving, intent) = editing("alice", "script", "Nguồn đã sửa")
            .start_save("op".into())
            .unwrap();
        for changed in [
            saving.signed_in("bob".into()),
            // Defensive: no transition can move a pending save to another script.
            Editor {
                script: "another".into(),
                ..saving.clone()
            },
            saving.save_failed(&intent, &error(ErrorCode::InvalidScript)),
        ] {
            assert!(!changed.accepts_save_findings(&intent));
        }
        assert!(saving.accepts_save_findings(&intent));
        assert!(saving
            .edit("new".into())
            .edit(saving.draft().into())
            .accepts_save_findings(&intent));
    }

    #[test]
    fn typing_after_an_acknowledged_save_is_visibly_unsaved() {
        let initial = editing("alice", "script", "original");
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let saved = saving.saved(&intent, response(1));
        assert_eq!(saved.status(), Status::Saved);
        assert_eq!(saved.edit("new text".into()).status(), Status::Dirty);
        assert_eq!(saved.edit("new text".into()).base(), 1);
    }

    #[test]
    fn changing_a_clean_save_target_clears_the_previous_document() {
        let initial = editing("alice", "script", "original");
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let saved = saving.saved(&intent, response(1));
        let selected = saved.select_script("other".into());
        assert_eq!(selected.script(), "other");
        assert_eq!(selected.draft(), "");
        assert_eq!(selected.base(), 0);
        assert!(selected.remote().is_none());
        assert_eq!(selected.status(), Status::Idle);
        assert_eq!(saved.select_script("script".into()).base(), 1);
        assert_eq!(saving.select_script("other".into()).script(), "script");
    }

    #[test]
    fn reverting_an_edit_to_the_acknowledged_snapshot_is_clean() {
        let initial = editing("alice", "script", "Ngày mai, mình có diễn tiếp không?");
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let saved = saving.saved(&intent, response(1));
        let reverted = saved.edit("changed".into()).edit(initial.draft().into());
        assert_eq!(reverted.status(), Status::Saved);
        assert!(reverted.start_save("duplicate".into()).is_none());
    }

    #[test]
    fn a_clean_refresh_replaces_the_previous_saved_document() {
        let initial = editing("alice", "script", "original");
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let (reading, ticket) = reading(&saving.saved(&intent, response(1)));
        let loaded = reading.loaded(&ticket, response(2));
        assert_eq!(loaded.draft(), "canonical");
        assert_eq!(loaded.base(), 2);
        assert_eq!(loaded.status(), Status::Loaded);
    }

    #[test]
    fn selecting_another_script_cannot_move_an_unsaved_draft() {
        let initial = editing_revision(3, "my draft");
        let selected = initial.select_script("other".into());
        assert_eq!(selected.script(), "script");
        assert_eq!(selected.draft(), "my draft");
        assert_eq!(selected.base(), 3);
        assert_eq!(selected, initial);
    }

    #[test]
    fn server_normalization_does_not_make_acknowledged_local_bytes_dirty() {
        let initial = editing(
            "alice",
            "script",
            "Ngo\u{31b}o\u{31b}i da\u{302}n chuye\u{323}\u{302}n",
        );
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let saved = saving.saved(&intent, response(1));
        assert_eq!(saved.baseline, initial.draft());
        assert_eq!(saved.draft(), initial.draft());
        assert_eq!(saved.remote().unwrap().script_json, "canonical");
        assert!(!saved.is_dirty());
        assert!(!saved.has_unsaved());
        assert!(!saved.can_save());
        assert_eq!(saved.draft_actor, "alice");
    }

    #[test]
    fn explicit_discard_selects_a_new_target_without_reusing_the_old_draft() {
        let initial = editing_revision(3, "my draft");
        let selected = initial.discard_and_select("other".into());
        assert_eq!(selected.script(), "other");
        assert_eq!(selected.draft(), "");
        assert_eq!(selected.baseline, "");
        assert_eq!(selected.draft_actor, "");
        assert_eq!(selected.actor(), "alice");
        assert_eq!(selected.base(), 0);
        assert_eq!(selected.sequence, initial.sequence + 1);
        assert_eq!(selected.status(), Status::Idle);
        assert!(!selected.has_unsaved());

        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let rejected = saving.discard_and_select("other".into());
        assert_eq!(rejected, saving);
        assert_eq!(rejected.pending(), Some(&intent));
        let ambiguous = saving.save_failed(&intent, &error(ErrorCode::Unavailable));
        let rejected = ambiguous.discard_and_select("other".into());
        assert_eq!(rejected, ambiguous);
        assert_eq!(rejected.script(), "script");
        assert_eq!(rejected.pending(), Some(&intent));
        assert!(rejected.awaits_reconciliation());
    }

    #[test]
    fn edit_and_read_during_an_earlier_read_preserve_the_new_local_text() {
        let initial = Editor::default()
            .signed_in("alice".into())
            .open_revision(response(1));
        let (reading, ticket) = reading(&initial);
        let edited = reading.edit("new dialogue".into());
        let loaded = edited.loaded(&ticket, response(2));
        assert_eq!(loaded.draft(), "new dialogue");
        assert_eq!(loaded.baseline, "canonical");
        assert_eq!(loaded.base(), 1);
        assert_eq!(loaded.remote().unwrap().revision, 2);
        assert_eq!(loaded.status(), Status::DraftPreserved);
        assert!(!loaded.busy());
        assert!(loaded.has_unsaved());
    }

    #[test]
    fn a_read_cannot_replace_clean_reducer_rows_with_native_text_still_buffered() {
        let original = include_str!(
            "../../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json"
        );
        let mut initial_response = response(1);
        initial_response.script_json = original.into();
        let opened = Editor::default()
            .signed_in("alice".into())
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
            let (reading, ticket) = reading(&opened);
            let preserved = reading.loaded_with_buffer(&ticket, remote.clone(), true);
            assert_eq!(preserved.draft(), original);
            assert_eq!(preserved.baseline, original);
            assert_eq!(preserved.base(), 1);
            assert_eq!(preserved.draft_actor, "alice");
            assert_eq!(preserved.status(), Status::DraftPreserved);
            assert_eq!(preserved.remote().unwrap().script_json, remote.script_json);
            assert_eq!(preserved.remote().unwrap().revision, 2);
            assert!(!preserved.busy());
            // The shell owns the actual native buffer; committing it still marks this base dirty.
            let committed = preserved.edit("An: Chữ đang nhập trong ô vẫn còn.".into());
            assert!(committed.is_dirty());
            assert_eq!(committed.base(), 1);
            let replaced = reading.loaded_with_buffer(&ticket, remote.clone(), false);
            assert_eq!(replaced.draft(), remote.script_json);
            assert_eq!(replaced.base(), 2);
            assert_eq!(replaced.status(), Status::Loaded);
        }
    }

    #[test]
    fn changing_a_clean_account_clears_private_revision_identity_and_bytes() {
        let (first, ticket) = reading(
            &Editor::default()
                .signed_in("alice".into())
                .select_script("script".into()),
        );
        let clean = first.loaded(&ticket, response(3));
        assert!(!clean.is_dirty());
        let (reading, ticket) = reading(&clean);
        let bob = reading.signed_in("bob".into());
        assert_eq!(bob.actor(), "bob");
        assert!(bob.script().is_empty());
        assert_eq!(bob.base(), 0);
        assert!(bob.draft().is_empty());
        assert!(bob.baseline.is_empty());
        assert!(bob.draft_actor.is_empty());
        assert!(bob.remote().is_none());
        assert_eq!(bob.phase, Phase::Idle);
        assert_eq!(bob.sequence, reading.sequence + 1);
        assert!(bob.can_edit());
        assert_eq!(bob.status(), Status::SignedIn);
        assert_eq!(bob.loaded(&ticket, response(4)), bob);
    }

    #[test]
    fn session_changes_preserve_the_original_author_and_invalidate_reads() {
        let (reading, ticket) = reading(&editing("alice", "script", "Alice's draft"));
        let changed = reading.signed_in("bob".into());
        assert_eq!(changed.draft_actor, "alice");
        assert_eq!(changed.draft(), "Alice's draft");
        assert_eq!(changed.status(), Status::Forbidden);
        assert_eq!(changed.sequence, reading.sequence + 1);
        assert!(!changed.busy());
        assert!(!changed.can_edit());
        assert!(!changed.can_save());
        assert!(changed.start_read().is_none());
        assert_eq!(changed.edit("Bob's draft".into()).draft(), "Alice's draft");
        assert!(changed.loaded(&ticket, response(2)).remote().is_none());
        assert_eq!(
            changed
                .read_failed(&ticket, &error(ErrorCode::NotFound))
                .status(),
            Status::Forbidden
        );
        let restored = changed.signed_in("alice".into());
        assert_eq!(restored.draft(), "Alice's draft");
        assert_eq!(restored.status(), Status::SignedIn);
        assert!(restored.can_edit());
        assert!(restored.can_save());
    }

    #[test]
    fn a_late_save_response_under_another_actor_cannot_acknowledge_or_fail_the_draft() {
        let initial = editing("alice", "script", "Alice's draft");
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let changed = saving.signed_in("bob".into());
        let late = changed.saved(&intent, response(1));
        assert_eq!(late.actor(), "bob");
        assert_eq!(late.status(), Status::Forbidden);
        assert_eq!(late.draft_actor, "alice");
        assert_eq!(late.pending(), Some(&intent));
        assert_eq!(late.base(), 0);
        assert_eq!(late.baseline, "");
        let failed = changed.save_failed(&intent, &error(ErrorCode::InvalidScript));
        assert_eq!(failed.pending(), Some(&intent));
        assert_eq!(failed.status(), Status::Forbidden);
        assert_eq!(failed.draft(), "Alice's draft");
    }

    #[test]
    fn a_consumed_read_ticket_cannot_reapply_success_or_failure() {
        let (reading, ticket) = reading(
            &Editor::default()
                .signed_in("alice".into())
                .select_script("script".into()),
        );
        let loaded = reading.loaded(&ticket, response(1));
        let late = loaded.loaded(&ticket, response(2));
        assert_eq!(late.base(), 1);
        assert_eq!(late.remote().unwrap().revision, 1);
        assert_eq!(
            loaded
                .read_failed(&ticket, &error(ErrorCode::Unavailable))
                .status(),
            Status::Loaded
        );
    }

    #[test]
    fn explicit_history_open_is_clean_and_cannot_discard_an_unresolved_save() {
        let initial = editing_revision(3, "newer local draft");
        let opened = initial.open_revision(response(1));
        assert_eq!(opened.draft(), "canonical");
        assert_eq!(opened.baseline, "canonical");
        assert_eq!(opened.draft_actor, "alice");
        assert_eq!(opened.base(), 1);
        assert_eq!(opened.status(), Status::Loaded);
        assert!(!opened.has_unsaved());
        assert!(!opened.can_save());
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let rejected = saving.open_revision(response(1));
        assert_eq!(rejected.draft(), "newer local draft");
        assert_eq!(rejected.base(), 3);
        assert_eq!(rejected.pending(), Some(&intent));
    }

    #[test]
    fn browser_exit_remains_guarded_during_loading_saving_and_ambiguous_retry() {
        let clean = Editor::default()
            .signed_in("alice".into())
            .select_script("script".into());
        assert!(!clean.has_unsaved());
        assert!(reading(&clean).0.has_unsaved());
        let dirty = clean.edit("new dialogue".into());
        let (saving, intent) = dirty.start_save("op".into()).unwrap();
        assert!(saving.has_unsaved());
        let reverted = saving.edit("".into());
        assert!(!reverted.is_dirty());
        assert!(reverted.has_unsaved());
        let ambiguous = reverted.save_failed(&intent, &error(ErrorCode::Unavailable));
        assert!(!ambiguous.busy());
        assert!(ambiguous.has_unsaved());
        assert_eq!(ambiguous.pending(), Some(&intent));
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
        let initial = editing("alice", "script", "text");
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let unavailable = saving.save_failed(&intent, &error(ErrorCode::Unavailable));
        let expired = unavailable.failed(&error(ErrorCode::Unauthenticated));
        assert_eq!(expired.pending(), Some(&intent));
        assert_eq!(expired.draft(), "text");
        let login_denied = expired.session_failed(&error(ErrorCode::Unauthenticated));
        assert_eq!(login_denied.pending(), expired.pending());
        let different_actor = login_denied.signed_in("bob".into());
        assert!(different_actor.retry().is_none());
        let original_actor = different_actor.signed_in("alice".into());
        assert_eq!(original_actor.retry().unwrap().1, intent);
    }

    #[test]
    fn acknowledgement_from_a_changed_cookie_does_not_clear_the_original_intent() {
        let initial = editing("alice", "script", "text");
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let mut unexpected = response(1);
        unexpected.accepted_by = "bob".into();
        let result = saving.saved(&intent, unexpected);
        assert_eq!(result.pending(), Some(&intent));
        assert!(result.awaits_reconciliation());
        assert!(!result.busy());
        assert_eq!(result.base(), 0);
        assert_eq!(result.draft(), "text");
        assert!(result.retry().is_none());
        // Only the submitting actor may reconcile; a denial cannot discard the possible commit.
        let reauthenticated = result.signed_in("alice".into());
        let (retrying, retried) = reauthenticated.retry().unwrap();
        assert_eq!(retried, intent);
        let denied = retrying.save_failed(&intent, &error(ErrorCode::Forbidden));
        assert_eq!(denied.pending(), Some(&intent));
        assert_eq!(denied.status(), Status::Forbidden);
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
        let initial = editing("alice", "script", "Ngày mai, mình có diễn tiếp không?");
        let (saving, intent) = initial.start_save("operation".into()).unwrap();
        assert!(saving.start_save("duplicate".into()).is_none());
        let failed = saving.save_failed(&intent, &error(ErrorCode::Unavailable));
        let failed = failed.edit(format!("{} Nhé?", failed.draft()));
        assert!(failed.start_save("new".into()).is_none());
        let (retrying, retried) = failed.retry().unwrap();
        assert_eq!(retried, intent);
        assert_eq!(retrying.status(), Status::Saving);
        let saved = retrying.saved(&retried, response(1));
        assert_eq!(saved.draft(), failed.draft());
        assert_eq!(saved.base(), 1);
        assert_eq!(saved.status(), Status::DraftPreserved);
        assert!(saved.pending().is_none());
    }
    #[test]
    fn conflict_and_reload_preserve_text_until_explicit_rebase() {
        let initial = editing_revision(1, "my edit");
        let (saving, intent) = initial.start_save("op".into()).unwrap();
        let conflict = saving.save_failed(
            &intent,
            &ApiError {
                code: ErrorCode::StaleRevision,
                current_revision: Some(2),
                issues: vec![],
            },
        );
        assert_eq!(conflict.draft(), "my edit");
        assert_eq!(conflict.base(), 1);
        assert_eq!(conflict.status(), Status::StaleRevision);
        let (reading, ticket) = reading(&conflict);
        let loaded = reading.loaded(&ticket, response(2));
        assert_eq!(loaded.draft(), "my edit");
        assert_eq!(loaded.base(), 1);
        assert_eq!(loaded.rebase().base(), 2);
    }
    #[test]
    fn late_completed_read_cannot_replace_newer_selection_or_ticket() {
        let selected = Editor::default()
            .signed_in("alice".into())
            .select_script("script".into());
        let (first, old) = reading(&selected);
        // A session settlement abandons the read; a newer read then owns the editor.
        let (second, newer) = reading(&first.signed_in("alice".into()));
        assert_eq!(second.loaded(&old, response(1)), second);
        assert_eq!(second.loaded(&newer, response(1)).base(), 1);
        // A newer selection cannot adopt the old script's response either.
        let (other, _) = reading(
            &first
                .signed_in("alice".into())
                .discard_and_select("other".into()),
        );
        assert_eq!(other.loaded(&old, response(1)), other);
        assert_eq!(other.read_failed(&old, &error(ErrorCode::NotFound)), other);
    }

    #[test]
    fn status_shows_activity_only_while_a_request_is_in_flight() {
        let clean = Editor::default()
            .signed_in("alice".into())
            .select_script("script".into());
        assert_eq!(reading(&clean).0.status(), Status::Loading);
        let (saving, intent) = clean
            .edit("Lời thoại".into())
            .start_save("op".into())
            .unwrap();
        assert_eq!(saving.status(), Status::Saving);
        assert_eq!(saving.checking_session().status(), Status::Loading);
        let unresolved = saving.save_failed(&intent, &error(ErrorCode::Unavailable));
        assert_eq!(unresolved.status(), Status::Unavailable);
        assert_eq!(unresolved.retry().unwrap().0.status(), Status::Saving);
    }

    #[test]
    fn session_check_blocks_new_requests_but_lets_held_work_settle() {
        let clean = Editor::default()
            .signed_in("alice".into())
            .select_script("script".into());
        let dirty = clean.edit("Bản nháp".into());
        let (saving, intent) = dirty.start_save("op".into()).unwrap();
        let unresolved = saving.save_failed(&intent, &error(ErrorCode::Unavailable));
        let (reading, ticket) = reading(&clean);
        for before in [
            clean.clone(),
            dirty.clone(),
            saving.clone(),
            unresolved.clone(),
            reading.clone(),
        ] {
            let checking = before.checking_session();
            assert!(checking.busy());
            assert!(checking.has_unsaved());
            assert_eq!(checking.status(), Status::Loading);
            assert_eq!(checking.checking_session(), checking);
            assert!(checking.start_save("new".into()).is_none());
            assert!(checking.start_read().is_none());
            assert!(checking.retry().is_none());
            assert_eq!(checking.discard_and_select("other".into()), checking);
            assert_eq!(checking.open_revision(response(9)), checking);
            assert_eq!(checking.rebase(), checking);
            assert_eq!(checking.pending(), before.pending());
            assert_eq!(checking.draft(), before.draft());
            // Cancelling the session check releases it, never the save identity.
            let released = checking.release_session_check();
            assert!(!released.busy());
            assert_eq!(released.pending(), before.pending());
            assert_eq!(released.awaits_reconciliation(), before.pending().is_some());
            assert_eq!(released.draft(), before.draft());
            assert_eq!(released.status(), before.checking_session().notice);
        }
        // A read or save already in flight still settles under its own exact guard.
        let held_read = reading.checking_session();
        let loaded = held_read.loaded(&ticket, response(4));
        assert_eq!(loaded.base(), 4);
        assert_eq!(loaded.phase, Phase::Idle);
        let held_save = saving.checking_session();
        let acknowledged = held_save.saved(&intent, response(5));
        assert!(acknowledged.pending().is_none());
        assert_eq!(acknowledged.status(), Status::Saved);
        // A settled check abandons the read and keeps the save for reconciliation.
        assert_eq!(held_read.signed_in("alice".into()).phase, Phase::Idle);
        assert!(held_save.signed_in("alice".into()).awaits_reconciliation());
        assert!(held_save
            .session_failed(&error(ErrorCode::Unauthenticated))
            .awaits_reconciliation());
        // Releasing outside a check never abandons in-flight work.
        assert_eq!(saving.release_session_check(), saving);
        assert_eq!(reading.release_session_check(), reading);
    }

    #[test]
    fn an_abandoned_read_cannot_settle_a_later_save_during_a_session_check() {
        let (reading, ticket) = reading(&editing("alice", "script", "Bản nháp A"));
        // The session settles (abandoning the read), the author saves, then a new check starts.
        let (saving, intent) = reading
            .signed_in("alice".into())
            .start_save("op".into())
            .unwrap();
        let checking = saving.checking_session();
        let failed = checking.read_failed(&ticket, &error(ErrorCode::NotFound));
        assert_eq!(failed, checking);
        assert_eq!(failed.pending(), Some(&intent));
        let loaded = checking.edit("".into()).loaded(&ticket, response(7));
        assert_eq!(loaded.base(), 0);
        assert!(loaded.remote().is_none());
        assert_eq!(loaded.pending(), Some(&intent));
        assert!(loaded.busy());
    }

    /// Shell inputs, including late, duplicated and foreign responses to earlier requests.
    #[derive(Clone, Debug)]
    enum Event {
        SignIn(&'static str),
        CheckSession,
        ReleaseCheck,
        SessionFails,
        Select(&'static str),
        Discard(&'static str),
        Edit(&'static str),
        Save,
        Retry,
        Ack,
        AckForOtherCookie,
        SaveFails(ErrorCode),
        OperationIdFails,
        Read,
        Load {
            buffered: bool,
        },
        ReadFails,
        /// Replays of the request before the last one: late duplicates from an older attempt.
        LoadStale,
        AckStale,
        Open,
        Rebase,
    }

    const EVENTS: [Event; 28] = [
        Event::SignIn("alice"),
        Event::SignIn("bob"),
        Event::SignIn(""),
        Event::CheckSession,
        Event::ReleaseCheck,
        Event::SessionFails,
        Event::Select("script"),
        Event::Select("other"),
        Event::Discard("script"),
        Event::Edit("Lời A"),
        Event::Edit("Lời B"),
        Event::Edit(""),
        Event::Save,
        Event::Retry,
        Event::Ack,
        Event::AckForOtherCookie,
        Event::SaveFails(ErrorCode::Unavailable),
        Event::SaveFails(ErrorCode::Forbidden),
        Event::SaveFails(ErrorCode::InvalidScript),
        Event::OperationIdFails,
        Event::Read,
        Event::Load { buffered: false },
        Event::Load { buffered: true },
        Event::ReadFails,
        Event::LoadStale,
        Event::AckStale,
        Event::Open,
        Event::Rebase,
    ];

    /// The editor plus the shell's memory of the last request it dispatched.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct World {
        editor: Editor,
        intent: Option<super::Intent>,
        ticket: Option<ReadTicket>,
        stale_intent: Option<super::Intent>,
        stale_ticket: Option<ReadTicket>,
        operations: u32,
    }

    fn head(script: &str, revision: u64, accepted_by: &str) -> RevisionResponse {
        RevisionResponse {
            script_id: script.into(),
            accepted_by: accepted_by.into(),
            ..response(revision)
        }
    }

    fn apply(world: &World, event: &Event) -> World {
        let mut next = world.clone();
        let editor = &world.editor;
        next.editor = match event {
            Event::SignIn(actor) => editor.signed_in(actor.to_string()),
            Event::CheckSession => editor.checking_session(),
            Event::ReleaseCheck => editor.release_session_check(),
            Event::SessionFails => editor.session_failed(&error(ErrorCode::Unauthenticated)),
            Event::Select(script) => editor.select_script(script.to_string()),
            Event::Discard(script) => editor.discard_and_select(script.to_string()),
            Event::Edit(text) => editor.edit(text.to_string()),
            Event::Save => match editor.start_save(format!("op-{}", world.operations)) {
                Some((saving, intent)) => {
                    next.stale_intent = next.intent.replace(intent);
                    next.operations += 1;
                    saving
                }
                None => editor.clone(),
            },
            Event::Retry => match editor.retry() {
                Some((saving, intent)) => {
                    next.stale_intent = next.intent.replace(intent);
                    saving
                }
                None => editor.clone(),
            },
            Event::Ack | Event::AckForOtherCookie => match &world.intent {
                Some(intent) => {
                    let by = match event {
                        Event::Ack => intent.actor.as_str(),
                        _ => "mallory",
                    };
                    let revision = intent.request.expected_revision + 1;
                    editor.saved(intent, head(&intent.script, revision, by))
                }
                None => editor.clone(),
            },
            Event::SaveFails(code) => match &world.intent {
                Some(intent) => editor.save_failed(intent, &error(code.clone())),
                None => editor.clone(),
            },
            Event::OperationIdFails => editor.failed(&error(ErrorCode::Unavailable)),
            Event::Read => match editor.start_read() {
                Some((reading, ticket)) => {
                    next.stale_ticket = next.ticket.replace(ticket);
                    reading
                }
                None => editor.clone(),
            },
            Event::Load { buffered } => match &world.ticket {
                Some(ticket) => editor.loaded_with_buffer(
                    ticket,
                    head(ticket.script(), 5, ticket.actor.as_str()),
                    *buffered,
                ),
                None => editor.clone(),
            },
            Event::ReadFails => match &world.ticket {
                Some(ticket) => editor.read_failed(ticket, &error(ErrorCode::NotFound)),
                None => editor.clone(),
            },
            Event::LoadStale => match &world.stale_ticket {
                Some(ticket) => editor.loaded(ticket, head(ticket.script(), 6, "alice")),
                None => editor.clone(),
            },
            Event::AckStale => match &world.stale_intent {
                Some(intent) => editor.saved(
                    intent,
                    head(
                        &intent.script,
                        intent.request.expected_revision + 1,
                        &intent.actor,
                    ),
                ),
                None => editor.clone(),
            },
            Event::Open => editor.open_revision(response(3)),
            Event::Rebase => editor.rebase(),
        };
        next
    }

    fn holds_read(editor: &Editor, ticket: &ReadTicket) -> bool {
        matches!(&editor.phase,
            Phase::Reading(held) | Phase::CheckingSession(super::Held::Read(held)) if held == ticket)
    }

    fn check_transition(before: &World, event: &Event, after: &World) {
        let (b, a) = (&before.editor, &after.editor);
        let context = || format!("{event:?}\nbefore: {b:?}\nafter: {a:?}");
        // Activity is shown exactly while a request is in flight.
        assert_eq!(
            a.busy(),
            matches!(a.status(), Status::Loading | Status::Saving),
            "{}",
            context()
        );
        assert_eq!(
            a.awaits_reconciliation(),
            a.pending().is_some() && !a.busy(),
            "{}",
            context()
        );
        assert_eq!(
            a.has_unsaved(),
            a.is_dirty() || a.busy() || a.pending().is_some(),
            "{}",
            context()
        );
        // Unsaved text changes only by typing or an explicit discard/open the shell confirmed.
        if b.is_dirty() && !matches!(event, Event::Edit(_) | Event::Discard(_) | Event::Open) {
            assert_eq!(a.draft(), b.draft(), "{}", context());
        }
        // A save intent is never replaced, and only a new save creates one.
        if let (Some(old), Some(new)) = (b.pending(), a.pending()) {
            assert_eq!(old, new, "{}", context());
        }
        if b.pending().is_none() && a.pending().is_some() {
            assert!(matches!(event, Event::Save), "{}", context());
        }
        // A possibly committed save survives everything but its acknowledgement or a definitive
        // rejection of the same operation.
        if b.pending_save().is_some_and(|save| save.ambiguous)
            && !matches!(
                event,
                Event::Ack | Event::AckStale | Event::SaveFails(ErrorCode::InvalidScript)
            )
        {
            assert_eq!(a.pending(), b.pending(), "{}", context());
        }
        // Responses to anything but the owned request are ignored.
        let ticket = match event {
            Event::Load { .. } | Event::ReadFails => before.ticket.as_ref(),
            Event::LoadStale => before.stale_ticket.as_ref(),
            _ => None,
        };
        if ticket.is_some_and(|ticket| !holds_read(b, ticket)) {
            assert_eq!(a, b, "{}", context());
        }
        let intent = match event {
            Event::Ack | Event::AckForOtherCookie | Event::SaveFails(_) => before.intent.as_ref(),
            Event::AckStale => before.stale_intent.as_ref(),
            _ => None,
        };
        if intent.is_some_and(|intent| b.pending() != Some(intent)) {
            assert_eq!(a, b, "{}", context());
        }
        // A request in flight admits no new request and no discard.
        if b.busy() {
            assert!(b.start_save("extra".into()).is_none(), "{}", context());
            assert!(b.start_read().is_none(), "{}", context());
            assert!(b.retry().is_none(), "{}", context());
            assert_eq!(&b.discard_and_select("other".into()), b, "{}", context());
            assert_eq!(&b.open_revision(response(9)), b, "{}", context());
        }
        // Retry resends exactly the pending intent, and only for its own actor.
        if let Some((_, intent)) = b.retry() {
            assert_eq!(Some(&intent), b.pending(), "{}", context());
            assert_eq!(intent.actor, b.actor(), "{}", context());
        }
    }

    #[test]
    fn bounded_exhaustive_event_sequences_preserve_editor_invariants() {
        const DEPTH: usize = 8;
        let mut seen = std::collections::HashSet::new();
        let mut frontier = vec![World::default()];
        let mut transitions = 0usize;
        for _ in 0..DEPTH {
            let mut next_frontier = Vec::new();
            for world in &frontier {
                for event in &EVENTS {
                    let after = apply(world, event);
                    check_transition(world, event, &after);
                    transitions += 1;
                    if seen.insert(format!("{after:?}")) {
                        next_frontier.push(after);
                    }
                }
            }
            frontier = next_frontier;
        }
        // Guard the oracle itself: the search must reach saving, reconciliation and checks.
        let reached = |pattern: &str| seen.iter().any(|state| state.contains(pattern));
        for phase in [
            "Reading(",
            "Saving(",
            "ReconciliationRequired(",
            "CheckingSession(Save(",
        ] {
            assert!(reached(phase), "{phase} unreachable within {DEPTH} events");
        }
        assert!(reached("ambiguous: true"));
        // 483,280 checked transitions over 48,157 distinct worlds at depth 8 when written.
        assert!(
            transitions > 480_000 && seen.len() > 48_000,
            "{transitions}"
        );
    }
}
