//! Exact, bounded publication selection. Catalog drafts and the request owner keep their lifetimes.
use crate::contract::{
    CatalogContentUnit, CatalogWork, MetadataPublicationRequest, MetadataPublicationState,
    MetadataPublicationUnit,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewError {
    WrongWork,
    Archived,
    Limit,
    MissingUnit,
    RightsUnconfirmed,
    InvalidState,
}

/// A review is a frozen selection, independent of subsequent head/page/status reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataReview {
    work: String,
    work_revision: u64,
    saved: CatalogWork,
    baseline: MetadataPublicationState,
    expected_version: u64,
    units: Vec<MetadataPublicationUnit>,
}

impl MetadataReview {
    pub fn new(work: &CatalogWork, state: &MetadataPublicationState) -> Result<Self, ReviewError> {
        if work.id != state.work_id {
            return Err(ReviewError::WrongWork);
        }
        if work.lifecycle != "draft" {
            return Err(ReviewError::Archived);
        }
        if !valid_state(state) {
            return Err(ReviewError::InvalidState);
        }
        Ok(Self {
            work: work.id.clone(),
            work_revision: work.revision,
            saved: work.clone(),
            baseline: state.clone(),
            expected_version: state.version,
            units: state
                .generation
                .as_ref()
                .map_or_else(Vec::new, |generation| {
                    generation
                        .entries
                        .iter()
                        .map(|entry| MetadataPublicationUnit {
                            unit_id: entry.unit_id.clone(),
                            revision: entry.revision,
                        })
                        .collect()
                }),
        })
    }

    pub fn work(&self) -> &str {
        &self.work
    }
    pub fn baseline(&self) -> &MetadataPublicationState {
        &self.baseline
    }
    pub fn saved(&self) -> &CatalogWork {
        &self.saved
    }
    pub fn work_revision(&self) -> u64 {
        self.work_revision
    }
    pub fn expected_version(&self) -> u64 {
        self.expected_version
    }
    pub fn units(&self) -> &[MetadataPublicationUnit] {
        &self.units
    }

    /// Only an explicitly chosen, already observed Unit can enter or update this selection.
    pub fn select(&self, unit: &CatalogContentUnit) -> Result<Self, ReviewError> {
        if unit.work_id != self.work {
            return Err(ReviewError::WrongWork);
        }
        if unit.lifecycle != "draft" {
            return Err(ReviewError::Archived);
        }
        let mut next = self.clone();
        let selected = MetadataPublicationUnit {
            unit_id: unit.id.clone(),
            revision: unit.revision,
        };
        if let Some(old) = next.units.iter_mut().find(|old| old.unit_id == unit.id) {
            *old = selected;
        } else if next.units.len() < 1000 {
            next.units.push(selected);
        } else {
            return Err(ReviewError::Limit);
        }
        Ok(next)
    }

    /// Removal from the complete final review is explicit; deselecting an update never removes a pin.
    pub fn remove(&self, unit: &str) -> Self {
        let mut next = self.clone();
        next.units.retain(|entry| entry.unit_id != unit);
        next
    }

    pub fn move_unit(&self, unit: &str, earlier: bool) -> Result<Self, ReviewError> {
        let mut next = self.clone();
        let index = next
            .units
            .iter()
            .position(|entry| entry.unit_id == unit)
            .ok_or(ReviewError::MissingUnit)?;
        let adjacent = if earlier {
            index.checked_sub(1)
        } else {
            index.checked_add(1).filter(|i| *i < next.units.len())
        };
        if let Some(adjacent) = adjacent {
            next.units.swap(index, adjacent);
        }
        Ok(next)
    }

    /// Explicit conflict recovery keeps every selected pin; the owner reviews the full replacement again.
    pub fn rebase(&self, state: &MetadataPublicationState) -> Result<Self, ReviewError> {
        if state.work_id != self.work {
            return Err(ReviewError::WrongWork);
        }
        if !valid_state(state) {
            return Err(ReviewError::InvalidState);
        }
        Ok(Self {
            expected_version: state.version,
            baseline: state.clone(),
            ..self.clone()
        })
    }

    pub fn request(
        &self,
        operation_id: String,
        confirmed: bool,
    ) -> Result<MetadataPublicationRequest, ReviewError> {
        if !confirmed {
            return Err(ReviewError::RightsUnconfirmed);
        }
        Ok(MetadataPublicationRequest::Publish {
            operation_id,
            expected_version: self.expected_version,
            work_revision: self.work_revision,
            units: self.units.clone(),
            confirm_metadata_rights: true,
        })
    }
}

/// Correlate transport structure only. The Server decides ownership, rights and admission.
pub fn valid_state(state: &MetadataPublicationState) -> bool {
    use crate::contract::MetadataPublicationLifecycle::*;
    match state.lifecycle {
        NeverPublished => {
            state.version == 0 && state.generation.is_none() && state.first_visible_at_ms.is_none()
        }
        Active | Withdrawn => {
            state.version > 0
                && state.first_visible_at_ms.is_some_and(|at| at >= 0)
                && state.generation.as_ref().is_some_and(|generation| {
                    generation.number > 0
                        && generation.work_revision > 0
                        && generation.entries.len() <= 1000
                        && generation.entries.iter().enumerate().all(|(index, entry)| {
                            entry.position as usize == index + 1 && entry.revision > 0
                        })
                        && generation
                            .entries
                            .iter()
                            .map(|entry| &entry.unit_id)
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            == generation.entries.len()
                })
        }
    }
}

