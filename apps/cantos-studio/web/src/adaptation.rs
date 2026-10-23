//! Pure host-proposal review state; only human acceptance dispatches a mutation.
use crate::contract::{
    AcceptAdaptationRequest, AdaptationProposal, AdaptationReviewResponse, AdaptationStatus,
    ApiError, ErrorCode, FieldIssue, ImportOutcome, ImportResponse, RevisionResponse,
};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewStatus {
    Idle,
    LoadingRun,
    RunOpened,
    DraftChanged,
    Accepting,
    Accepted,
    LoadingAccepted,
    AcceptedOpened,
    Error(ErrorCode),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdaptationIntent {
    pub actor: String,
    pub run_id: String,
    pub script_id: String,
    pub request: AcceptAdaptationRequest,
}

impl AdaptationIntent {
    pub fn operation_id(&self) -> &str {
        &self.request.operation_id
    }
}

#[derive(Clone, Debug)]
pub struct AdaptationReview {
    pub actor: String,
    pub run_id: String,
    pub stored: Option<Arc<AdaptationReviewResponse>>,
    pub source: Option<Arc<ImportResponse>>,
    pub input_revision: Option<Arc<RevisionResponse>>,
    pub draft: String,
    /// Local acknowledged bytes; the stored accepted export may be normalized differently.
    pub baseline: String,
    pub draft_actor: String,
    pub draft_changed: bool,
    pub reviewed_findings: bool,
    pub accepted: Option<Arc<RevisionResponse>>,
    pub pending: Option<AdaptationIntent>,
    pub ambiguous: bool,
    pub busy: bool,
    pub ticket: u64,
    pub issues: Vec<FieldIssue>,
    pub status: ReviewStatus,
}

impl Default for AdaptationReview {
    fn default() -> Self {
        Self {
            actor: String::new(),
            run_id: String::new(),
            stored: None,
            source: None,
            input_revision: None,
            draft: String::new(),
            baseline: String::new(),
            draft_actor: String::new(),
            draft_changed: false,
            reviewed_findings: false,
            accepted: None,
            pending: None,
            ambiguous: false,
            busy: false,
            ticket: 0,
            issues: vec![],
            status: ReviewStatus::Idle,
        }
    }
}

pub fn is_run_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

pub fn response_id(response: &AdaptationReviewResponse) -> &str {
    let AdaptationReviewResponse::Caller { context } = response;
    &context.id
}
pub fn response_actor(response: &AdaptationReviewResponse) -> &str {
    let AdaptationReviewResponse::Caller { context } = response;
    &context.created_by
}
pub fn response_status(response: &AdaptationReviewResponse) -> AdaptationStatus {
    let AdaptationReviewResponse::Caller { context } = response;
    context.status
}
pub fn response_proposal(response: &AdaptationReviewResponse) -> Option<&AdaptationProposal> {
    let AdaptationReviewResponse::Caller { context } = response;
    context.proposal.as_ref()
}
fn response_source(response: &AdaptationReviewResponse) -> &ImportResponse {
    let AdaptationReviewResponse::Caller { context } = response;
    &context.source
}
fn response_target(response: &AdaptationReviewResponse) -> (&str, u64) {
    let AdaptationReviewResponse::Caller { context } = response;
    (
        &context.request.script_id,
        context.request.expected_revision,
    )
}
fn response_input(response: &AdaptationReviewResponse) -> Option<&RevisionResponse> {
    let AdaptationReviewResponse::Caller { context } = response;
    context.input_revision.as_ref()
}
fn response_accepted(response: &AdaptationReviewResponse) -> Option<&RevisionResponse> {
    let AdaptationReviewResponse::Caller { context } = response;
    context.accepted_revision.as_ref()
}
fn source_is_bound(response: &AdaptationReviewResponse) -> bool {
    let AdaptationReviewResponse::Caller { context } = response;
    let source = &context.source;
    source.id == context.request.source_id
        && source.sha256 == context.request.source_sha256
        && matches!(&source.outcome, ImportOutcome::Parsed { extraction } if extraction.extractor_version == context.request.extractor_version)
}
fn input_is_bound(response: &AdaptationReviewResponse) -> bool {
    let (script, expected) = response_target(response);
    match response_input(response) {
        None => expected == 0,
        Some(revision) => revision.script_id == script && revision.revision == expected,
    }
}

impl AdaptationReview {
    pub fn blocked(&self) -> bool {
        self.busy || self.pending.is_some()
    }

    pub fn signed_in_as(&self, actor: String) -> Self {
        if actor == self.actor {
            return self.clone();
        }
        if !self.draft_changed && self.pending.is_none() {
            // A clean acknowledged run belongs to its old account, including its identifiers.
            // Dirty drafts and unresolved acceptance below keep their original recovery owner.
            return Self {
                actor,
                ticket: self.ticket.saturating_add(1),
                ..Self::default()
            };
        }
        let mut next = self.clone();
        next.actor = actor;
        next.stored = None;
        next.source = None;
        next.input_revision = None;
        next.accepted = None;
        next.reviewed_findings = false;
        next.ticket = next.ticket.saturating_add(1);
        next.busy = false;
        next.issues.clear();
        next.status = if next.pending.is_some() {
            ReviewStatus::Error(ErrorCode::Forbidden)
        } else {
            ReviewStatus::Idle
        };
        next
    }

    pub fn select_run(&self, id: String) -> Self {
        if self.blocked() || self.draft_changed || self.run_id == id {
            return self.clone();
        }
        Self {
            actor: self.actor.clone(),
            run_id: id,
            ticket: self.ticket.saturating_add(1),
            ..Self::default()
        }
    }

    /// Explicit discard/choose-another action; never abandons an unresolved acceptance.
    pub fn new_run(&self) -> Self {
        if self.blocked() {
            return self.clone();
        }
        Self {
            actor: self.actor.clone(),
            ticket: self.ticket.saturating_add(1),
            ..Self::default()
        }
    }

    pub fn edit_draft(&self, draft: String) -> Self {
        if self.draft_actor != self.actor
            || self.stored.as_ref().is_none_or(|stored| {
                !matches!(
                    response_status(stored),
                    AdaptationStatus::Succeeded | AdaptationStatus::Accepted
                )
            })
        {
            return self.clone();
        }
        let mut next = self.clone();
        next.draft = draft;
        next.draft_changed = next.draft != self.baseline;
        next.reviewed_findings = false;
        if !self.blocked() {
            next.status = ReviewStatus::DraftChanged;
        }
        next
    }

    pub fn begin_composition(&self) -> Self {
        let mut next = self.clone();
        if self.draft_actor == self.actor
            && self
                .stored
                .as_ref()
                .is_some_and(|stored| response_proposal(stored).is_some())
        {
            next.draft_changed = true;
            next.reviewed_findings = false;
        }
        next
    }

    pub fn review_findings(&self, reviewed: bool) -> Self {
        let mut next = self.clone();
        if !self.blocked() && self.draft_actor == self.actor {
            next.reviewed_findings = reviewed;
        }
        next
    }

    pub fn can_accept(&self) -> bool {
        !self.blocked()
            && !self.actor.is_empty()
            && self.draft_actor == self.actor
            && self.reviewed_findings
            && !self.draft.trim().is_empty()
            && self.stored.as_ref().is_some_and(|stored| {
                response_actor(stored) == self.actor
                    && response_status(stored) == AdaptationStatus::Succeeded
                    && response_proposal(stored).is_some()
                    && source_is_bound(stored)
                    && input_is_bound(stored)
                    && self.source.as_deref() == Some(response_source(stored))
                    && self.input_revision.as_deref() == response_input(stored)
            })
    }

    pub fn start_accept(&self, operation_id: String) -> Option<(Self, AdaptationIntent)> {
        if !self.can_accept() {
            return None;
        }
        let stored = self.stored.as_ref()?;
        let (script, expected_revision) = response_target(stored);
        let intent = AdaptationIntent {
            actor: self.actor.clone(),
            run_id: response_id(stored).into(),
            script_id: script.into(),
            request: AcceptAdaptationRequest {
                operation_id,
                expected_revision,
                script_json: self.draft.clone(),
                reviewed_findings: true,
            },
        };
        Some((self.sending(intent.clone()), intent))
    }

    fn sending(&self, intent: AdaptationIntent) -> Self {
        let mut next = self.clone();
        next.pending = Some(intent);
        next.busy = true;
        next.issues.clear();
        next.status = ReviewStatus::Accepting;
        next
    }
    pub fn can_retry(&self) -> bool {
        !self.busy
            && self
                .pending
                .as_ref()
                .is_some_and(|intent| intent.actor == self.actor)
    }
    pub fn retry(&self) -> Option<(Self, AdaptationIntent)> {
        if !self.can_retry() {
            return None;
        }
        let intent = self.pending.clone()?;
        Some((self.sending(intent.clone()), intent))
    }

    pub fn accepted_result(&self, intent: &AdaptationIntent, revision: RevisionResponse) -> Self {
        if self.pending.as_ref() != Some(intent) {
            return self.clone();
        }
        let mut next = self.clone();
        next.busy = false;
        if revision.accepted_by != intent.actor
            || revision.script_id != intent.script_id
            || revision.revision != intent.request.expected_revision.saturating_add(1)
        {
            next.ambiguous = true;
            next.status = ReviewStatus::Error(if revision.accepted_by == intent.actor {
                ErrorCode::CorruptRevision
            } else {
                ErrorCode::Forbidden
            });
            return next;
        }
        if self.actor != intent.actor {
            next.ambiguous = true;
            next.status = ReviewStatus::Error(ErrorCode::Forbidden);
            return next;
        }
        if self
            .stored
            .as_ref()
            .is_some_and(|stored| response_id(stored) != intent.run_id)
        {
            next.ambiguous = true;
            next.status = ReviewStatus::Error(ErrorCode::CorruptRevision);
            return next;
        }
        next.pending = None;
        next.ambiguous = false;
        next.baseline = intent.request.script_json.clone();
        next.draft_changed = next.draft != next.baseline;
        if let Some(stored) = &self.stored {
            let mut response = stored.as_ref().clone();
            let AdaptationReviewResponse::Caller { context } = &mut response;
            context.status = AdaptationStatus::Accepted;
            context.accepted_revision = Some(revision.clone());
            next.stored = Some(Arc::new(response));
        }
        next.accepted = Some(Arc::new(revision));
        next.status = ReviewStatus::Accepted;
        next
    }

    pub fn can_read(&self) -> bool {
        !self.blocked() && !self.actor.is_empty() && is_run_id(&self.run_id)
    }
    pub fn start_read(&self) -> Option<Self> {
        if !self.can_read() {
            return None;
        }
        let mut next = self.clone();
        next.busy = true;
        next.ticket = self.ticket.checked_add(1)?;
        next.issues.clear();
        next.status = ReviewStatus::LoadingRun;
        Some(next)
    }

    pub fn loaded(&self, ticket: u64, actor: &str, response: AdaptationReviewResponse) -> Self {
        self.loaded_with_buffer(ticket, actor, response, false)
    }

    /// A browser-owned native buffer must survive a read even while the reducer is clean.
    pub fn loaded_with_buffer(
        &self,
        ticket: u64,
        actor: &str,
        response: AdaptationReviewResponse,
        buffer_pending: bool,
    ) -> Self {
        if ticket != self.ticket || actor != self.actor || response_id(&response) != self.run_id {
            return self.clone();
        }
        let mut next = self.clone();
        next.busy = false;
        if response_actor(&response) != actor || response_source(&response).imported_by != actor {
            next.status = ReviewStatus::Error(ErrorCode::Forbidden);
            return next;
        }
        if !source_is_bound(&response)
            || !input_is_bound(&response)
            || !is_run_id(response_target(&response).0)
        {
            next.status = ReviewStatus::Error(ErrorCode::CorruptRevision);
            return next;
        }
        let (script, expected) = response_target(&response);
        if response_accepted(&response).is_some_and(|revision| {
            revision.script_id != script
                || revision.revision != expected.saturating_add(1)
                || revision.accepted_by != actor
        }) || (response_status(&response) == AdaptationStatus::Accepted)
            != response_accepted(&response).is_some()
            || (response_status(&response) == AdaptationStatus::Succeeded
                && response_proposal(&response).is_none())
        {
            next.status = ReviewStatus::Error(ErrorCode::CorruptRevision);
            return next;
        }
        let AdaptationReviewResponse::Caller { context } = &response;
        if context.latest_submission.as_ref().is_some_and(|receipt| {
            receipt.run_id != context.id
                || receipt.context_digest != context.context_digest
                || receipt.submitted_by != actor
        }) {
            next.status = ReviewStatus::Error(ErrorCode::CorruptRevision);
            return next;
        }
        // A different proposal or pinned input cannot inherit the previous acknowledgement.
        if self.stored.as_ref().is_some_and(|stored| {
            response_proposal(stored) != response_proposal(&response)
                || response_target(stored) != response_target(&response)
                || response_source(stored) != response_source(&response)
                || response_input(stored) != response_input(&response)
        }) {
            next.reviewed_findings = false;
        }
        if !buffer_pending && (!self.draft_changed || self.draft_actor != self.actor) {
            next.draft = response_accepted(&response)
                .map(|accepted| accepted.script_json.clone())
                .or_else(|| {
                    response_proposal(&response).map(|proposal| proposal.script_json.clone())
                })
                .unwrap_or_default();
            next.baseline = next.draft.clone();
            next.draft_actor = self.actor.clone();
            next.draft_changed = false;
        }
        if buffer_pending {
            // The shell owns this buffer until commit or Escape; do not invent byte changes.
            next.reviewed_findings = false;
        }
        next.source = Some(Arc::new(response_source(&response).clone()));
        next.input_revision = response_input(&response).cloned().map(Arc::new);
        next.accepted = response_accepted(&response).cloned().map(Arc::new);
        next.status = if buffer_pending {
            ReviewStatus::DraftChanged
        } else if response_status(&response) == AdaptationStatus::Accepted {
            ReviewStatus::Accepted
        } else {
            ReviewStatus::RunOpened
        };
        next.stored = Some(Arc::new(response));
        next
    }

    pub fn failed(&self, error: &ApiError) -> Self {
        let mut next = self.clone();
        next.busy = false;
        next.issues = error.issues.clone();
        next.status = ReviewStatus::Error(error.code.clone());
        if matches!(
            error.code,
            ErrorCode::Unavailable | ErrorCode::CorruptRevision
        ) {
            next.ambiguous = next.pending.is_some();
        } else if !(self.ambiguous
            && matches!(
                error.code,
                ErrorCode::Unauthenticated | ErrorCode::Forbidden | ErrorCode::NotFound
            ))
        {
            next.pending = None;
            next.ambiguous = false;
        }
        next
    }
    pub fn read_failed(&self, ticket: u64, actor: &str, error: &ApiError) -> Self {
        if ticket != self.ticket || actor != self.actor {
            return self.clone();
        }
        self.failed(error)
    }
    pub fn start_accepted_read(&self) -> Option<Self> {
        if self.blocked() || self.actor.is_empty() || self.accepted.is_none() {
            return None;
        }
        let mut next = self.clone();
        next.busy = true;
        next.ticket = next.ticket.checked_add(1)?;
        next.status = ReviewStatus::LoadingAccepted;
        Some(next)
    }
    pub fn accepted_loaded(&self, ticket: u64, actor: &str, revision: RevisionResponse) -> Self {
        self.accepted_loaded_with_buffer(ticket, actor, revision, false)
    }

    pub fn accepted_loaded_with_buffer(
        &self,
        ticket: u64,
        actor: &str,
        revision: RevisionResponse,
        buffer_pending: bool,
    ) -> Self {
        if ticket != self.ticket || actor != self.actor {
            return self.clone();
        }
        let Some(accepted) = self.accepted.as_ref() else {
            return self.clone();
        };
        if &revision != accepted.as_ref() {
            return self.read_failed(
                ticket,
                actor,
                &ApiError {
                    code: ErrorCode::CorruptRevision,
                    current_revision: None,
                    issues: vec![],
                },
            );
        }
        let mut next = self.clone();
        next.busy = false;
        next.status = if buffer_pending {
            next.reviewed_findings = false;
            ReviewStatus::DraftChanged
        } else {
            ReviewStatus::AcceptedOpened
        };
        next
    }
}

#[cfg(test)]
mod clean_session_tests {
    use super::*;

    #[test]
    fn changing_a_clean_account_clears_private_adaptation_run_and_bytes() {
        let clean = AdaptationReview {
            actor: "alice".into(),
            run_id: "00000000-0000-4000-8000-000000000002".into(),
            draft: "private acknowledged proposal".into(),
            baseline: "private acknowledged proposal".into(),
            draft_actor: "alice".into(),
            status: ReviewStatus::RunOpened,
            ..AdaptationReview::default()
        };
        let reading = clean.start_read().unwrap();
        let bob = reading.signed_in_as("bob".into());
        assert_eq!(bob.actor, "bob");
        assert!(bob.run_id.is_empty());
        assert!(bob.draft.is_empty());
        assert!(bob.baseline.is_empty());
        assert!(bob.draft_actor.is_empty());
        assert!(bob.stored.is_none());
        assert!(bob.source.is_none());
        assert!(bob.input_revision.is_none());
        assert!(bob.accepted.is_none());
        assert!(bob.issues.is_empty());
        assert_eq!(bob.ticket, reading.ticket + 1);
        assert!(!bob.busy);
        assert_eq!(bob.status, ReviewStatus::Idle);
    }
}

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::ScriptAdaptation;
#[cfg(test)]
mod tests;
