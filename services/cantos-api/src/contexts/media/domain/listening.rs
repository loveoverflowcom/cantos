//! Pure listener query and eligibility policy. Anonymous public access grants no Studio rights.
//! Theatre wire DTOs are projected by the application layer (`listener_projection`), never here.
use super::publication::{
    retained_release_eligible, AudioRelease, DistributionClearance, PublicationApproval,
};
use crate::contexts::catalog::domain::{ContentUnit, Lifecycle, Visibility, Work, WorkKind};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenerDomainError {
    InvalidQuery,
    InvalidLimit,
    BindingMismatch,
    InvalidTime,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryQuery {
    search: String,
    kind: Option<WorkKind>,
    language: Option<String>,
    limit: u8,
}
impl DiscoveryQuery {
    pub fn new(
        search: &str,
        kind: Option<&str>,
        language: Option<&str>,
        limit: u16,
    ) -> Result<Self, ListenerDomainError> {
        if !(1..=50).contains(&limit) {
            return Err(ListenerDomainError::InvalidLimit);
        }
        if search.len() > 512 || search.chars().any(char::is_control) {
            return Err(ListenerDomainError::InvalidQuery);
        }
        let kind = kind
            .map(|v| WorkKind::parse(v).map_err(|_| ListenerDomainError::InvalidQuery))
            .transpose()?;
        let language = language
            .map(|v| {
                if v.is_empty()
                    || v.len() > 63
                    || !v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                {
                    return Err(ListenerDomainError::InvalidQuery);
                }
                Ok(v.to_ascii_lowercase())
            })
            .transpose()?;
        Ok(Self {
            search: normalize_search(search.trim()),
            kind,
            language,
            limit: limit as u8,
        })
    }
    pub fn limit(&self) -> usize {
        usize::from(self.limit)
    }
    pub fn search(&self) -> &str {
        &self.search
    }
    pub fn kind(&self) -> Option<&'static str> {
        self.kind.map(WorkKind::as_str)
    }
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
    pub fn matches(&self, work: &DiscoverableWork<'_>) -> bool {
        self.kind.is_none_or(|k| k == work.kind)
            && self
                .language
                .as_deref()
                .is_none_or(|l| l == work.language.to_ascii_lowercase())
            && normalize_search(&format!("{} {}", work.title, work.description))
                .contains(&self.search)
    }
}
/// The public facts a discovery query filters on: the pinned work's category and text, and the
/// language of the audio actually published, which may differ from the work's metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiscoverableWork<'a> {
    pub kind: WorkKind,
    pub language: &'a str,
    pub title: &'a str,
    pub description: &'a str,
}
pub fn normalize_search(value: &str) -> String {
    value
        .nfc()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .nfc()
        .collect()
}
/// Production ordering kernel, shared by the keyset adapter and its proof. Descending time,
/// ascending immutable numeric UUID tie-breaker. No arithmetic over untrusted times.
pub fn after_work(time: i64, id: u128, after_time: i64, after_id: u128) -> bool {
    time < after_time || (time == after_time && id > after_id)
}
/// Units ordering kernel, shared by the guard over reader rows and its proof. Ascending listing
/// position, then ascending immutable numeric UUID. For canonical lower-case UUID text this is
/// the SQL `COLLATE "C"` byte order of `unit_id`, so the guard and the query agree.
pub fn after_unit(position: u32, id: u128, after_position: u32, after_id: u128) -> bool {
    position > after_position || (position == after_position && id > after_id)
}
pub fn access_window_valid(published: i64, expires: i64, now: i64) -> bool {
    published >= 0 && published <= now && now < expires
}