#[derive(Clone, Default)]
pub struct MetadataFacts {
    pub current: Option<MetadataPublicationState>,
    /// A replayed receipt is historical evidence, never a current pointer read.
    pub receipt: Option<MetadataPublicationState>,
    /// Immutable artwork selections read through the existing exact-revision owner endpoint.
    pub artwork: std::collections::BTreeMap<u64, crate::contract::WorkArtworkResponse>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn work() -> CatalogWork {
        serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/work.json"
        ))
        .unwrap()
    }
    fn unit() -> CatalogContentUnit {
        serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/unit.json"
        ))
        .unwrap()
    }
    fn state(work: &CatalogWork) -> MetadataPublicationState {
        MetadataPublicationState {
            work_id: work.id.clone(),
            lifecycle: crate::contract::MetadataPublicationLifecycle::NeverPublished,
            version: 0,
            first_visible_at_ms: None,
            generation: None,
        }
    }
    #[test]
    fn loaded_pages_never_implicitly_select_new_drafts_and_confirmation_is_exact() {
        let work = work();
        let mut unit = unit();
        unit.work_id = work.id.clone();
        let review = MetadataReview::new(&work, &state(&work)).unwrap();
        assert!(review.units().is_empty());
        let selected = review.select(&unit).unwrap();
        unit.revision += 1;
        assert_eq!(selected.units()[0].revision + 1, unit.revision);
        assert_eq!(
            selected.request("op".into(), false),
            Err(ReviewError::RightsUnconfirmed)
        );
        assert_eq!(
            selected.request("op".into(), true).unwrap(),
            MetadataPublicationRequest::Publish {
                operation_id: "op".into(),
                expected_version: 0,
                work_revision: work.revision,
                units: selected.units().to_vec(),
                confirm_metadata_rights: true
            }
        );
    }
    #[test]
    fn published_pins_survive_unloaded_pages_and_new_drafts_have_a_hard_bound() {
        let mut work = work();
        let mut active:MetadataPublicationState=serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/metadata-publication/v1/fixtures/active.json")).unwrap();
        work.id = active.work_id.clone();
        let review = MetadataReview::new(&work, &active).unwrap();
        assert_eq!(
            review.units().len(),
            active.generation.as_ref().unwrap().entries.len()
        );
        work.details.title = "New head after opening the review".into();
        assert_ne!(review.saved().details.title, work.details.title);
        active.version += 1;
        let rebased = review.rebase(&active).unwrap();
        assert_eq!(rebased.units(), review.units());
        assert_eq!(review.baseline().version + 1, rebased.baseline().version);
        let mut bounded = MetadataReview::new(&work, &state(&work)).unwrap();
        let mut draft = unit();
        draft.work_id = work.id.clone();
        for index in 0..1000 {
            draft.id = format!("observed-{index}");
            bounded = bounded.select(&draft).unwrap();
        }
        draft.id = "unseen-overflow".into();
        assert_eq!(bounded.select(&draft), Err(ReviewError::Limit));
        draft.id = "observed-500".into();
        draft.revision += 1;
        assert!(
            bounded.select(&draft).is_ok(),
            "updating an existing pin does not add membership"
        );
    }
    #[test]
    fn reorder_and_removal_are_explicit_and_wrong_parent_leaves_selection_unchanged() {
        let work = work();
        let mut a = unit();
        a.work_id = work.id.clone();
        let mut b = a.clone();
        b.id = "second".into();
        let review = MetadataReview::new(&work, &state(&work))
            .unwrap()
            .select(&a)
            .unwrap()
            .select(&b)
            .unwrap();
        assert_eq!(
            review.move_unit(&b.id, true).unwrap().units()[0].unit_id,
            b.id
        );
        b.work_id = "another".into();
        assert_eq!(review.select(&b), Err(ReviewError::WrongWork));
        assert_eq!(review.units().len(), 2);
        assert_eq!(
            review.remove(&a.id).units(),
            &[MetadataPublicationUnit {
                unit_id: b.id,
                revision: b.revision
            }]
        );
    }
}

