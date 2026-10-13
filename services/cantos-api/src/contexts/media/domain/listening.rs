//! Pure listener query and eligibility policy. Anonymous public access grants no Studio rights.
use super::publication::{
    retained_release_eligible, AudioRelease, DistributionClearance, PublicationApproval,
};
use crate::contexts::catalog::domain::{ContentUnit, Lifecycle, Visibility, Work};
use cantos_contracts::theatre as dto;
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
    kind: Option<dto::WorkKind>,
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
            .map(|v| match v {
                "novel" => Ok(dto::WorkKind::Novel),
                "story" => Ok(dto::WorkKind::Story),
                "podcast" => Ok(dto::WorkKind::Podcast),
                _ => Err(ListenerDomainError::InvalidQuery),
            })
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
        self.kind.map(|k| match k {
            dto::WorkKind::Novel => "novel",
            dto::WorkKind::Story => "story",
            dto::WorkKind::Podcast => "podcast",
        })
    }
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
    pub fn matches(&self, work: &dto::WorkSummary) -> bool {
        self.kind.is_none_or(|k| k == work.kind)
            && self
                .language
                .as_deref()
                .is_none_or(|l| l == work.language.to_ascii_lowercase())
            && normalize_search(&format!("{} {}", work.title, work.description))
                .contains(&self.search)
    }
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
            || m.language != f.pinned_work.details().language
            || d.published_by != f.pinned_work.owner_id().as_str()
            || f.current_work.id() != f.pinned_work.id()
            || f.current_unit.id() != f.pinned_unit.id()
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
    pub fn release(&self) -> &AudioRelease {
        &self.release
    }
    pub fn summary(&self, first_published_at_ms: i64) -> dto::WorkSummary {
        dto::WorkSummary {
            work_id: self.work.id().as_str().into(),
            kind: match self.work.kind() {
                crate::contexts::catalog::domain::WorkKind::Novel => dto::WorkKind::Novel,
                crate::contexts::catalog::domain::WorkKind::Story => dto::WorkKind::Story,
                crate::contexts::catalog::domain::WorkKind::Podcast => dto::WorkKind::Podcast,
            },
            title: self.work.details().title.clone(),
            description: self.work.details().synopsis.clone(),
            language: self.work.details().language.clone(),
            first_published_at_ms,
        }
    }
    pub fn unit(&self) -> dto::UnitSummary {
        let d = self.release.document();
        dto::UnitSummary {
            unit_id: self.unit.id().as_str().into(),
            kind: match self.unit.kind() {
                crate::contexts::catalog::domain::ContentUnitKind::Chapter => {
                    dto::UnitKind::Chapter
                }
                crate::contexts::catalog::domain::ContentUnitKind::Episode => {
                    dto::UnitKind::Episode
                }
            },
            position: self.unit.position(),
            title: self.unit.details().title.clone(),
            description: self.unit.details().description.clone(),
            publication_id: d.id.clone(),
            published_at_ms: d.published_at,
            duration_ms: d.manifest.media.duration_ms,
        }
    }
    pub fn manifest(&self) -> dto::PublicationManifest {
        let d = self.release.document();
        let m = &d.manifest;
        let a = &m.media;
        dto::PublicationManifest {
            schema_version: "1".into(),
            publication_id: d.id.clone(),
            work_id: m.work_id.clone(),
            unit_id: m.unit_id.clone(),
            rendition_id: m.rendition_id.clone(),
            manifest_digest: m.digest.clone(),
            sha256: a.sha256.clone(),
            byte_length: a.byte_length,
            duration_ms: a.duration_ms,
            container: a.container.clone(),
            codec: a.codec.clone(),
            channels: a.channels,
            sample_rate_hz: a.sample_rate_hz,
            profile_id: a.profile_id.clone(),
            language: m.language.clone(),
            published_at_ms: d.published_at,
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
        let w = dto::WorkSummary {
            work_id: "id".into(),
            kind: dto::WorkKind::Story,
            title: "Ánh đèn cuối sân khấu".into(),
            description: "Một lời hẹn".into(),
            language: "vi-VN".into(),
            first_published_at_ms: 10,
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
                let manifest = p.manifest();
                assert_eq!(manifest.publication_id, release.document().id);
                assert_eq!(manifest.rendition_id, f.r.document().id);
                assert_eq!(manifest.sha256, f.r.document().verified_media.sha256);
                let fields = serde_json::to_value(manifest).unwrap();
                for forbidden in [
                    "owner_id",
                    "candidate_id",
                    "delivery_object_id",
                    "binding",
                    "review_evidence",
                    "rights_refs",
                    "url",
                ] {
                    assert!(fields.get(forbidden).is_none());
                }
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
        kani::cover!(t < u, "older page item");
        kani::cover!(t == u && i > j, "tie breaker");
        kani::cover!(t == u && i == j, "same identity excluded");
        kani::cover!(t >= 0 && now == t && now < end, "inclusive start");
        kani::cover!(now == end, "exclusive expiry");
    }
}
