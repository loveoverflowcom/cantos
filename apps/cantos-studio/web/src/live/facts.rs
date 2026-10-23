//! Pure acknowledgement projection; changing a parent clears every dependent snapshot.
use super::Reply;
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
    pub content_base: Option<u64>,
    pub history: Option<ContentHistoryResponse>,
    pub brief: Option<ProductionBriefResponse>,
    pub submission: Option<AudioSubmissionResponse>,
    pub candidate: Option<ReleaseCandidateResponse>,
}
impl Facts {
    pub fn apply(&self, reply: Reply) -> Self {
        let mut next = self.clone();
        match reply {
            Reply::Works(page) => {
                next.works = page.items;
                next.works_next = page.next_cursor;
            }
            Reply::Work(work) => next.work = Some(work),
            Reply::Units(page) => {
                next.units = page.items;
                next.units_next = page.next_cursor;
            }
            Reply::Mutation(reply) => {
                if next.work.as_ref().is_none_or(|w| w.id != reply.work.id) {
                    next = Self {
                        works: next.works,
                        works_next: next.works_next,
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
            Reply::Content(content) => {
                next.content_base = Some(content.revision);
                next.content = Some(content);
                next.brief = None;
                next.submission = None;
                next.candidate = None;
            }
            Reply::ContentSaved(saved) => {
                if let Some(work) = &mut next.work {
                    work.revision = saved.work_revision;
                    if let Some(cached) = next.works.iter_mut().find(|w| w.id == work.id) {
                        cached.revision = saved.work_revision;
                    }
                }
                next.content_base = Some(saved.content.revision);
                next.content = Some(saved.content);
                next.brief = None;
                next.submission = None;
                next.candidate = None;
            }
            Reply::History(history) => next.history = Some(history),
            Reply::Brief(brief) => {
                next.brief = Some(*brief);
                next.submission = None;
                next.candidate = None;
            }
            Reply::Submission(submission) => {
                next.submission = Some(*submission);
                next.candidate = None;
            }
            Reply::Rendition(rendition) => {
                if let Some(submission) = &mut next.submission {
                    if submission.submission.id == rendition.submission_id {
                        submission.state = AudioSubmissionState::Admitted;
                        submission.rendition = Some(*rendition);
                    }
                }
                next.candidate = None;
            }
            Reply::Candidate(candidate) => next.candidate = Some(candidate),
            Reply::Review(_) | Reply::Revoked => {
                next.submission = None;
                next.candidate = None;
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