/// Check the actual operation result; a state for another selection cannot acknowledge a write.
pub fn answers(request: &MetadataPublicationRequest, state: &MetadataPublicationState) -> bool {
    use crate::contract::MetadataPublicationLifecycle::{Active, Withdrawn};
    use MetadataPublicationRequest::*;
    if !valid_state(state) || request.expected_version().checked_add(1) != Some(state.version) {
        return false;
    }
    match request {
        Publish {
            work_revision,
            units,
            ..
        } => {
            state.lifecycle == Active
                && state.generation.as_ref().is_some_and(|g| {
                    g.work_revision == *work_revision
                        && g.entries.len() == units.len()
                        && g.entries.iter().zip(units).all(|(entry, unit)| {
                            entry.unit_id == unit.unit_id && entry.revision == unit.revision
                        })
                })
        }
        PublishUnit {
            unit_id,
            revision,
            position,
            ..
        } => {
            state.lifecycle == Active
                && state.generation.as_ref().is_some_and(|g| {
                    g.entries.iter().any(|entry| {
                        entry.unit_id == *unit_id
                            && entry.revision == *revision
                            && entry.position == *position
                    })
                })
        }
        WithdrawUnit { unit_id, .. } => {
            state.lifecycle == Active
                && state
                    .generation
                    .as_ref()
                    .is_some_and(|g| g.entries.iter().all(|entry| entry.unit_id != *unit_id))
        }
        Withdraw { .. } => state.lifecycle == Withdrawn,
        AssociateAudio {
            unit_id,
            release_id,
            ..
        } => {
            state.lifecycle == Active
                && state.generation.as_ref().is_some_and(|g| {
                    g.entries.iter().any(|entry| {
                        entry.unit_id == *unit_id
                            && entry
                                .audio
                                .as_ref()
                                .is_some_and(|a| a.release_id == *release_id)
                    })
                })
        }
    }
}

#[cfg(test)]
mod request_tests {
    use super::*;
    use crate::live::{ClientError, Command, Facts, Phase, Reply, RequestState};
    fn active() -> MetadataPublicationState {
        serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/metadata-publication/v1/fixtures/active.json")).unwrap()
    }
    fn command(state: &MetadataPublicationState) -> Command {
        let g = state.generation.as_ref().unwrap();
        Command::MetadataCommand {
            work: state.work_id.clone(),
            request: MetadataPublicationRequest::Publish {
                operation_id: "65100000-0000-4000-8000-000000000001".into(),
                expected_version: state.version - 1,
                work_revision: g.work_revision,
                units: g
                    .entries
                    .iter()
                    .map(|e| MetadataPublicationUnit {
                        unit_id: e.unit_id.clone(),
                        revision: e.revision,
                    })
                    .collect(),
                confirm_metadata_rights: true,
            },
        }
    }
    #[test]
    fn lost_ack_retries_the_same_operation_and_old_receipts_never_claim_current_state() {
        let response = active();
        let command = command(&response);
        let initial = RequestState::default()
            .context("owner", &response.work_id)
            .unwrap();
        let (sending, ticket) = initial.begin(command.clone()).unwrap();
        assert!(sending.begin(command.clone()).is_none());
        let ambiguous = sending.finish(ticket, Err(ClientError::Transport));
        assert_eq!(ambiguous.phase(), Phase::Ambiguous);
        let (retry, ticket) = ambiguous.retry().unwrap();
        assert_eq!(
            retry.pending().unwrap().json().unwrap(),
            command.json().unwrap()
        );
        let done = retry.finish(ticket, Ok(Reply::MetadataState(response.clone())));
        assert_eq!(done.phase(), Phase::Ready);
        let facts =
            Facts::default().apply_command(&command, Reply::MetadataState(response.clone()));
        assert_eq!(facts.metadata.current, None);
        assert_eq!(facts.metadata.receipt, Some(response));
    }
    #[test]
    fn a_wrong_revision_or_selection_cannot_acknowledge_the_publish() {
        let response = active();
        let command = command(&response);
        let mut wrong = response.clone();
        wrong.generation.as_mut().unwrap().work_revision += 1;
        assert_eq!(
            command.check_reply(&Reply::MetadataState(wrong)),
            Err(ClientError::ReplyMismatch)
        );
        let mut wrong = response.clone();
        wrong.generation.as_mut().unwrap().entries[0].revision += 1;
        assert_eq!(
            command.check_reply(&Reply::MetadataState(wrong)),
            Err(ClientError::ReplyMismatch)
        );
    }
}

#[cfg(test)]
mod asset_tests {
    use crate::live::{Command, Facts, Reply};
    #[test]
    fn cover_review_uses_exact_historical_selection_and_rejects_a_different_head() {
        let command = Command::MetadataArtwork {
            work: "65100000-0000-4000-8000-000000000001".into(),
            revision: 3,
        };
        assert!(command
            .path()
            .unwrap()
            .ends_with("/artwork?work_revision=3"));
        let mut response = crate::contract::WorkArtworkResponse {
            work_id: "65100000-0000-4000-8000-000000000001".into(),
            work_revision: 4,
            artwork: None,
            changed_at_ms: None,
        };
        assert!(command
            .check_reply(&Reply::Artwork(response.clone()))
            .is_err());
        response.work_revision = 3;
        assert!(command
            .check_reply(&Reply::Artwork(response.clone()))
            .is_ok());
        let facts = Facts::default().apply_command(&command, Reply::Artwork(response.clone()));
        assert_eq!(facts.metadata.artwork.get(&3), Some(&response));
        assert!(!facts.metadata.artwork.contains_key(&4));
    }
}