pub struct ListenerFacts<'a> {
    pub release: &'a AudioRelease,
    pub pinned_work: &'a Work,
    pub pinned_unit: &'a ContentUnit,
    pub current_work: &'a Work,
    pub current_unit: &'a ContentUnit,
    pub rights: Option<&'a DistributionClearance>,
    pub approval: Option<&'a PublicationApproval>,
    pub active: bool,
    pub owner_active: bool,
    pub withdrawn: bool,
    pub submission_revoked: bool,
    pub now: i64,
}
/// Cannot deserialize or construct public authority from a raw DTO.
/// ```compile_fail,E0277
/// use cantos_api::contexts::media::domain::listening::ListenerPublication;
/// let forged=serde_json::from_str::<ListenerPublication>("{}");
/// ```
/// ```compile_fail,E0616
/// use cantos_api::contexts::media::domain::listening::ListenerPublication;
/// fn mutate(p:&mut ListenerPublication){ p.release = p.release.clone(); }
/// ```
#[derive(Clone, Debug)]
pub struct ListenerPublication {
    release: AudioRelease,
    work: Work,
    unit: ContentUnit,
}
impl ListenerPublication {
    pub fn resolve(f: ListenerFacts<'_>) -> Result<Option<Self>, ListenerDomainError> {
        let d = f.release.document();
        let m = &d.manifest;
        if m.work_id != f.pinned_work.id().as_str()
            || m.unit_id != f.pinned_unit.id().as_str()
            || f.pinned_unit.work_id() != f.pinned_work.id()
            || m.work_metadata_revision != f.pinned_work.revision()
            || m.unit_metadata_revision != f.pinned_unit.revision()
            || m.work_title != f.pinned_work.details().title
            || m.unit_title != f.pinned_unit.details().title
            || d.published_by != f.pinned_work.owner_id().as_str()
            || f.current_work.id() != f.pinned_work.id()
            || f.current_unit.id() != f.pinned_unit.id()
            || f.current_unit.work_id() != f.current_work.id()
            || f.current_work.kind() != f.pinned_work.kind()
            || f.current_unit.kind() != f.pinned_unit.kind()
            || f.current_work.owner_id() != f.pinned_work.owner_id()
        {
            return Err(ListenerDomainError::BindingMismatch);
        }
        if f.now < 0 {
            return Err(ListenerDomainError::InvalidTime);
        }
        let visible = f.active
            && f.owner_active
            && f.pinned_work.details().visibility == Visibility::Public
            && f.current_work.details().visibility == Visibility::Public
            && f.current_work.lifecycle() != Lifecycle::Archived
            && f.current_unit.lifecycle() != Lifecycle::Archived
            && access_window_valid(
                d.published_at,
                m.distribution_clearance.request.valid_until,
                f.now,
            )
            && retained_release_eligible(
                f.release,
                f.rights,
                f.approval,
                f.withdrawn || f.submission_revoked,
                f.now,
            );
        Ok(visible.then(|| Self {
            release: f.release.clone(),
            work: f.pinned_work.clone(),
            unit: f.pinned_unit.clone(),
        }))
    }
    pub fn valid_at(&self, now: i64) -> bool {
        let d = self.release.document();
        access_window_valid(
            d.published_at,
            d.manifest.distribution_clearance.request.valid_until,
            now,
        )
    }
    pub fn release(&self) -> &AudioRelease {
        &self.release
    }
    /// The work metadata revision this release was published against, not the current one.
    pub fn work(&self) -> &Work {
        &self.work
    }
    /// The content unit metadata revision this release was published against.
    pub fn unit(&self) -> &ContentUnit {
        &self.unit
    }
    pub fn discoverable(&self) -> DiscoverableWork<'_> {
        let details = self.work.details();
        DiscoverableWork {
            kind: self.work.kind(),
            language: &self.release.document().manifest.language,
            title: &details.title,
            description: &details.synopsis,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    #[test]
    fn vietnamese_nfc_nfd_and_case_search_real_metadata() {
        let q = DiscoveryQuery::new("A\u{301}NH ĐÈN", None, None, 20).unwrap();
        let w = DiscoverableWork {
            kind: WorkKind::Story,
            title: "Ánh đèn cuối sân khấu",
            description: "Một lời hẹn",
            language: "vi-VN",
        };
        assert!(q.matches(&w));
        assert!(
            DiscoveryQuery::new("LỜI HẸN", Some("story"), Some("vi-vn"), 1)
                .unwrap()
                .matches(&w)
        );
        assert!(!DiscoveryQuery::new("", Some("podcast"), None, 1)
            .unwrap()
            .matches(&w));
    }
    #[test]
    fn kind_filter_accepts_exactly_the_catalog_work_kinds() {
        for (raw, kind) in [
            ("novel", WorkKind::Novel),
            ("story", WorkKind::Story),
            ("podcast", WorkKind::Podcast),
        ] {
            let q = DiscoveryQuery::new("", Some(raw), None, 1).unwrap();
            assert_eq!(q.kind(), Some(raw));
            for other in [WorkKind::Novel, WorkKind::Story, WorkKind::Podcast] {
                let w = DiscoverableWork {
                    kind: other,
                    title: "Tựa",
                    description: "",
                    language: "vi-VN",
                };
                assert_eq!(q.matches(&w), other == kind);
            }
        }
        for raw in ["Novel", "chapter", "episode", " story"] {
            assert_eq!(
                DiscoveryQuery::new("", Some(raw), None, 1),
                Err(ListenerDomainError::InvalidQuery)
            );
        }
        assert_eq!(DiscoveryQuery::new("", None, None, 1).unwrap().kind(), None);
    }
    #[test]
    fn invalid_query_partitions_are_exact() {
        for n in [0, 51, u16::MAX] {
            assert_eq!(
                DiscoveryQuery::new("", None, None, n),
                Err(ListenerDomainError::InvalidLimit)
            );
        }
        assert_eq!(
            DiscoveryQuery::new("a\n", None, None, 1),
            Err(ListenerDomainError::InvalidQuery)
        );
        assert_eq!(
            DiscoveryQuery::new("", Some("genre"), None, 1),
            Err(ListenerDomainError::InvalidQuery)
        );
        assert_eq!(
            DiscoveryQuery::new("", None, Some("../vi"), 1),
            Err(ListenerDomainError::InvalidQuery)
        );
    }
    fn public_fixture() -> super::super::publication::tests::Fixture {
        use crate::contexts::catalog::domain::{NewWork, Work};
        let mut f = super::super::publication::tests::fixture();
        let mut details = f.work.details().clone();
        details.visibility = Visibility::Public;
        f.work = Work::new(NewWork {
            id: f.work.id().clone(),
            kind: f.work.kind(),
            owner_id: f.work.owner_id().clone(),
            details,
            at: 1000,
        })
        .unwrap();
        f
    }
    #[test]
    fn only_current_public_approved_bound_release_resolves() {
        use super::super::publication::{decide, AudioRelease, ReleaseId};
        let f = public_fixture();
        let release = AudioRelease::publish(
            ReleaseId::parse("80000000-0000-4000-8000-000000000008").unwrap(),
            &f.actor,
            5000,
            None,
            decide(&f.facts()).unwrap(),
        )
        .unwrap();
        for denied in 0..8 {
            let resolved = ListenerPublication::resolve(ListenerFacts {
                release: &release,
                pinned_work: &f.work,
                pinned_unit: &f.unit,
                current_work: &f.work,
                current_unit: &f.unit,
                rights: (denied != 1).then_some(&f.rights),
                approval: (denied != 2).then_some(&f.approval),
                active: denied != 3,
                owner_active: denied != 4,
                withdrawn: denied == 5,
                submission_revoked: denied == 6,
                now: if denied == 7 { 10000 } else { 5000 },
            })
            .unwrap();
            assert_eq!(resolved.is_some(), denied == 0);
            if let Some(p) = resolved {
                // The decision carries the exact release and pinned metadata it was made for.
                assert_eq!(p.release().document().id, release.document().id);
                assert_eq!(
                    p.release().document().manifest.rendition_id,
                    f.r.document().id
                );
                assert_eq!(p.work().id(), f.work.id());
                assert_eq!(p.work().revision(), f.work.revision());
                assert_eq!(p.unit().id(), f.unit.id());
                assert_eq!(p.unit().revision(), f.unit.revision());
            }
        }
        let private = super::super::publication::tests::fixture();
        assert!(ListenerPublication::resolve(ListenerFacts {
            release: &release,
            pinned_work: &private.work,
            pinned_unit: &private.unit,
            current_work: &f.work,
            current_unit: &f.unit,
            rights: Some(&f.rights),
            approval: Some(&f.approval),
            active: true,
            owner_active: true,
            withdrawn: false,
            submission_revoked: false,
            now: 5000
        })
        .unwrap()
        .is_none());
        let wrong = crate::contexts::catalog::domain::Work::new(
            crate::contexts::catalog::domain::NewWork {
                id: f.work.id().clone(),
                kind: f.work.kind(),
                owner_id: crate::contexts::catalog::domain::OwnerId::parse("bob").unwrap(),
                details: f.work.details().clone(),
                at: 1000,
            },
        )
        .unwrap();
        assert_eq!(
            ListenerPublication::resolve(ListenerFacts {
                release: &release,
                pinned_work: &wrong,
                pinned_unit: &f.unit,
                current_work: &wrong,
                current_unit: &f.unit,
                rights: Some(&f.rights),
                approval: Some(&f.approval),
                active: true,
                owner_active: true,
                withdrawn: false,
                submission_revoked: false,
                now: 5000
            })
            .unwrap_err(),
            ListenerDomainError::BindingMismatch
        );
    }
    #[test]
    fn current_privacy_and_archival_changes_remove_access_without_rewriting_release() {
        use super::super::publication::{decide, AudioRelease, ReleaseId};
        let f = public_fixture();
        let release = AudioRelease::publish(
            ReleaseId::parse("80000000-0000-4000-8000-000000000008").unwrap(),
            &f.actor,
            5000,
            None,
            decide(&f.facts()).unwrap(),
        )
        .unwrap();
        let resolve = |work: &Work, unit: &ContentUnit| {
            ListenerPublication::resolve(ListenerFacts {
                release: &release,
                pinned_work: &f.work,
                pinned_unit: &f.unit,
                current_work: work,
                current_unit: unit,
                rights: Some(&f.rights),
                approval: Some(&f.approval),
                active: true,
                owner_active: true,
                withdrawn: false,
                submission_revoked: false,
                now: 6000,
            })
            .unwrap()
        };
        for visibility in [Visibility::Private, Visibility::Unlisted] {
            let mut details = f.work.details().clone();
            details.visibility = visibility;
            let current = f.work.edit(1, details, 6000).unwrap();
            assert!(resolve(&current, &f.unit).is_none());
        }
        assert!(resolve(&f.work.archive(1, 6000).unwrap(), &f.unit).is_none());
        assert!(resolve(&f.work, &f.unit.archive(&f.work, 1, 6000).unwrap()).is_none());
        assert!(resolve(&f.work, &f.unit).is_some());
    }
    #[test]
    fn mixed_language_work_preserves_actual_published_audio_language() {
        use super::super::publication::{decide, AudioRelease, ReleaseId};
        use crate::contexts::catalog::domain::{NewWork, Work};
        let mut f = public_fixture();
        let mut details = f.work.details().clone();
        details.language = "en-US".into();
        f.work = Work::new(NewWork {
            id: f.work.id().clone(),
            kind: f.work.kind(),
            owner_id: f.work.owner_id().clone(),
            details,
            at: 1000,
        })
        .unwrap();
        let r = AudioRelease::publish(
            ReleaseId::parse("80000000-0000-4000-8000-000000000008").unwrap(),
            &f.actor,
            5000,
            None,
            decide(&f.facts()).unwrap(),
        )
        .unwrap();
        let p = ListenerPublication::resolve(ListenerFacts {
            release: &r,
            pinned_work: &f.work,
            pinned_unit: &f.unit,
            current_work: &f.work,
            current_unit: &f.unit,
            rights: Some(&f.rights),
            approval: Some(&f.approval),
            active: true,
            owner_active: true,
            withdrawn: false,
            submission_revoked: false,
            now: 5000,
        })
        .unwrap()
        .unwrap();
        let discoverable = p.discoverable();
        assert_eq!(discoverable.language, "vi-VN");
        assert_eq!(discoverable.kind, f.work.kind());
        assert_eq!(discoverable.title, f.work.details().title);
        assert_eq!(discoverable.description, f.work.details().synopsis);
        assert!(DiscoveryQuery::new("", None, Some("vi-VN"), 1)
            .unwrap()
            .matches(&discoverable));
        assert!(!DiscoveryQuery::new("", None, Some("en-US"), 1)
            .unwrap()
            .matches(&discoverable));
    }
    #[test]
    fn work_keyset_direction_matches_descending_time_and_ascending_identity_at_extremes() {
        for (time, id, anchor_time, anchor_id, expected) in [
            (i64::MIN, 0, i64::MAX, u128::MAX, true),
            (i64::MAX, u128::MAX, i64::MIN, 0, false),
            (0, u128::MAX, 0, 0, true),
            (0, 0, 0, u128::MAX, false),
            (i64::MAX, u128::MAX, i64::MAX, u128::MAX, false),
        ] {
            assert_eq!(after_work(time, id, anchor_time, anchor_id), expected);
        }
    }

    #[test]
    fn unit_keyset_direction_matches_ascending_position_and_ascending_identity_at_extremes() {
        for (position, id, anchor_position, anchor_id, expected) in [
            (u32::MAX, 0, 0, u128::MAX, true),
            (0, u128::MAX, u32::MAX, 0, false),
            (0, u128::MAX, 0, 0, true),
            (0, 0, 0, u128::MAX, false),
            (u32::MAX, u128::MAX, u32::MAX, u128::MAX, false),
        ] {
            assert_eq!(
                after_unit(position, id, anchor_position, anchor_id),
                expected
            );
        }
    }

    #[test]
    fn immutable_anchor_prevents_replacement_duplicates_and_deletion_skips() {
        let initial = [(30, 1), (30, 2), (20, 3), (10, 4)];
        let first = &initial[..2];
        let after = first[1];
        // Work2's audio is replaced, Work1 deleted, Work5 inserted newer. Work3/4 stay visible.
        let changed = [(40, 5), (30, 2), (20, 3), (10, 4)];
        let rest: Vec<_> = changed
            .into_iter()
            .filter(|(t, i)| after_work(*t, *i, after.0, after.1))
            .collect();
        assert_eq!(rest, [(20, 3), (10, 4)]);
    }
    proptest! {
      #[test] fn keyset_is_strict_total_order(t in 0i64..i64::MAX,i in any::<u128>(),u in 0i64..i64::MAX,j in any::<u128>()){prop_assert!(!after_work(t,i,t,i));if (t,i)!=(u,j){prop_assert_ne!(after_work(t,i,u,j),after_work(u,j,t,i));}}
        #[test] fn unit_keyset_is_strict_total_order(p in 0u32..3,i in any::<u128>(),q in 0u32..3,j in any::<u128>()){prop_assert!(!after_unit(p,i,p,i));if (p,i)!=(q,j){prop_assert_ne!(after_unit(p,i,q,j),after_unit(q,j,p,i));}}
      #[test] fn unit_keyset_matches_sql_c_collation_of_canonical_uuid_text(p in 0u32..3,i in any::<u128>(),q in 0u32..3,j in any::<u128>()){
            let (a,b)=(uuid::Uuid::from_u128(i).to_string(),uuid::Uuid::from_u128(j).to_string());
            prop_assert_eq!(after_unit(p,i,q,j),(p,a.as_bytes())>(q,b.as_bytes()));
        }
      #[test] fn stable_work_anchors_page_once_under_insert_delete_and_rendition_replacement(data in prop::collection::vec((0i64..20,any::<bool>()),1..80),limit in 1usize..10){
            let original:Vec<_>=data.iter().enumerate().map(|(i,(t,stable))|(*t,i as u128,*stable)).collect();
            let stable:std::collections::BTreeSet<_>=original.iter().filter(|v|v.2).map(|v|v.1).collect();
            let mut anchor=None;let mut seen=Vec::new();let mut step=0;
            loop{let mut visible:Vec<_>=original.iter().copied().filter(|v|v.2 || (v.1+step)%3!=0).collect();
                // New works may land on either side; rendition replacement preserves work anchor.
                if step<4{visible.push((step as i64,(data.len()+step as usize)as u128,false));}
                visible.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                let page:Vec<_>=visible.into_iter().filter(|v|anchor.is_none_or(|(t,i)|after_work(v.0,v.1,t,i))).take(limit).collect();
                if page.is_empty(){break;}for v in &page{seen.push(v.1);}let last=page.last().unwrap();anchor=Some((last.0,last.1));step+=1;if step>100{prop_assert!(false,"did not terminate");}
            }
            let unique:std::collections::BTreeSet<_>=seen.iter().copied().collect();prop_assert_eq!(unique.len(),seen.len());prop_assert!(stable.is_subset(&unique));
        }
      #[test] fn expiry_is_half_open(start in any::<i64>(),end in any::<i64>(),now in any::<i64>()){prop_assert_eq!(access_window_valid(start,end,now),start>=0 && now>=start && end>now);}
    }
}
#[cfg(kani)]
mod kani_proofs {
    use super::{access_window_valid, after_work};
    #[kani::proof]
    #[kani::unwind(2)]
    fn listener_keyset_and_access_window() {
        let t: i64 = kani::any();
        let u: i64 = kani::any();
        let i: u128 = kani::any();
        let j: u128 = kani::any();
        // Compare the complete contractual sort key, not just properties that an
        // accidentally reversed ordering would also satisfy. Reverse avoids negation overflow.
        assert_eq!(
            after_work(t, i, u, j),
            (std::cmp::Reverse(t), i) > (std::cmp::Reverse(u), j)
        );
        assert!(!after_work(t, i, t, i));
        if (t, i) != (u, j) {
            assert_ne!(after_work(t, i, u, j), after_work(u, j, t, i));
        }
        let end: i64 = kani::any();
        let now: i64 = kani::any();
        assert_eq!(
            access_window_valid(t, end, now),
            t >= 0 && now >= t && end > now
        );
        kani::cover!(t < u && after_work(t, i, u, j), "older page item");
        kani::cover!(t == u && i > j && after_work(t, i, u, j), "tie breaker");
        kani::cover!(
            t == u && i == j && !after_work(t, i, u, j),
            "same identity excluded"
        );
        kani::cover!(t >= 0 && now == t && now < end, "inclusive start");
        kani::cover!(now == end, "exclusive expiry");
    }
}
