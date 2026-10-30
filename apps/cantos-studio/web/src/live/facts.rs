//! Pure acknowledgement projection; changing a parent clears every dependent snapshot.
use super::{Command, Reply};
use crate::contract::*;
#[derive(Clone, Default)]
pub struct Facts {
    pub works: Vec<CatalogWork>,
    pub works_next: Option<String>,
    pub work: Option<CatalogWork>,
    pub units: Vec<CatalogContentUnit>,
    pub units_next: Option<String>,
    pub unit: Option<CatalogContentUnit>,
    pub content: Option<ContentRevisionResponse>,
    /// Current-head authority is separate from a historical inspection.
    pub content_head: Option<ContentRevisionResponse>,
    pub content_base: Option<u64>,
    pub history: Option<ContentHistoryResponse>,
    pub pages: super::ReadPages,
    pub brief: Option<ProductionBriefResponse>,
    pub submission: Option<AudioSubmissionResponse>,
    pub candidate: Option<ReleaseCandidateResponse>,
    pub publication: super::publication::PublicationFacts,
    /// The exact saved revision last previewed; dropped when the saved head moves past it.
    pub preview: Option<super::preview::PreviewFacts>,
}
impl Facts {
    /// Only a head read or an append receipt advances the current-content projection.
    pub fn apply_command(&self, command: &Command, reply: Reply) -> Self {
        let head = match (command, &reply) {
            (Command::ContentHead { .. }, Reply::Content(content)) => Some(content.clone()),
            _ => None,
        };
        let publication_reply = match (command, &reply) {
            (Command::Publish { .. }, Reply::ActiveRelease(receipt)) => {
                Some((false, receipt.clone()))
            }
            (Command::ActiveRelease { .. }, Reply::ActiveRelease(active)) => {
                Some((true, active.clone()))
            }
            _ => None,
        };
        let mut pages = self.pages.clone();
        pages.acknowledge(command, &reply);
        let paged_read = matches!(
            (command, &reply),
            (Command::Works { .. }, Reply::Works(_))
                | (Command::Units { .. }, Reply::Units(_))
                | (Command::ContentHistory { .. }, Reply::History(_))
        );
        let preview_walk = match (command, &reply) {
            (
                Command::Preview {
                    cursor: Some(_), ..
                },
                Reply::Preview(page),
            ) => Some(
                self.preview
                    .as_ref()
                    .and_then(|shown| shown.extend(page.clone()))
                    .or_else(|| self.preview.clone()),
            ),
            _ => None,
        };
        let mut next = self.apply(reply);
        if let Some(walk) = preview_walk {
            next.preview = walk;
        }
        if paged_read {
            next.pages = pages;
        }
        if let Some((is_current_read, response)) = publication_reply {
            if is_current_read {
                next.publication.active = Some(*response);
            } else {
                next.publication.publish_receipt = Some(*response);
                // A replay returns its original receipt. Only a new owner GET observes today's pointer.
                next.publication.active = None;
                next.publication.readiness = None;
            }
        }
        if matches!(
            command,
            Command::PatchWork { .. }
                | Command::ArchiveWork { .. }
                | Command::PatchUnit { .. }
                | Command::ArchiveUnit { .. }
                | Command::Reorder { .. }
        ) {
            next.clear_publication();
        }
        if let Some(head) = head {
            next.content_base = Some(head.revision);
            next.content_head = Some(head);
            next.brief = None;
            next.submission = None;
            next.candidate = None;
            next.clear_publication();
        }
        // A save, archive, reorder or newer read moves the head: older text must not stay shown.
        if let (Some(shown), Some(saved)) = (&next.preview, &next.work) {
            if !shown.current_for(saved) {
                next.preview = None;
            }
        }
        next.publication.form_ack = match command {
            Command::RecordDistributionRights { request, .. } => Some(request.operation_id.clone()),
            Command::RecordPublicationApproval { request, .. } => {
                Some(request.operation_id.clone())
            }
            Command::Withdraw { request, .. } => Some(request.operation_id.clone()),
            Command::PatchWork { request, .. } => Some(request.operation_id.clone()),
            _ => None,
        };
        next
    }
    /// Acknowledged unit creation opens only its exact returned unit in the selected Work.
    /// Every prior unit-dependent observation is discarded before the new context is shown.
    pub fn select_created_unit(&mut self, id: &str) -> bool {
        let Some(work) = self.work.as_ref() else {
            return false;
        };
        let Some(unit) = self
            .units
            .iter()
            .find(|unit| unit.id == id && unit.work_id == work.id)
            .cloned()
        else {
            return false;
        };
        self.unit = Some(unit);
        self.content = None;
        self.content_head = None;
        self.content_base = Some(0);
        self.history = None;
        self.pages.clear_history();
        self.brief = None;
        self.submission = None;
        self.candidate = None;
        self.clear_publication();
        true
    }
    pub fn clear_publication(&mut self) {
        self.publication = super::publication::PublicationFacts::default();
    }
    pub fn editor_script(&self) -> Option<CatalogScriptRef> {
        let unit = self.unit.as_ref()?;
        if let Some(head) = self
            .content_head
            .as_ref()
            .filter(|head| head.unit_id == unit.id)
        {
            if let ContentBody::Script { script, .. } = &head.details.body {
                return Some(script.clone());
            }
        }
        unit.details.script.clone()
    }

