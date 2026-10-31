//! Bounded owner review. Reads never overwrite a dirty policy or turn a receipt into current state.
use crate::contract::*;
pub fn valid_page(work: &str, page: &ChapterMatrixPage) -> bool {
    let mut ids = std::collections::BTreeSet::new();
    page.work_id == work
        && page.work_revision > 0
        && page.items.len() <= 20
        && page.items.iter().all(|row| {
            crate::adaptation::is_run_id(&row.unit_id)
                && ids.insert(&row.unit_id)
                && row.draft_revision > 0
                && row.published_revision.is_some() == row.published_position.is_some()
                && row.published_revision.is_none_or(|r| r > 0)
                && row
                    .published_position
                    .is_none_or(|p| (1..=1000).contains(&p))
                && match &row.policy {
                    UnitViewerPolicy::Inherit => true,
                    UnitViewerPolicy::Restricted { grants } => grants.len() <= 1000,
                }
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitPolicyReview {
    pub work: String,
    pub row: ChapterMatrixRow,
    pub policy_version: u64,
    pub publication_version: u64,
    pub policy: UnitViewerPolicy,
}
impl UnitPolicyReview {
    pub fn publication_request(
        &self,
        page: &ChapterMatrixPage,
        operation_id: String,
        position: Option<u32>,
        withdraw: bool,
        confirmed: bool,
    ) -> Option<MetadataPublicationRequest> {
        if !confirmed
            || page.work_id != self.work
            || page.publication_version != self.publication_version
            || page.publication_lifecycle != MetadataPublicationLifecycle::Active
            || !page.items.iter().any(|r| {
                r.unit_id == self.row.unit_id && r.draft_revision == self.row.draft_revision
            })
        {
            return None;
        }
        if withdraw {
            self.row.published_revision?;
            Some(MetadataPublicationRequest::WithdrawUnit {
                operation_id,
                expected_version: self.publication_version,
                unit_id: self.row.unit_id.clone(),
            })
        } else {
            let position = position.filter(|p| (1..=1000).contains(p))?;
            Some(MetadataPublicationRequest::PublishUnit {
                operation_id,
                expected_version: self.publication_version,
                unit_id: self.row.unit_id.clone(),
                revision: self.row.draft_revision,
                position,
                confirm_metadata_rights: true,
            })
        }
    }
    pub fn select(page: &ChapterMatrixPage, unit: &str) -> Option<Self> {
        if !valid_page(&page.work_id, page) {
            return None;
        }
        let row = page.items.iter().find(|row| row.unit_id == unit)?.clone();
        Some(Self {
            work: page.work_id.clone(),
            policy: row.policy.clone(),
            row,
            policy_version: page.policy_version,
            publication_version: page.publication_version,
        })
    }
    pub fn dirty(&self) -> bool {
        self.policy != self.row.policy
    }
    pub fn add(&self, grant: UnitViewerGrant) -> Option<Self> {
        let mut next = self.clone();
        let mut grants = match &self.policy {
            UnitViewerPolicy::Inherit => vec![],
            UnitViewerPolicy::Restricted { grants } => grants.clone(),
        };
        if grants.len() >= 1000 || grants.iter().any(|old| old.principal == grant.principal) {
            return None;
        }
        grants.push(grant);
        next.policy = UnitViewerPolicy::Restricted { grants };
        Some(next)
    }
    pub fn remove(&self, principal: &ViewerPrincipal) -> Self {
        let mut next = self.clone();
        if let UnitViewerPolicy::Restricted { grants } = &mut next.policy {
            grants.retain(|g| &g.principal != principal);
        }
        next
    }
    /// Explicit reload/compare advances only the read fence and displayed pins. The user's
    /// replacement survives a stale policy, publication change or failed request unchanged.
    pub fn rebase(&self, page: &ChapterMatrixPage) -> Option<Self> {
        if page.work_id != self.work {
            return None;
        }
        let mut next = Self::select(page, &self.row.unit_id)?;
        next.policy = self.policy.clone();
        Some(next)
    }
    pub fn request(
        &self,
        page: &ChapterMatrixPage,
        operation_id: String,
        confirmed: bool,
    ) -> Option<ViewerAccessRequest> {
        if !confirmed
            || !self.dirty()
            || page.work_id != self.work
            || page.policy_version != self.policy_version
            || page.publication_version != self.publication_version
            || !page.items.iter().any(|r| r.unit_id == self.row.unit_id)
        {
            return None;
        }
        Some(ViewerAccessRequest {
            operation_id,
            expected_version: self.policy_version,
            change: ViewerAccessChange::SetUnitPolicy {
                unit_id: self.row.unit_id.clone(),
                policy: self.policy.clone(),
                confirm_metadata_rights: matches!(self.policy, UnitViewerPolicy::Inherit),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page() -> ChapterMatrixPage {
        ChapterMatrixPage {
            publication_lifecycle: MetadataPublicationLifecycle::Active,
            work_id: "00000000-0000-4000-8000-000000000001".into(),
            work_revision: 4,
            publication_version: 2,
            policy_version: 7,
            work_audience: ViewerAudience::Restricted,
            items: vec![ChapterMatrixRow {
                unit_id: "00000000-0000-4000-8000-000000000002".into(),
                title: "Ngày mai, mình có diễn tiếp không?".into(),
                draft_revision: 4,
                published_revision: Some(3),
                published_position: Some(1),
                audio: None,
                policy: UnitViewerPolicy::Inherit,
            }],
            next_cursor: Some("opaque".into()),
        }
    }
    #[test]
    fn unit_publication_names_the_exact_loaded_revision_order_and_work_cas() {
        let page = page();
        let review = UnitPolicyReview::select(&page, &page.items[0].unit_id).unwrap();
        assert!(review
            .publication_request(&page, "op".into(), Some(2), false, false)
            .is_none());
        assert!(review
            .publication_request(&page, "op".into(), Some(0), false, true)
            .is_none());
        assert!(matches!(
            review.publication_request(&page, "op".into(), Some(2), false, true),
            Some(MetadataPublicationRequest::PublishUnit {
                expected_version: 2,
                revision: 4,
                position: 2,
                confirm_metadata_rights: true,
                ..
            })
        ));
        assert!(matches!(
            review.publication_request(&page, "op".into(), None, true, true),
            Some(MetadataPublicationRequest::WithdrawUnit {
                expected_version: 2,
                ..
            })
        ));
        let mut stale = page.clone();
        stale.publication_version = 3;
        assert!(review
            .publication_request(&stale, "op".into(), Some(2), false, true)
            .is_none());
        let mut withdrawn = page;
        withdrawn.publication_lifecycle = MetadataPublicationLifecycle::Withdrawn;
        assert!(review
            .publication_request(&withdrawn, "op".into(), Some(2), false, true)
            .is_none());
    }
    #[test]
    fn exact_loaded_selection_cannot_include_an_unloaded_chapter_or_implicit_all() {
        let page = page();
        assert!(valid_page(&page.work_id, &page));
        assert!(UnitPolicyReview::select(&page, "unloaded").is_none());
        let review = UnitPolicyReview::select(&page, &page.items[0].unit_id).unwrap();
        assert!(!review.dirty());
        assert!(review.request(&page, "op".into(), true).is_none());
    }
    #[test]
    fn stale_reload_keeps_the_replacement_until_explicit_compare_and_confirmation() {
        let page = page();
        let mut review = UnitPolicyReview::select(&page, &page.items[0].unit_id).unwrap();
        review.policy = UnitViewerPolicy::Restricted { grants: vec![] };
        assert!(review.request(&page, "op".into(), false).is_none());
        let request = review.request(&page, "original".into(), true).unwrap();
        assert_eq!(request.expected_version, 7);
        let mut fresh = page.clone();
        fresh.policy_version = 8;
        fresh.publication_version = 3;
        fresh.items[0].draft_revision = 5;
        assert!(review.request(&fresh, "new".into(), true).is_none());
        assert_eq!(review.row.draft_revision, 4);
        let rebased = review.rebase(&fresh).unwrap();
        assert_eq!(rebased.policy, review.policy);
        assert_eq!(rebased.row.draft_revision, 5);
        assert_eq!(
            rebased
                .request(&fresh, "new".into(), true)
                .unwrap()
                .expected_version,
            8
        );
        // The original keyed command remains unchanged for uncertain-outcome reconciliation.
        assert_eq!(request.operation_id, "original");
        assert_eq!(request.expected_version, 7);
    }
}
