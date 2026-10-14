//! Pure revision/source inspection and immutable owner-review intents.
use crate::editor::Editor;
use cantos_contracts::{
    ApiError, ErrorCode, HistoryResponse, ImportResponse, ReviewRequest, ReviewResponse,
    RevisionResponse, SourceResponse,
};
use std::collections::BTreeSet;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::RevisionInspector;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub actor: String,
    pub script: String,
    pub saved: Option<(u64, String)>,
}

impl Context {
    fn from_editor(editor: &Editor) -> Self {
        Self {
            actor: editor.actor().to_owned(),
            script: editor.script().to_owned(),
            saved: editor
                .remote()
                .filter(|remote| remote.script_id == editor.script())
                .map(|remote| (remote.revision, remote.export_digest.clone())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewSnapshot {
    pub actor: String,
    pub script: String,
    pub revision: u64,
    pub content_digest: String,
    pub export_digest: String,
}

impl ReviewSnapshot {
    fn from_editor(editor: &Editor) -> Option<Self> {
        if editor.actor().is_empty()
            || editor.script().is_empty()
            || editor.base() == 0
            || editor.is_dirty()
            || editor.busy()
            || editor.pending().is_some()
            || !editor.can_edit()
        {
            return None;
        }
        let remote = editor.remote()?;
        if remote.script_id != editor.script() || remote.revision != editor.base() {
            return None;
        }
        Some(Self {
            actor: editor.actor().to_owned(),
            script: editor.script().to_owned(),
            revision: remote.revision,
            content_digest: remote.content_digest.clone(),
            export_digest: remote.export_digest.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewIntent {
    pub snapshot: ReviewSnapshot,
    pub request: ReviewRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadKind {
    History { after: u64 },
    Revision { revision: u64 },
    Sources { ids: Vec<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadTicket {
    pub context: Context,
    pub sequence: u64,
    pub kind: ReadKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreservedSource {
    Text(SourceResponse),
    Imported(Box<ImportResponse>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRead {
    pub id: String,
    pub result: Result<PreservedSource, ErrorCode>,
    /// The script-scoped text route rejects binary originals; import access is checked separately.
    pub import_fallback: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inspector {
    pub context: Context,
    pub sequence: u64,
    pub reading: Option<ReadTicket>,
    pub history: Option<HistoryResponse>,
    pub viewed: Option<RevisionResponse>,
    pub sources: Vec<SourceRead>,
    pub confirmation: Option<ReviewSnapshot>,
    pub pending: Option<ReviewIntent>,
    pub sending_review: bool,
    pub ambiguous: bool,
    pub reviewed: Option<ReviewResponse>,
    pub error: Option<ErrorCode>,
}

impl Inspector {
    /// A buffered field or IME composition is unsaved even before Editor receives its bytes.
    pub fn observe_workspace(&self, editor: &Editor, draft_activity: bool) -> Self {
        let mut next = self.observe(editor);
        if draft_activity {
            next.confirmation = None;
        }
        next
    }

    /// Read data belongs to the exact session/script/head. Pending writes cannot be abandoned.
    pub fn observe(&self, editor: &Editor) -> Self {
        let context = Context::from_editor(editor);
        let mut next = self.clone();
        if self.context != context {
            next.context = context;
            next.sequence = self.sequence.saturating_add(1);
            next.reading = None;
            next.history = None;
            next.viewed = None;
            next.sources.clear();
            next.reviewed = None;
            next.error = None;
            if self.pending.is_some() {
                // The old response must remain invisible; retry will reconcile its exact intent.
                next.sending_review = false;
                next.ambiguous = true;
            }
        }
        if next.confirmation != ReviewSnapshot::from_editor(editor) {
            next.confirmation = None;
        }
        next
    }

    pub fn has_activity(&self) -> bool {
        self.pending.is_some() || self.reading.is_some()
    }

    pub fn can_read(&self, editor: &Editor) -> bool {
        !self.has_activity()
            && !editor.busy()
            && editor.pending().is_none()
            && !editor.actor().is_empty()
            && !editor.script().is_empty()
            && self.context == Context::from_editor(editor)
    }

    pub fn start_read(&self, editor: &Editor, kind: ReadKind) -> Option<(Self, ReadTicket)> {
        if !self.can_read(editor) {
            return None;
        }
        let valid = match &kind {
            ReadKind::History { after } => *after <= i64::MAX as u64,
            ReadKind::Revision { revision } => *revision > 0 && *revision <= i64::MAX as u64,
            ReadKind::Sources { ids } => {
                !ids.is_empty()
                    && editor
                        .remote()
                        .is_some_and(|remote| source_ids(&remote.script_json) == *ids)
            }
        };
        if !valid {
            return None;
        }
        let ticket = ReadTicket {
            context: self.context.clone(),
            sequence: self.sequence.checked_add(1)?,
            kind,
        };
        let mut next = self.clone();
        next.sequence = ticket.sequence;
        next.reading = Some(ticket.clone());
        next.error = None;
        Some((next, ticket))
    }

    fn accepts_read(&self, ticket: &ReadTicket) -> bool {
        self.reading.as_ref() == Some(ticket) && self.context == ticket.context
    }

    pub fn history_loaded(&self, ticket: &ReadTicket, history: HistoryResponse) -> Self {
        let ReadKind::History { after } = ticket.kind else {
            return self.clone();
        };
        if !self.accepts_read(ticket) {
            return self.clone();
        }
        if history
            .revisions
            .iter()
            .any(|revision| revision.script_id != self.context.script || revision.revision <= after)
            || history
                .reviews
                .iter()
                .any(|review| review.script_id != self.context.script || review.revision <= after)
            || history.next_after.is_some_and(|cursor| cursor <= after)
        {
            return self.read_failed(ticket, ErrorCode::CorruptRevision);
        }
        let mut next = self.clone();
        next.reading = None;
        next.history = Some(history);
        next
    }

    pub fn revision_loaded(&self, ticket: &ReadTicket, revision: RevisionResponse) -> Self {
        if !self.accepts_read(ticket) {
            return self.clone();
        }
        if ticket.kind
            != (ReadKind::Revision {
                revision: revision.revision,
            })
            || revision.script_id != self.context.script
        {
            return self.read_failed(ticket, ErrorCode::CorruptRevision);
        }
        let mut next = self.clone();
        next.reading = None;
        next.viewed = Some(revision);
        next
    }

    pub fn sources_loaded(&self, ticket: &ReadTicket, sources: Vec<SourceRead>) -> Self {
        let ReadKind::Sources { ids } = &ticket.kind else {
            return self.clone();
        };
        if !self.accepts_read(ticket) {
            return self.clone();
        }
        if sources.iter().map(|source| &source.id).ne(ids.iter())
            || sources.iter().any(|source| match &source.result {
                Ok(PreservedSource::Text(response)) => response.id != source.id,
                Ok(PreservedSource::Imported(response)) => {
                    response.id != source.id || response.imported_by != self.context.actor
                }
                Err(_) => false,
            })
        {
            return self.read_failed(ticket, ErrorCode::CorruptRevision);
        }
        let mut next = self.clone();
        next.reading = None;
        next.sources = sources;
        next
    }

    pub fn read_failed(&self, ticket: &ReadTicket, error: ErrorCode) -> Self {
        if !self.accepts_read(ticket) {
            return self.clone();
        }
        let mut next = self.clone();
        next.reading = None;
        next.error = Some(error);
        next
    }

    pub fn confirm(&self, editor: &Editor, checked: bool) -> Self {
        let mut next = self.observe(editor);
        next.confirmation = if checked && !next.has_activity() {
            ReviewSnapshot::from_editor(editor)
        } else {
            None
        };
        next
    }

    pub fn can_review(&self, editor: &Editor) -> bool {
        !self.has_activity()
            && self.confirmation.is_some()
            && self.confirmation == ReviewSnapshot::from_editor(editor)
            && self.context == Context::from_editor(editor)
    }

    pub fn start_review(&self, editor: &Editor, operation: String) -> Option<(Self, ReviewIntent)> {
        if !self.can_review(editor) {
            return None;
        }
        let snapshot = self.confirmation.clone()?;
        let intent = ReviewIntent {
            request: ReviewRequest {
                operation_id: operation,
                revision: snapshot.revision,
            },
            snapshot,
        };
        let mut next = self.clone();
        next.pending = Some(intent.clone());
        next.sending_review = true;
        next.ambiguous = false;
        next.reviewed = None;
        next.error = None;
        Some((next, intent))
    }

    pub fn can_retry(&self) -> bool {
        !self.sending_review
            && self.reading.is_none()
            && self.pending.as_ref().is_some_and(|intent| {
                intent.snapshot.actor == self.context.actor
                    && intent.snapshot.script == self.context.script
            })
    }

    pub fn retry(&self) -> Option<(Self, ReviewIntent)> {
        if !self.can_retry() {
            return None;
        }
        let intent = self.pending.clone()?;
        let mut next = self.clone();
        next.sending_review = true;
        next.error = None;
        Some((next, intent))
    }

    fn accepts_review(&self, intent: &ReviewIntent) -> bool {
        self.sending_review
            && self.pending.as_ref() == Some(intent)
            && intent.snapshot.actor == self.context.actor
            && intent.snapshot.script == self.context.script
    }

    pub fn review_loaded(&self, intent: &ReviewIntent, review: ReviewResponse) -> Self {
        if !self.accepts_review(intent) {
            return self.clone();
        }
        if review.script_id != intent.snapshot.script
            || review.reviewed_by != intent.snapshot.actor
            || review.revision != intent.request.revision
            || review.content_digest != intent.snapshot.content_digest
            || review.export_digest != intent.snapshot.export_digest
        {
            return self.review_failed(intent, ErrorCode::CorruptRevision);
        }
        let mut next = self.clone();
        // The API reuses the first review row for a head. A later operation aliases that row,
        // whose immutable operation_id may differ from this request's retry identity.
        next.pending = None;
        next.sending_review = false;
        next.ambiguous = false;
        next.confirmation = None;
        next.reviewed = Some(review);
        next.error = None;
        next
    }

    pub fn review_failed(&self, intent: &ReviewIntent, error: ErrorCode) -> Self {
        if !self.accepts_review(intent) {
            return self.clone();
        }
        let mut next = self.clone();
        next.sending_review = false;
        next.error = Some(error.clone());
        next.confirmation = None;
        if matches!(error, ErrorCode::Unavailable | ErrorCode::CorruptRevision) {
            next.ambiguous = true;
        } else if !(next.ambiguous
            && matches!(
                error,
                ErrorCode::Unauthenticated | ErrorCode::Forbidden | ErrorCode::NotFound
            ))
        {
            next.pending = None;
            next.ambiguous = false;
        }
        next
    }

    pub fn local_error(&self, error: &ApiError) -> Self {
        let mut next = self.clone();
        next.error = Some(error.code.clone());
        next
    }
}

/// Only immutable saved exports supply script-scoped source links; draft edits grant no access.
pub fn source_ids(script_json: &str) -> Vec<String> {
    let Ok(document) = serde_json::from_str::<serde_json::Value>(script_json) else {
        return vec![];
    };
    let Some(provenance) = document
        .get("provenance")
        .and_then(|value| value.as_array())
    else {
        return vec![];
    };
    provenance
        .iter()
        .filter_map(|source| source.get("source_record_id")?.as_str())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// This chooses a read adapter, never grants import permission or bypasses a denied source read.
pub fn should_read_import(id: &str, error: &ErrorCode) -> bool {
    id.starts_with("src_") && *error == ErrorCode::InvalidRequest
}

#[cfg(test)]
mod tests {
    use super::{
        should_read_import, source_ids, Inspector, PreservedSource, ReadKind, ReviewIntent,
        SourceRead,
    };
    use crate::editor::Editor;
    use cantos_contracts::{
        ErrorCode, HistoryResponse, ReviewResponse, RevisionResponse, SourceResponse,
    };

    fn revision(version: u64) -> RevisionResponse {
        RevisionResponse {
            script_id: "script".into(),
            revision: version,
            accepted_by: "alice".into(),
            accepted_at: "2026-10-10T00:00:00Z".into(),
            content_digest: format!("content-{version}"),
            export_digest: format!("export-{version}"),
            script_json: r#"{"provenance":[{"source_record_id":"source-a"}]}"#.into(),
        }
    }

    /// `actor` has opened `response` cleanly, built only from public Editor transitions.
    fn opened(actor: &str, response: RevisionResponse) -> Editor {
        Editor::default()
            .signed_in(actor.into())
            .open_revision(response)
    }

    fn editor() -> Editor {
        opened("alice", revision(3))
    }

    fn sending() -> (Editor, Inspector, ReviewIntent) {
        let editor = editor();
        let (inspector, intent) = Inspector::default()
            .observe(&editor)
            .confirm(&editor, true)
            .start_review(&editor, "fixed-operation".into())
            .unwrap();
        (editor, inspector, intent)
    }

    fn review(intent: &ReviewIntent) -> ReviewResponse {
        ReviewResponse {
            id: "review-1".into(),
            script_id: intent.snapshot.script.clone(),
            revision: intent.request.revision,
            operation_id: intent.request.operation_id.clone(),
            reviewed_by: intent.snapshot.actor.clone(),
            reviewed_at: "2026-10-10T00:00:00Z".into(),
            content_digest: intent.snapshot.content_digest.clone(),
            export_digest: intent.snapshot.export_digest.clone(),
        }
    }

    #[test]
    fn review_requires_explicit_confirmation_of_clean_current_saved_head() {
        let editor = editor();
        let inspector = Inspector::default().observe(&editor);
        assert!(!inspector.can_review(&editor));
        let confirmed = inspector.confirm(&editor, true);
        assert!(confirmed.can_review(&editor));
        let dirty = editor.edit("new Vietnamese dialogue".into());
        assert!(confirmed.observe(&dirty).confirmation.is_none());
        assert!(!confirmed.can_review(&dirty));
        let buffered = confirmed.observe_workspace(&editor, true);
        assert!(buffered.confirmation.is_none());
        assert!(!buffered.can_review(&editor));
        assert!(buffered
            .observe_workspace(&editor, false)
            .confirmation
            .is_none());
        // A clean draft whose base trails the displayed head (read while the buffer was active).
        let (reading, ticket) = opened("alice", revision(2)).start_read().unwrap();
        let outdated = reading.loaded_with_buffer(&ticket, revision(3), true);
        assert!(!outdated.is_dirty());
        assert_eq!(
            (outdated.base(), outdated.remote().unwrap().revision),
            (2, 3)
        );
        assert!(confirmed.confirm(&outdated, true).confirmation.is_none());
        let (pending, _) = editor.start_read().unwrap();
        assert!(pending.busy());
        assert!(confirmed.confirm(&pending, true).confirmation.is_none());
    }

    #[test]
    fn timeout_retry_retains_exact_operation_actor_and_revision_and_blocks_other_actions() {
        let (editor, sending, intent) = sending();
        let failed = sending.review_failed(&intent, ErrorCode::Unavailable);
        assert!(failed.has_activity());
        assert!(failed
            .start_review(&editor, "new-operation".into())
            .is_none());
        assert!(failed
            .start_read(&editor, ReadKind::History { after: 0 })
            .is_none());
        let (retrying, retry) = failed.retry().unwrap();
        assert_eq!(retry, intent);
        assert!(retrying.retry().is_none());
        let completed = retrying.review_loaded(&retry, review(&retry));
        assert_eq!(completed.reviewed.as_ref().unwrap().revision, 3);
        assert!(!completed.has_activity());
        assert!(!completed.can_review(&editor));
        assert!(completed
            .review_loaded(&retry, review(&retry))
            .pending
            .is_none());
    }

    #[test]
    fn later_permission_denial_does_not_abandon_a_previously_ambiguous_review() {
        let (_, sending, intent) = sending();
        let (retrying, _) = sending
            .review_failed(&intent, ErrorCode::Unavailable)
            .retry()
            .unwrap();
        let denied = retrying.review_failed(&intent, ErrorCode::Forbidden);
        assert_eq!(denied.pending.as_ref(), Some(&intent));
        assert!(denied.ambiguous);
        assert_eq!(denied.error, Some(ErrorCode::Forbidden));
        let definite = sending.review_failed(&intent, ErrorCode::Forbidden);
        assert!(definite.pending.is_none());
        assert!(definite.reviewed.is_none());
    }

    #[test]
    fn repeated_head_review_accepts_the_first_immutable_record_without_another_row() {
        let (_, sending, intent) = sending();
        let mut existing = review(&intent);
        existing.operation_id = "first-operation-for-this-head".into();
        let completed = sending.review_loaded(&intent, existing.clone());
        assert_eq!(completed.reviewed, Some(existing));
        assert!(completed.pending.is_none());
        assert!(!completed.ambiguous);
    }

    #[test]
    fn changed_actor_or_script_hides_old_data_and_keeps_review_for_original_actor() {
        let (editor, sending, intent) = sending();
        let other = opened("bob", revision(3));
        let changed = sending
            .observe(&other)
            .review_loaded(&intent, review(&intent));
        assert!(changed.reviewed.is_none());
        assert_eq!(changed.pending.as_ref(), Some(&intent));
        assert!(changed.retry().is_none());
        let restored = changed.observe(&editor);
        assert!(restored.can_retry());
        let other = editor.discard_and_select("other-script".into());
        assert!(sending.observe(&other).retry().is_none());
    }

    #[test]
    fn changed_cookie_or_corrupt_response_never_confirms_the_wrong_review() {
        let (_, sending, intent) = sending();
        let mut wrong = review(&intent);
        wrong.reviewed_by = "bob".into();
        let failed = sending.review_loaded(&intent, wrong);
        assert_eq!(failed.error, Some(ErrorCode::CorruptRevision));
        assert_eq!(failed.pending.as_ref(), Some(&intent));
        assert!(failed.reviewed.is_none());
        let mut wrong = review(&intent);
        wrong.export_digest = "different-export".into();
        assert_eq!(
            sending.review_loaded(&intent, wrong).error,
            Some(ErrorCode::CorruptRevision)
        );
    }

    #[test]
    fn stale_owner_review_clears_intent_and_requires_another_explicit_review() {
        let (editor, sending, intent) = sending();
        let stale = sending.review_failed(&intent, ErrorCode::StaleRevision);
        assert!(stale.pending.is_none());
        assert!(stale.confirmation.is_none());
        assert!(!stale.can_review(&editor));
        assert_eq!(stale.error, Some(ErrorCode::StaleRevision));
    }

    #[test]
    fn late_reads_cannot_cross_actor_script_head_or_request_tickets() {
        let editor = editor();
        let inspector = Inspector::default().observe(&editor);
        let (reading, ticket) = inspector
            .start_read(&editor, ReadKind::Revision { revision: 1 })
            .unwrap();
        let exact = reading.revision_loaded(&ticket, revision(1));
        assert_eq!(exact.viewed.as_ref().unwrap().revision, 1);
        assert!(!editor.is_dirty());
        for (actor, script, version) in [
            ("bob", "script", 3),
            ("alice", "other", 3),
            ("alice", "script", 4),
        ] {
            let changed = opened(actor, revision(version));
            let changed = if script == "script" {
                changed
            } else {
                changed.discard_and_select(script.into())
            };
            assert!(reading
                .observe(&changed)
                .revision_loaded(&ticket, revision(1))
                .viewed
                .is_none());
        }
        let mut wrong = ticket.clone();
        wrong.sequence += 1;
        assert!(reading
            .revision_loaded(&wrong, revision(1))
            .viewed
            .is_none());
        assert_eq!(
            reading.revision_loaded(&ticket, revision(2)).error,
            Some(ErrorCode::CorruptRevision)
        );
    }

    #[test]
    fn history_page_and_source_results_are_bound_to_the_requested_scope() {
        let editor = editor();
        let inspector = Inspector::default().observe(&editor);
        let (reading, ticket) = inspector
            .start_read(&editor, ReadKind::History { after: 3 })
            .unwrap();
        let bad = HistoryResponse {
            revisions: vec![],
            reviews: vec![],
            next_after: Some(3),
        };
        assert_eq!(
            reading.history_loaded(&ticket, bad).error,
            Some(ErrorCode::CorruptRevision)
        );
        let (reading, ticket) = inspector
            .start_read(
                &editor,
                ReadKind::Sources {
                    ids: vec!["source-a".into()],
                },
            )
            .unwrap();
        let source = SourceRead {
            id: "source-a".into(),
            result: Ok(PreservedSource::Text(SourceResponse {
                id: "different".into(),
                reference: "synthetic".into(),
                original_text: "Người dẫn chuyện".into(),
                sha256: "checksum".into(),
                recorded_at: "now".into(),
            })),
            import_fallback: false,
        };
        assert_eq!(
            reading.sources_loaded(&ticket, vec![source]).error,
            Some(ErrorCode::CorruptRevision)
        );
        let denied = SourceRead {
            id: "source-a".into(),
            result: Err(ErrorCode::Forbidden),
            import_fallback: false,
        };
        assert_eq!(
            reading
                .sources_loaded(&ticket, vec![denied.clone()])
                .sources,
            vec![denied]
        );
    }

    #[test]
    fn source_links_are_deduplicated_without_changing_the_preserved_document() {
        let text = r#"{"provenance":[{"source_record_id":"b"},{"source_record_id":"a"},{"source_record_id":"b"},{"unknown":"retained"}]}"#;
        assert_eq!(source_ids(text), vec!["a", "b"]);
        assert!(source_ids("invalid JSON").is_empty());
        assert!(source_ids(r#"{"provenance":null}"#).is_empty());
    }

    #[test]
    fn binary_fallback_is_selected_only_after_text_contract_rejects_an_import_id() {
        assert!(should_read_import(
            "src_example",
            &ErrorCode::InvalidRequest
        ));
        assert!(!should_read_import(
            "legacy-source",
            &ErrorCode::InvalidRequest
        ));
        for error in [
            ErrorCode::Forbidden,
            ErrorCode::Unauthenticated,
            ErrorCode::NotFound,
            ErrorCode::Unavailable,
            ErrorCode::CorruptRevision,
        ] {
            assert!(!should_read_import("src_example", &error));
        }
    }

    #[test]
    fn imported_source_receipt_cannot_cross_the_requested_id_or_signed_actor() {
        use cantos_contracts::{ImportFormat, ImportMetadata, ImportOutcome, ImportProblem};
        let document = r#"{"provenance":[{"source_record_id":"src_example"}]}"#;
        let mut head = revision(3);
        head.script_json = document.into();
        let editor = opened("alice", head);
        let inspector = Inspector::default().observe(&editor);
        let (reading, ticket) = inspector
            .start_read(
                &editor,
                ReadKind::Sources {
                    ids: vec!["src_example".into()],
                },
            )
            .unwrap();
        let receipt = cantos_contracts::ImportResponse {
            id: "src_example".into(),
            imported_by: "alice".into(),
            recorded_at: "now".into(),
            sha256: "checksum".into(),
            byte_len: 42,
            metadata: ImportMetadata {
                operation_id: "synthetic-operation".into(),
                file_name: "synthetic.docx".into(),
                format: ImportFormat::Docx,
                reference: "synthetic source".into(),
                rights_holder: None,
                permission_evidence: None,
                usage_scope: None,
            },
            original_text: None,
            outcome: ImportOutcome::Failed {
                error: ImportProblem {
                    code: "synthetic-failure".into(),
                    offset: None,
                },
            },
        };
        let loaded = SourceRead {
            id: "src_example".into(),
            result: Ok(PreservedSource::Imported(Box::new(receipt.clone()))),
            import_fallback: true,
        };
        assert_eq!(
            reading
                .sources_loaded(&ticket, vec![loaded.clone()])
                .sources,
            vec![loaded]
        );
        for (id, actor) in [("wrong-id", "alice"), ("src_example", "bob")] {
            let mut other = receipt.clone();
            other.id = id.into();
            other.imported_by = actor.into();
            let loaded = SourceRead {
                id: "src_example".into(),
                result: Ok(PreservedSource::Imported(Box::new(other))),
                import_fallback: true,
            };
            assert_eq!(
                reading.sources_loaded(&ticket, vec![loaded]).error,
                Some(ErrorCode::CorruptRevision)
            );
        }
        let denied = SourceRead {
            id: "src_example".into(),
            result: Err(ErrorCode::NotFound),
            import_fallback: true,
        };
        assert_eq!(
            reading
                .sources_loaded(&ticket, vec![denied.clone()])
                .sources,
            vec![denied]
        );
    }
}