    pub fn apply(&self, reply: Reply) -> Self {
        let mut next = self.clone();
        match reply {
            Reply::Works(page) => {
                next.works = page.items;
                next.works_next = page.next_cursor;
            }
            Reply::Work(work) => {
                next.work = Some(work);
                next.clear_publication();
            }
            Reply::Preview(page) => next.preview = Some(super::preview::PreviewFacts::first(page)),
            Reply::Units(page) => {
                if let Some(current) = &next.unit {
                    if let Some(observed) = page.items.iter().find(|unit| unit.id == current.id) {
                        if observed != current {
                            next.clear_publication();
                        }
                        next.unit = Some(observed.clone());
                    }
                }
                next.units = page.items;
                next.units_next = page.next_cursor;
            }
            Reply::Mutation(reply) => {
                if next.work.as_ref().is_none_or(|w| w.id != reply.work.id) {
                    next = Self {
                        works: next.works,
                        works_next: next.works_next,
                        pages: next.pages.for_work(),
                        ..Self::default()
                    };
                }
                next.work = Some(reply.work.clone());
                if let Some(work) = next.works.iter_mut().find(|w| w.id == reply.work.id) {
                    *work = reply.work;
                } else {
                    next.works.push(reply.work);
                }
                for unit in reply.units {
                    if let Some(old) = next.units.iter_mut().find(|u| u.id == unit.id) {
                        *old = unit.clone();
                    } else {
                        next.units.push(unit.clone());
                    }
                    if next.unit.as_ref().is_some_and(|u| u.id == unit.id) {
                        next.unit = Some(unit);
                    }
                }
            }
            Reply::Content(content) => next.content = Some(content),
            Reply::ContentSaved(saved) => {
                if let Some(work) = &mut next.work {
                    work.revision = saved.work_revision;
                    if let Some(cached) = next.works.iter_mut().find(|w| w.id == work.id) {
                        cached.revision = saved.work_revision;
                    }
                }
                next.content_base = Some(saved.content.revision);
                next.content_head = Some(saved.content.clone());
                next.content = Some(saved.content);
                next.brief = None;
                next.submission = None;
                next.candidate = None;
                next.clear_publication();
            }
            Reply::History(history) => next.history = Some(history),
            Reply::Brief(brief) => {
                next.brief = Some(*brief);
                next.submission = None;
                next.candidate = None;
                next.clear_publication();
            }
            Reply::Submission(submission) => {
                next.submission = Some(*submission);
                next.candidate = None;
                next.clear_publication();
            }
            Reply::Rendition(rendition) => {
                if let Some(submission) = &mut next.submission {
                    if submission.submission.id == rendition.submission_id {
                        submission.state = AudioSubmissionState::Admitted;
                        submission.rendition = Some(*rendition);
                    }
                }
                next.candidate = None;
                next.clear_publication();
            }
            Reply::Candidate(candidate) => {
                next.candidate = Some(candidate);
                next.clear_publication();
            }
            Reply::Readiness(readiness) => next.publication.readiness = Some(*readiness),
            Reply::DistributionRights(receipt) => {
                next.publication.clearance_receipt = Some(receipt);
                next.publication.readiness = None;
                next.publication.active = None;
            }
            Reply::PublicationApproval(receipt) => {
                next.publication.approval_receipt = Some(receipt);
                next.publication.readiness = None;
                next.publication.active = None;
            }
            Reply::ActiveRelease(_) | Reply::AudioPreview(_) => {}
            Reply::Withdrawal(receipt) => {
                next.publication.withdrawal_receipt = Some(receipt);
                next.publication.readiness = None;
                next.publication.active = None;
            }
            Reply::Review(_) | Reply::Revoked => {
                next.submission = None;
                next.candidate = None;
                next.clear_publication();
            }
            Reply::Artwork(_)
            | Reply::ImagePrepared(_)
            | Reply::ImageIntent(_)
            | Reply::ImageAdmitted(_) => {}
            // Delegation grants are projected by the delegation panel, never content facts.
            Reply::Delegations(_) | Reply::DelegationIssued(_) => {}
        }
        next
    }
    pub fn unit_id(&self) -> String {
        self.unit.as_ref().map(|u| u.id.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod created_unit_tests {
    use super::*;
    const CREATED: &str = "20000000-0000-4000-8000-000000000099";

    fn previous_unit_facts() -> Facts {
        let work: CatalogWork = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/work.json"
        ))
        .unwrap();
        let mut unit: CatalogContentUnit = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/unit.json"
        ))
        .unwrap();
        let brief: ProductionBriefDocument = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/briefs/v1/fixtures/narration-document.json"
        )).unwrap();
        let rendition: AudioRenditionDocument = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/rendition-response.json"
        )).unwrap();
        unit.id = brief.content.unit_id.clone();
        let mut created = unit.clone();
        created.id = CREATED.into();
        let candidate = ReleaseCandidateResponse {
            candidate: ReleaseCandidateDocument {
                id: "candidate-for-previous-unit".into(),
                rendition_id: rendition.id.clone(),
                rendition_digest: rendition.digest.clone(),
                binding: rendition.binding.clone(),
                sha256: rendition.verified_media.sha256.clone(),
                work_metadata_revision: 1,
                unit_metadata_revision: 1,
                review_ids: vec![],
                created_by: "synthetic-owner".into(),
                created_at: 1,
                operation_id: "candidate-operation".into(),
                digest: "previous-candidate-digest".into(),
            },
            blockers: vec![AudioReleaseBlocker::PublicDeliveryNotImplemented],
        };
        let submission = AudioSubmissionResponse {
            submission: AudioSubmissionDocument {
                schema_version: "1.0.0".into(),
                id: rendition.submission_id.clone(),
                operation_id: "submission-operation".into(),
                binding: rendition.binding.clone(),
                expected_sha256: rendition.verified_media.sha256.clone(),
                expected_byte_length: rendition.verified_media.byte_length,
                declarations: rendition.declarations.clone(),
                owner_id: "synthetic-owner".into(),
                created_at: 1,
                expires_at: 1000,
                upload_path: "/synthetic-private-path".into(),
                digest: rendition.submission_digest.clone(),
            },
            state: AudioSubmissionState::Admitted,
            rendition: Some(rendition),
            reviews: vec![],
        };
        let mut facts = Facts {
            work: Some(work),
            units: vec![unit.clone(), created],
            unit: Some(unit),
            content: Some(brief.content.clone()),
            content_head: Some(brief.content.clone()),
            content_base: Some(7),
            history: Some(ContentHistoryResponse {
                revisions: vec![],
                next_after: None,
            }),
            brief: Some(ProductionBriefResponse {
                brief,
                freshness: BriefFreshness::Current,
            }),
            submission: Some(submission),
            candidate: Some(candidate),
            ..Facts::default()
        };
        facts.publication.readiness = Some(PublicationReadinessResponse {
            candidate_id: "previous-candidate".into(),
            candidate_digest: "previous-digest".into(),
            pointer_version: 9,
            active_release_id: Some("previous-release".into()),
            blockers: vec![],
            clearance: None,
            approval: None,
        });
        facts.publication.active = Some(ActiveAudioReleaseResponse {
            pointer_version: 9,
            release: None,
        });
        facts.publication.form_ack = Some("previous-form-operation".into());
        facts
    }

    #[test]
    fn created_unit_opens_without_previous_content_brief_audio_candidate_or_publication() {
        let mut facts = previous_unit_facts();
        assert!(facts.select_created_unit(CREATED));
        assert_eq!(facts.unit_id(), CREATED);
        assert_eq!(facts.content, None);
        assert_eq!(facts.content_head, None);
        assert_eq!(facts.content_base, Some(0));
        assert_eq!(facts.history, None);
        assert_eq!(facts.brief, None);
        assert_eq!(facts.submission, None);
        assert_eq!(facts.candidate, None);
        assert_eq!(
            facts.publication,
            super::super::publication::PublicationFacts::default()
        );
        assert_eq!(facts.units.len(), 2);
        assert!(facts.work.is_some());
    }

    #[test]
    fn missing_or_other_work_created_unit_never_sets_an_empty_content_base() {
        let original = previous_unit_facts();
        for target in ["missing", CREATED] {
            let mut facts = original.clone();
            if target == CREATED {
                facts.units.last_mut().unwrap().work_id = "other-work".into();
            }
            assert!(!facts.select_created_unit(target));
            assert_eq!(facts.unit, original.unit);
            assert_eq!(facts.content_base, Some(7));
            assert_eq!(facts.content_head, original.content_head);
            assert_eq!(facts.brief, original.brief);
            assert_eq!(facts.submission, original.submission);
            assert_eq!(facts.candidate, original.candidate);
            assert_eq!(facts.publication, original.publication);
        }
    }
}
