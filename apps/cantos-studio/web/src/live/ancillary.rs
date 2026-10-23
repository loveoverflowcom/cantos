//! Exact browser-memory production forms; lookup controls carry no unsaved write intent.
use super::{Command, Reply};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BriefForm {
    pub notes: String,
    pub profile: String,
    pub permitted_use: String,
    pub attribution: String,
    pub naming: String,
    pub qa: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProducerForm {
    pub producer: String,
    pub provenance: String,
    pub method: String,
    pub quality: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewForm {
    pub kind: String,
    pub decision: String,
    pub notes: String,
    pub evidence: String,
    pub confirmed: bool,
}
impl Default for ReviewForm {
    fn default() -> Self {
        Self {
            kind: "qc".into(),
            decision: "rejected".into(),
            notes: String::new(),
            evidence: String::new(),
            confirmed: false,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Form {
    pub brief: BriefForm,
    pub producer: ProducerForm,
    pub review: ReviewForm,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Draft {
    current: Form,
    saved: Form,
    submitted: Option<(Intent, Section)>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Intent {
    Brief(String),
    Prepare(String),
    Review(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Section {
    Brief(BriefForm),
    Producer(ProducerForm),
    Review(ReviewForm),
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AncillaryDrafts {
    selected: String,
    drafts: BTreeMap<String, Draft>,
}
fn intent(command: &Command) -> Option<(&str, Intent)> {
    Some(match command {
        Command::CreateBrief { unit, request } => {
            (unit, Intent::Brief(request.operation_id.clone()))
        }
        Command::Prepare { unit, request } => (unit, Intent::Prepare(request.operation_id.clone())),
        Command::Review { unit, request, .. } => {
            (unit, Intent::Review(request.operation_id.clone()))
        }
        _ => return None,
    })
}
impl AncillaryDrafts {
    pub fn current(&self) -> Form {
        self.drafts
            .get(&self.selected)
            .map(|d| d.current.clone())
            .unwrap_or_default()
    }
    pub fn open(&mut self, unit: &str) -> Form {
        self.selected = unit.into();
        if let Some(draft) = self.drafts.get_mut(unit) {
            // Consent binds a loaded rendition, not a portable form or navigation history.
            draft.current.review.confirmed = false;
            draft.saved.review.confirmed = false;
        }
        self.current()
    }
    pub fn current_dirty(&self) -> bool {
        self.drafts
            .get(&self.selected)
            .is_some_and(|d| d.current != d.saved)
    }
    pub fn has_unsaved(&self) -> bool {
        self.drafts.values().any(|d| d.current != d.saved)
    }
    /// Only editable production fields enter the form; lookup and child-panel events are ignored.
    pub fn input(&mut self, id: &str, form: Form) {
        if matches!(
            id,
            "live-direction"
                | "live-profile"
                | "live-use"
                | "live-attribution"
                | "live-naming"
                | "live-qa"
                | "live-producer"
                | "live-producer-provenance"
                | "live-method"
                | "live-quality"
                | "live-review-kind"
                | "live-decision"
                | "live-review-notes"
                | "live-evidence"
                | "live-review-confirm"
        ) {
            self.drafts
                .entry(self.selected.clone())
                .or_default()
                .current = form;
        }
    }
    /// Consent follows the exact currently loaded rendition projection, without acknowledging form text.
    pub fn consent(&mut self, confirmed: bool) {
        if let Some(draft) = self.drafts.get_mut(&self.selected) {
            draft.current.review.confirmed = confirmed;
        } else if confirmed {
            self.drafts
                .entry(self.selected.clone())
                .or_default()
                .current
                .review
                .confirmed = true;
        }
    }
    /// Explicit discard compares the exact confirmed snapshot, leaving all other targets intact.
    pub fn discard(&mut self, confirmed: &Form) -> bool {
        if self.current() != *confirmed {
            return false;
        }
        self.drafts.remove(&self.selected);
        true
    }
    /// Called once while constructing an intent. A retry retains its original raw form snapshot.
    pub fn submit(&mut self, command: &Command, form: &Form) {
        let Some((unit, key)) = intent(command) else {
            return;
        };
        if unit != self.selected {
            return;
        }
        let draft = self.drafts.entry(unit.into()).or_default();
        draft.current = form.clone();
        if draft
            .submitted
            .as_ref()
            .is_some_and(|(original, _)| original == &key)
        {
            return;
        }
        let section = match key {
            Intent::Brief(_) => Section::Brief(form.brief.clone()),
            Intent::Prepare(_) => Section::Producer(form.producer.clone()),
            Intent::Review(_) => Section::Review(form.review.clone()),
        };
        draft.submitted = Some((key, section));
    }
    pub fn acknowledge(&mut self, command: &Command, reply: &Reply) {
        let Some((unit, key)) = intent(command) else {
            return;
        };
        if command.check_reply(reply).is_err() {
            return;
        }
        let Some(draft) = self.drafts.get_mut(unit) else {
            return;
        };
        let Some((original, section)) = &draft.submitted else {
            return;
        };
        if *original != key {
            return;
        }
        match section {
            Section::Brief(value) => draft.saved.brief = value.clone(),
            Section::Producer(value) => draft.saved.producer = value.clone(),
            Section::Review(value) => {
                draft.saved.review = ReviewForm {
                    confirmed: false,
                    ..value.clone()
                };
                // Acknowledgement consumes only the submitted checkbox; later edits survive.
                if draft.current.review == *value {
                    draft.current.review.confirmed = false;
                    draft.saved.review.confirmed = false;
                }
            }
        }
        draft.submitted = None;
    }
}
/// A selected file remains unsaved until the authoritative rendition proves its exact bytes.
pub fn audio_dirty(
    file: Option<(&str, u64)>,
    submission: Option<&crate::contract::AudioSubmissionResponse>,
) -> bool {
    let Some((sha256, byte_length)) = file else {
        return false;
    };
    !submission
        .and_then(|s| s.rendition.as_ref())
        .is_some_and(|r| {
            r.verified_media.sha256 == sha256 && r.verified_media.byte_length == byte_length
        })
}
/// Consume only the file selection acknowledged by this Upload; later selections stay protected.
pub fn consume_uploaded_file(
    selected: Option<(&str, u64)>,
    generation: u64,
    submitted_generation: Option<u64>,
    command: &Command,
    reply: &Reply,
) -> bool {
    let Command::Upload { sha256, bytes, .. } = command else {
        return false;
    };
    submitted_generation == Some(generation)
        && selected == Some((sha256.as_str(), bytes.len() as u64))
        && (command.check_reply(reply).is_ok()
            || matches!(reply, Reply::Submission(status) if command.reconciles(reply, &status.submission.owner_id)))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::*;
    const UNIT: &str = "20000000-0000-4000-8000-000000000002";
    fn brief() -> (Command, Reply) {
        let mut request: CreateProductionBriefRequest = serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/briefs/v1/fixtures/create-narration.json")).unwrap();
        let document: ProductionBriefDocument = serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/briefs/v1/fixtures/narration-document.json")).unwrap();
        request.content_revision_digest = document.content.revision_digest.clone();
        (
            Command::CreateBrief {
                unit: UNIT.into(),
                request,
            },
            Reply::Brief(Box::new(ProductionBriefResponse {
                brief: document,
                freshness: BriefFreshness::Current,
            })),
        )
    }
    #[test]
    fn ancillary_lookup_and_edit_revert_leave_catalog_actions_available() {
        let mut drafts = AncillaryDrafts::default();
        drafts.open(UNIT);
        drafts.input("live-submission-id", Form::default());
        assert!(
            !drafts.has_unsaved(),
            "read-only lookup does not create a write draft"
        );
        let mut form = Form::default();
        form.brief.notes = "Giữ nhịp đọc tự nhiên".into();
        drafts.input("live-direction", form);
        assert!(drafts.has_unsaved());
        drafts.input("live-direction", Form::default());
        assert!(
            !drafts.has_unsaved(),
            "reverting exact bytes releases the guard"
        );
    }
    #[test]
    fn ancillary_reads_and_other_acks_cannot_clear_a_genuine_draft() {
        let mut drafts = AncillaryDrafts::default();
        drafts.open(UNIT);
        let mut form = Form::default();
        form.review.notes = "Chưa hoàn tất đánh giá".into();
        drafts.input("live-review-notes", form.clone());
        let (_, reply) = brief();
        drafts.acknowledge(
            &Command::Brief {
                unit: UNIT.into(),
                version: 1,
            },
            &reply,
        );
        assert_eq!(drafts.current(), form);
        assert!(drafts.has_unsaved());
    }
    #[test]
    fn ancillary_exact_ack_retains_later_typing_and_unrelated_review() {
        let mut drafts = AncillaryDrafts::default();
        drafts.open(UNIT);
        let (command, reply) = brief();
        let mut form = Form::default();
        form.brief.notes = "submitted".into();
        drafts.input("live-direction", form.clone());
        drafts.submit(&command, &form);
        drafts.acknowledge(&command, &reply);
        assert!(!drafts.has_unsaved());
        form.brief.notes = "new submitted".into();
        drafts.input("live-direction", form.clone());
        let mut next_command = command.clone();
        if let Command::CreateBrief { request, .. } = &mut next_command {
            request.operation_id = "other-operation".into();
        }
        drafts.submit(&next_command, &form);
        form.brief.notes = "later typing".into();
        form.review.notes = "unrelated review".into();
        drafts.input("live-review-notes", form.clone());
        drafts.acknowledge(&command, &reply);
        assert_eq!(drafts.current(), form);
        assert!(drafts.has_unsaved());
        drafts.acknowledge(&next_command, &reply);
        assert_eq!(drafts.current(), form);
        assert!(drafts.has_unsaved());
    }
    #[test]
    fn ancillary_discard_is_exact_and_does_not_drop_other_unit_drafts_or_unknown_write() {
        let mut drafts = AncillaryDrafts::default();
        drafts.open(UNIT);
        let mut form = Form::default();
        form.review.notes = "unsaved review".into();
        drafts.input("live-review-notes", form.clone());
        assert!(!drafts.discard(&Form::default()));
        assert_eq!(drafts.current(), form);
        let (command, _) = brief();
        let state = super::super::RequestState::default()
            .context("alice", "workspace")
            .unwrap();
        let (pending, ticket) = state.begin(command).unwrap();
        let unknown = pending.finish(ticket, Err(super::super::ClientError::Transport));
        assert!(unknown.pending().is_some());
        assert!(drafts.has_unsaved());
        drafts.open("another-unit");
        assert_eq!(drafts.current(), Form::default());
        assert!(drafts.discard(&Form::default()));
        assert!(drafts.has_unsaved());
        assert_eq!(drafts.open(UNIT), form);
        assert!(drafts.discard(&form));
        assert!(!drafts.has_unsaved());
        assert!(
            unknown.pending().is_some(),
            "discard cannot settle an unresolved server write"
        );
    }
    #[test]
    fn ancillary_successful_reads_lookup_changes_and_retries_do_not_rearm_clean_forms() {
        let mut drafts = AncillaryDrafts::default();
        drafts.open(UNIT);
        let (command, reply) = brief();
        let mut form = Form::default();
        form.brief.notes = "submitted exact bytes".into();
        drafts.input("live-direction", form.clone());
        drafts.submit(&command, &form);
        let original = form.clone();
        form.brief.notes = "typed during request".into();
        drafts.input("live-direction", form.clone());
        drafts.submit(&command, &form);
        drafts.acknowledge(&command, &reply);
        assert!(drafts.has_unsaved());
        drafts.input("live-direction", original.clone());
        assert!(
            !drafts.has_unsaved(),
            "retry must acknowledge the first submitted snapshot"
        );
        for lookup in [
            "live-submission-id",
            "live-candidate-id",
            "live-brief-version",
            "delegation-delegate",
            "artwork-alt",
        ] {
            drafts.input(lookup, form.clone());
            drafts.acknowledge(
                &Command::Brief {
                    unit: UNIT.into(),
                    version: 1,
                },
                &reply,
            );
            assert_eq!(drafts.current(), original);
            assert!(
                !drafts.has_unsaved(),
                "lookups and reads leave archive/reorder dirty guard released"
            );
        }
    }
    #[test]
    fn ancillary_review_and_producer_changes_are_independent_and_context_drops_only_consent() {
        let mut drafts = AncillaryDrafts::default();
        drafts.open(UNIT);
        let mut form = Form::default();
        form.review.notes = "Cần nghe lại".into();
        form.review.confirmed = true;
        form.producer.provenance = "External recording notes".into();
        drafts.input("live-review-confirm", form.clone());
        assert!(drafts.current_dirty());
        drafts.open("another-unit");
        assert!(!drafts.current_dirty());
        assert!(drafts.has_unsaved());
        let restored = drafts.open(UNIT);
        assert!(!restored.review.confirmed);
        assert_eq!(restored.review.notes, form.review.notes);
        assert_eq!(restored.producer, form.producer);
        assert!(drafts.current_dirty());
    }
    #[test]
    fn ancillary_selected_audio_is_unsaved_until_exact_admitted_byte_identity() {
        let rendition: AudioRenditionDocument = serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/rendition-response.json")).unwrap();
        let submission = AudioSubmissionDocument {
            schema_version: "1".into(),
            id: rendition.submission_id.clone(),
            operation_id: "prepare-op".into(),
            binding: rendition.binding.clone(),
            expected_sha256: rendition.verified_media.sha256.clone(),
            expected_byte_length: rendition.verified_media.byte_length,
            declarations: rendition.declarations.clone(),
            owner_id: "alice".into(),
            created_at: 0,
            expires_at: 1000,
            upload_path: "/private-fixture".into(),
            digest: rendition.submission_digest.clone(),
        };
        let sha = rendition.verified_media.sha256.clone();
        let len = rendition.verified_media.byte_length;
        let mut response = AudioSubmissionResponse {
            submission,
            state: AudioSubmissionState::Admitted,
            rendition: Some(rendition),
            reviews: vec![],
        };
        assert!(!audio_dirty(None, None));
        assert!(audio_dirty(Some((&sha, len)), None));
        assert!(!audio_dirty(Some((&sha, len)), Some(&response)));
        assert!(audio_dirty(Some((&sha, len + 1)), Some(&response)));
        assert!(audio_dirty(Some((&"f".repeat(64), len)), Some(&response)));
        response.rendition = None;
        assert!(audio_dirty(Some((&sha, len)), Some(&response)));
    }
    #[test]
    fn ancillary_upload_ack_consumes_only_original_selection_so_reads_cannot_redirty_it() {
        let rendition: AudioRenditionDocument = serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/rendition-response.json")).unwrap();
        let sha = rendition.verified_media.sha256.clone();
        let len = rendition.verified_media.byte_length;
        let command = Command::Upload {
            binding: rendition.binding.clone(),
            sha256: sha.clone(),
            submission: rendition.submission_id.clone(),
            credential: super::super::UploadCredential::new("a".repeat(64)).unwrap(),
            bytes: std::sync::Arc::new(vec![0; len as usize]),
        };
        let reply = Reply::Rendition(Box::new(rendition));
        let selected = Some((sha.as_str(), len));
        assert!(
            !consume_uploaded_file(selected, 2, Some(1), &command, &reply),
            "later file selection survives"
        );
        assert!(!consume_uploaded_file(selected, 1, None, &command, &reply));
        assert!(!consume_uploaded_file(
            Some((&sha, len + 1)),
            1,
            Some(1),
            &command,
            &reply
        ));
        assert!(!consume_uploaded_file(
            selected,
            1,
            Some(1),
            &Command::Brief {
                unit: UNIT.into(),
                version: 1
            },
            &reply
        ));
        let mut wrong = reply.clone();
        if let Reply::Rendition(r) = &mut wrong {
            r.verified_media.sha256 = "f".repeat(64);
        }
        assert!(!consume_uploaded_file(
            selected,
            1,
            Some(1),
            &command,
            &wrong
        ));
        assert!(consume_uploaded_file(
            selected,
            1,
            Some(1),
            &command,
            &reply
        ));
        let Reply::Rendition(rendition) = &reply else {
            unreachable!()
        };
        let receipt = Reply::Submission(Box::new(AudioSubmissionResponse {
            submission: AudioSubmissionDocument {
                schema_version: "1".into(),
                id: rendition.submission_id.clone(),
                operation_id: "prepare-op".into(),
                binding: rendition.binding.clone(),
                expected_sha256: sha.clone(),
                expected_byte_length: len,
                declarations: rendition.declarations.clone(),
                owner_id: "alice".into(),
                created_at: 0,
                expires_at: 1000,
                upload_path: "/private-fixture".into(),
                digest: rendition.submission_digest.clone(),
            },
            state: AudioSubmissionState::Revoked,
            rendition: Some((**rendition).clone()),
            reviews: vec![],
        }));
        assert!(
            consume_uploaded_file(selected, 1, Some(1), &command, &receipt),
            "exact owner reconciliation also settles original file"
        );
        assert!(!consume_uploaded_file(
            selected,
            2,
            Some(1),
            &command,
            &receipt
        ));
        // The shell releases this exact selection, so loss of a cached submission on later reads is harmless.
        assert!(!audio_dirty(None, None));
    }

    #[test]
    fn ancillary_read_invalidates_only_consent_projection_not_unsaved_review_text() {
        let mut drafts = AncillaryDrafts::default();
        drafts.open(UNIT);
        let mut form = Form::default();
        form.review.confirmed = true;
        drafts.input("live-review-confirm", form.clone());
        assert!(drafts.has_unsaved());
        drafts.consent(false);
        assert!(!drafts.has_unsaved());
        form.review.notes = "Still needs review".into();
        drafts.input("live-review-notes", form.clone());
        drafts.consent(false);
        assert!(drafts.has_unsaved());
        assert_eq!(drafts.current().review.notes, form.review.notes);
        assert!(!drafts.current().review.confirmed);
    }
}
