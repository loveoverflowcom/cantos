//! Pure, fail-closed metadata-disclosure decisions for the owner-approved sample scope.
//!
//! These facts are not authority by themselves. The application must obtain them from the
//! authenticated actor and a single current database snapshot, including the exact approved
//! sample membership and verified rights/source records. This module does not publish anything,
//! create a playback grant, or expose a route.

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{Lifecycle, Visibility};

/// Whether the requested identity belongs to the explicitly approved sample dataset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleMembership {
    Approved,
    OutsideApprovedSample,
}

/// Hash of the exact #427 input whose metadata the owner approved on 27 October 2026.
/// This is a scope fence, not evidence that an arbitrary stored snapshot matches that input.
pub const APPROVED_SAMPLE_SOURCE_SHA256: &str =
    "d0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212";

/// Stable identities derived by #427's input mapping. A unit belongs only to the named Work.
/// The caller must also validate the decoded pinned snapshot against the pinned source before
/// it may assert `SnapshotBinding::Valid`; matching an ID alone is never disclosure authority.
const APPROVED_SAMPLE_IDS: [(&str, [&str; 2]); 6] = [
    (
        "b4630614-a59d-4a5f-8d34-ae9433f01d2a",
        [
            "98e5f9c9-9add-4dad-b0b3-03dcf110f120",
            "4a36b33e-1a36-4600-917e-71eb67be437f",
        ],
    ),
    (
        "5de35d0f-ae81-43b5-aed6-f64057f95ef7",
        [
            "b6a2e16d-69fe-4bed-85c0-cba2d8e7c2d7",
            "910292af-8df9-401a-a821-cc8b4190a69b",
        ],
    ),
    (
        "5a19cfec-a7d4-4bdd-bc35-dd27120a1885",
        [
            "de442e3f-57ae-4ab6-887e-79b75f709116",
            "5724f408-61f7-482a-8672-2e89900216f4",
        ],
    ),
    (
        "278351ac-1a3f-4fab-81a8-ffac8dfbc122",
        [
            "59af9d56-1a5e-48a9-888f-0e349b2af2ea",
            "db651483-9d07-4635-855c-5ace78284da9",
        ],
    ),
    (
        "56e6709f-c1fd-4e5f-abc5-6eeb88f7656c",
        [
            "a1af4be2-6fa6-4214-8495-0e5fbbbf17c5",
            "417de17f-a17c-436a-972c-56d47b4f0f20",
        ],
    ),
    (
        "8e0580bc-f44f-4443-bed8-36b9ea66154a",
        [
            "032d2c0b-48ad-40e4-8c23-85f7e66f5ebb",
            "4d384576-2ff0-4657-b87a-73bbd005a003",
        ],
    ),
];

/// Bounded approved identities; this is membership, never publication authority.
pub(crate) fn approved_sample_ids() -> &'static [(&'static str, [&'static str; 2])] {
    &APPROVED_SAMPLE_IDS
}

pub fn sample_membership(
    source_sha256: &str,
    work_id: &str,
    unit_id: Option<&str>,
) -> SampleMembership {
    if source_sha256 == APPROVED_SAMPLE_SOURCE_SHA256
        && APPROVED_SAMPLE_IDS.iter().any(|(work, units)| {
            *work == work_id && unit_id.is_none_or(|unit| units.contains(&unit))
        })
    {
        SampleMembership::Approved
    } else {
        SampleMembership::OutsideApprovedSample
    }
}

/// Parsed, source-pinned metadata. It is not a rights clearance or an owner authorization.
/// No audio references or image paths are projected from this type.
#[derive(Deserialize)]
pub struct ApprovedSampleSource {
    dataset_id: String,
    story_count: usize,
    chapter_count: usize,
    works: Vec<SampleWork>,
}

#[derive(Deserialize)]
struct SampleWork {
    sample_key: String,
    title: String,
    kind: String,
    description: String,
    language: String,
    chapters: Vec<SampleUnit>,
}

#[derive(Deserialize)]
struct SampleUnit {
    sample_key: String,
    position: u32,
    title: String,
    description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleSourceError {
    DigestMismatch,
    InvalidInput,
}

fn sample_identity(key: &str, purpose: &str) -> String {
    let digest = Sha256::digest(format!("cantos-sample-catalog-v1\0{key}\0{purpose}"));
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

impl ApprovedSampleSource {
    /// Validate the actual bytes before decoding any source-controlled values.
    pub fn parse(bytes: &[u8]) -> Result<Self, SampleSourceError> {
        if bytes.len() != 16_102
            || format!("{:x}", Sha256::digest(bytes)) != APPROVED_SAMPLE_SOURCE_SHA256
        {
            return Err(SampleSourceError::DigestMismatch);
        }
        let input: Self =
            serde_json::from_slice(bytes).map_err(|_| SampleSourceError::InvalidInput)?;
        if input.dataset_id != "cantos-sample-catalog-v1"
            || input.story_count != 6
            || input.chapter_count != 12
            || input.works.len() != 6
        {
            return Err(SampleSourceError::InvalidInput);
        }
        let mut works = std::collections::BTreeSet::new();
        let mut units = std::collections::BTreeSet::new();
        for work in &input.works {
            let id = sample_identity(&work.sample_key, "work");
            if sample_membership(APPROVED_SAMPLE_SOURCE_SHA256, &id, None)
                != SampleMembership::Approved
                || !works.insert(id.clone())
                || work.chapters.len() != 2
            {
                return Err(SampleSourceError::InvalidInput);
            }
            for (index, unit) in work.chapters.iter().enumerate() {
                let unit_id = sample_identity(&unit.sample_key, "chapter");
                if unit.position != index as u32
                    || sample_membership(APPROVED_SAMPLE_SOURCE_SHA256, &id, Some(&unit_id))
                        != SampleMembership::Approved
                    || !units.insert(unit_id)
                {
                    return Err(SampleSourceError::InvalidInput);
                }
            }
        }
        if works.len() != 6 || units.len() != 12 {
            return Err(SampleSourceError::InvalidInput);
        }
        Ok(input)
    }

    /// Compare only the intended public fields, in addition to separately checking the stored
    /// snapshot's digest, owner, current visibility and rights in the transaction.
    pub fn matches_work(&self, work: &super::Work) -> bool {
        self.works.iter().any(|item| {
            sample_identity(&item.sample_key, "work") == work.id().as_str()
                && item.kind == work.kind().as_str()
                && item.title == work.details().title
                && item.description == work.details().synopsis
                && item.language == work.details().language
        })
    }

    pub fn matches_unit(&self, unit: &super::ContentUnit) -> bool {
        self.works.iter().any(|work| {
            sample_identity(&work.sample_key, "work") == unit.work_id().as_str()
                && work.chapters.iter().any(|item| {
                    sample_identity(&item.sample_key, "chapter") == unit.id().as_str()
                        && item.position == unit.position()
                        && item.title == unit.details().title
                        && item.description == unit.details().description
                })
        })
    }
}

/// Facts supplied by the authenticated, serialized owner transaction. The resulting witness
/// identifies only the snapshot reviewed for metadata; it does not write or publish a record.
pub struct PublishFacts<'a> {
    pub actor: &'a super::OwnerId,
    pub owner_active: bool,
    pub work: &'a super::Work,
    pub unit: Option<&'a super::ContentUnit>,
    pub source: &'a ApprovedSampleSource,
    pub evidence: MetadataEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishDenial {
    InactiveOwner,
    NotOwner,
    OutsideApprovedSample,
    PrivateOrUnlisted,
    Archived,
    EvidenceMissingOrRevoked,
}

/// Opaque witness: only `authorize_publish` constructs it; the adapter must still bind exact
/// stored snapshot digests and recheck authentication and authority before commit.
pub struct PublishWitness<'a> {
    work: &'a super::Work,
    unit: Option<&'a super::ContentUnit>,
}

impl<'a> PublishWitness<'a> {
    pub fn work(&self) -> &'a super::Work {
        self.work
    }

    pub fn unit(&self) -> Option<&'a super::ContentUnit> {
        self.unit
    }
}

pub fn authorize_publish(facts: PublishFacts<'_>) -> Result<PublishWitness<'_>, PublishDenial> {
    if !facts.owner_active {
        return Err(PublishDenial::InactiveOwner);
    }
    if facts.actor != facts.work.owner_id() {
        return Err(PublishDenial::NotOwner);
    }
    if !facts.source.matches_work(facts.work)
        || facts.unit.is_some_and(|unit| {
            !facts.source.matches_unit(unit) || unit.work_id() != facts.work.id()
        })
    {
        return Err(PublishDenial::OutsideApprovedSample);
    }
    if facts.work.details().visibility != Visibility::Public {
        return Err(PublishDenial::PrivateOrUnlisted);
    }
    if facts.work.lifecycle() == Lifecycle::Archived
        || facts
            .unit
            .is_some_and(|unit| unit.lifecycle() == Lifecycle::Archived)
    {
        return Err(PublishDenial::Archived);
    }
    if facts.evidence != MetadataEvidence::Confirmed {
        return Err(PublishDenial::EvidenceMissingOrRevoked);
    }
    Ok(PublishWitness {
        work: facts.work,
        unit: facts.unit,
    })
}

/// Latest append-only decision for the exact subject, not a Work's visibility field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisclosureDecision {
    Absent,
    Disclosed,
    Withdrawn,
}

/// Result of resolving the pinned revision and its digest in the authoritative store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotBinding {
    Valid,
    MissingOrMismatched,
}

/// Current, owner-bound rights and source evidence for the exact disclosure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataEvidence {
    Confirmed,
    MissingOrRevoked,
}

/// A unit must have its own decision and an independently visible parent Work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParentDisclosure {
    Work,
    VisibleWork,
    HiddenWork,
}

/// Facts read together. `owner_active` is an observation, not a caller-supplied identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisclosureFacts {
    pub sample: SampleMembership,
    pub decision: DisclosureDecision,
    pub binding: SnapshotBinding,
    pub evidence: MetadataEvidence,
    pub parent: ParentDisclosure,
    pub visibility: Visibility,
    pub lifecycle: Lifecycle,
    pub owner_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataVisibility {
    Hidden,
    Visible,
}

/// Corrupt or inconsistent pinned data is an error, never a successful empty result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisclosureFault {
    InvalidSnapshotBinding,
}

/// Decide only metadata visibility. This result never implies an audio release or playback.
pub fn metadata_visibility(facts: DisclosureFacts) -> Result<MetadataVisibility, DisclosureFault> {
    use DisclosureDecision::{Absent, Disclosed, Withdrawn};
    use MetadataVisibility::{Hidden, Visible};

    if facts.sample != SampleMembership::Approved
        || facts.decision == Absent
        || facts.decision == Withdrawn
        || facts.evidence != MetadataEvidence::Confirmed
        || facts.parent == ParentDisclosure::HiddenWork
        || facts.visibility != Visibility::Public
        || facts.lifecycle == Lifecycle::Archived
        || !facts.owner_active
    {
        return Ok(Hidden);
    }
    if facts.binding != SnapshotBinding::Valid {
        return Err(DisclosureFault::InvalidSnapshotBinding);
    }
    debug_assert_eq!(facts.decision, Disclosed);
    Ok(Visible)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admitted() -> DisclosureFacts {
        DisclosureFacts {
            sample: SampleMembership::Approved,
            decision: DisclosureDecision::Disclosed,
            binding: SnapshotBinding::Valid,
            evidence: MetadataEvidence::Confirmed,
            parent: ParentDisclosure::VisibleWork,
            visibility: Visibility::Public,
            lifecycle: Lifecycle::Draft,
            owner_active: true,
        }
    }

    #[test]
    fn each_authority_gate_independently_hides_metadata() {
        let good = admitted();
        assert_eq!(metadata_visibility(good), Ok(MetadataVisibility::Visible));
        let cases = [
            DisclosureFacts {
                sample: SampleMembership::OutsideApprovedSample,
                ..good
            },
            DisclosureFacts {
                decision: DisclosureDecision::Absent,
                ..good
            },
            DisclosureFacts {
                decision: DisclosureDecision::Withdrawn,
                ..good
            },
            DisclosureFacts {
                evidence: MetadataEvidence::MissingOrRevoked,
                ..good
            },
            DisclosureFacts {
                parent: ParentDisclosure::HiddenWork,
                ..good
            },
            DisclosureFacts {
                visibility: Visibility::Private,
                ..good
            },
            DisclosureFacts {
                visibility: Visibility::Unlisted,
                ..good
            },
            DisclosureFacts {
                lifecycle: Lifecycle::Archived,
                ..good
            },
            DisclosureFacts {
                owner_active: false,
                ..good
            },
        ];
        for facts in cases {
            assert_eq!(metadata_visibility(facts), Ok(MetadataVisibility::Hidden));
        }
    }

    #[test]
    fn exact_pinned_snapshot_is_required_for_an_otherwise_visible_disclosure() {
        let facts = DisclosureFacts {
            binding: SnapshotBinding::MissingOrMismatched,
            ..admitted()
        };
        assert_eq!(
            metadata_visibility(facts),
            Err(DisclosureFault::InvalidSnapshotBinding)
        );
        // A withdrawn decision never makes old data visible even if its binding is now corrupt.
        assert_eq!(
            metadata_visibility(DisclosureFacts {
                decision: DisclosureDecision::Withdrawn,
                ..facts
            }),
            Ok(MetadataVisibility::Hidden)
        );
    }

    #[test]
    fn work_does_not_require_a_parent_but_a_unit_requires_a_visible_work() {
        assert_eq!(
            metadata_visibility(DisclosureFacts {
                parent: ParentDisclosure::Work,
                ..admitted()
            }),
            Ok(MetadataVisibility::Visible)
        );
        assert_eq!(
            metadata_visibility(DisclosureFacts {
                parent: ParentDisclosure::HiddenWork,
                ..admitted()
            }),
            Ok(MetadataVisibility::Hidden)
        );
    }

    #[test]
    fn approved_scope_requires_exact_source_and_work_unit_binding() {
        for (work, units) in APPROVED_SAMPLE_IDS {
            assert_eq!(
                sample_membership(APPROVED_SAMPLE_SOURCE_SHA256, work, None),
                SampleMembership::Approved
            );
            for unit in units {
                assert_eq!(
                    sample_membership(APPROVED_SAMPLE_SOURCE_SHA256, work, Some(unit)),
                    SampleMembership::Approved
                );
                assert_eq!(
                    sample_membership("0", work, Some(unit)),
                    SampleMembership::OutsideApprovedSample
                );
            }
        }
        assert_eq!(
            sample_membership(
                APPROVED_SAMPLE_SOURCE_SHA256,
                APPROVED_SAMPLE_IDS[0].0,
                Some(APPROVED_SAMPLE_IDS[1].1[0])
            ),
            SampleMembership::OutsideApprovedSample
        );
        assert_eq!(
            sample_membership(APPROVED_SAMPLE_SOURCE_SHA256, "other", None),
            SampleMembership::OutsideApprovedSample
        );
    }

    #[test]
    fn approved_ids_match_the_sample_import_identity_algorithm() {
        use sha2::{Digest, Sha256};

        fn identity(key: &str, purpose: &str) -> String {
            let hash = Sha256::digest(format!("cantos-sample-catalog-v1\0{key}\0{purpose}"));
            let mut bytes = [0_u8; 16];
            bytes.copy_from_slice(&hash[..16]);
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;
            uuid::Uuid::from_bytes(bytes).to_string()
        }

        let keys = [
            "ben-suong",
            "nguoi-giu-den",
            "thanh-pho-ngu-quen",
            "khu-vuon-tren-may",
            "chuyen-tau-cuoi-cung",
            "mua-gio-qua-nha",
        ];
        for (key, (work, units)) in keys.into_iter().zip(APPROVED_SAMPLE_IDS) {
            assert_eq!(identity(key, "work"), work);
            for (index, unit) in units.into_iter().enumerate() {
                assert_eq!(
                    identity(&format!("{key}/chapter-{:02}", index + 1), "chapter"),
                    unit
                );
            }
        }
    }

    #[test]
    fn an_unpinned_source_is_rejected_before_json_is_trusted() {
        assert!(matches!(
            ApprovedSampleSource::parse(b"{}"),
            Err(SampleSourceError::DigestMismatch)
        ));
    }

    #[test]
    fn authorization_requires_owner_evidence_and_exact_source_metadata() {
        use super::super::{NewWork, OwnerId, Work, WorkDetails, WorkId, WorkKind};
        // Internal test fixture for the pure decision; it is not an admitted external source.
        let source = ApprovedSampleSource {
            dataset_id: "cantos-sample-catalog-v1".into(),
            story_count: 6,
            chapter_count: 12,
            works: vec![SampleWork {
                sample_key: "ben-suong".into(),
                title: "Fixture".into(),
                kind: "story".into(),
                description: "Fixture description".into(),
                language: "vi-VN".into(),
                chapters: vec![],
            }],
        };
        let owner = OwnerId::parse("owner").unwrap();
        let details = WorkDetails {
            title: "Fixture".into(),
            synopsis: "Fixture description".into(),
            language: "vi-VN".into(),
            visibility: Visibility::Public,
            rights_refs: vec![],
        };
        let work = Work::new(NewWork {
            id: WorkId::parse(APPROVED_SAMPLE_IDS[0].0).unwrap(),
            kind: WorkKind::Story,
            owner_id: owner.clone(),
            details: details.clone(),
            at: 1,
        })
        .unwrap();
        let admitted = || PublishFacts {
            actor: &owner,
            owner_active: true,
            work: &work,
            unit: None,
            source: &source,
            evidence: MetadataEvidence::Confirmed,
        };
        assert!(authorize_publish(admitted()).is_ok());
        let other = OwnerId::parse("other").unwrap();
        assert!(matches!(
            authorize_publish(PublishFacts {
                actor: &other,
                ..admitted()
            }),
            Err(PublishDenial::NotOwner)
        ));
        assert!(matches!(
            authorize_publish(PublishFacts {
                evidence: MetadataEvidence::MissingOrRevoked,
                ..admitted()
            }),
            Err(PublishDenial::EvidenceMissingOrRevoked)
        ));
        assert!(matches!(
            authorize_publish(PublishFacts {
                owner_active: false,
                ..admitted()
            }),
            Err(PublishDenial::InactiveOwner)
        ));
        let mut edited = details.clone();
        edited.title.push('!');
        let changed = work.edit(1, edited, 2).unwrap();
        assert!(matches!(
            authorize_publish(PublishFacts {
                work: &changed,
                ..admitted()
            }),
            Err(PublishDenial::OutsideApprovedSample)
        ));
        let mut private_details = details;
        private_details.visibility = Visibility::Private;
        let private = work.edit(1, private_details, 2).unwrap();
        assert!(matches!(
            authorize_publish(PublishFacts {
                work: &private,
                ..admitted()
            }),
            Err(PublishDenial::PrivateOrUnlisted)
        ));
    }

    /// Run only with the authorized external #427 file, never a checked-in manuscript or media.
    #[test]
    #[ignore = "requires the externally supplied sample-catalog.json"]
    fn supplied_source_has_exact_approved_membership() {
        use super::super::{
            ContentUnit, ContentUnitId, NewContentUnit, NewWork, OwnerId, UnitDetails, Work,
            WorkDetails, WorkId, WorkKind,
        };
        let path = std::env::var_os("CANTOS_APPROVED_SAMPLE_INPUT").expect("sample input path");
        let mut bytes = std::fs::read(path).expect("read approved sample");
        let source = ApprovedSampleSource::parse(&bytes).expect("pinned sample");
        assert_eq!(source.works.len(), 6);
        assert_eq!(
            source
                .works
                .iter()
                .map(|work| work.chapters.len())
                .sum::<usize>(),
            12
        );
        for item in &source.works {
            let details = WorkDetails {
                title: item.title.clone(),
                synopsis: item.description.clone(),
                language: item.language.clone(),
                visibility: Visibility::Public,
                rights_refs: vec![],
            };
            let work = Work::new(NewWork {
                id: WorkId::parse(&sample_identity(&item.sample_key, "work")).unwrap(),
                kind: WorkKind::parse(&item.kind).unwrap(),
                owner_id: OwnerId::parse("sample_owner").unwrap(),
                details: details.clone(),
                at: 1,
            })
            .unwrap();
            assert!(source.matches_work(&work));
            let admitted = || PublishFacts {
                actor: work.owner_id(),
                owner_active: true,
                work: &work,
                unit: None,
                source: &source,
                evidence: MetadataEvidence::Confirmed,
            };
            assert!(authorize_publish(admitted()).is_ok());
            assert!(matches!(
                authorize_publish(PublishFacts {
                    owner_active: false,
                    ..admitted()
                }),
                Err(PublishDenial::InactiveOwner)
            ));
            let other = OwnerId::parse("other_owner").unwrap();
            assert!(matches!(
                authorize_publish(PublishFacts {
                    actor: &other,
                    ..admitted()
                }),
                Err(PublishDenial::NotOwner)
            ));
            assert!(matches!(
                authorize_publish(PublishFacts {
                    evidence: MetadataEvidence::MissingOrRevoked,
                    ..admitted()
                }),
                Err(PublishDenial::EvidenceMissingOrRevoked)
            ));
            let mut changed = details;
            changed.title.push('!');
            let changed_work = work.edit(1, changed, 2).unwrap();
            assert!(!source.matches_work(&changed_work));
            assert!(matches!(
                authorize_publish(PublishFacts {
                    work: &changed_work,
                    ..admitted()
                }),
                Err(PublishDenial::OutsideApprovedSample)
            ));
            for chapter in &item.chapters {
                let details = UnitDetails {
                    title: chapter.title.clone(),
                    description: chapter.description.clone(),
                    source: None,
                    script: None,
                    content_digest: None,
                };
                let unit = ContentUnit::new(
                    &work,
                    NewContentUnit {
                        id: ContentUnitId::parse(&sample_identity(&chapter.sample_key, "chapter"))
                            .unwrap(),
                        kind: work.kind().content_unit_kind(),
                        details,
                        position: chapter.position,
                        at: 1,
                    },
                )
                .unwrap();
                assert!(source.matches_unit(&unit));
                assert!(authorize_publish(PublishFacts {
                    unit: Some(&unit),
                    ..admitted()
                })
                .is_ok());
                let mut edited = unit.details().clone();
                edited.title.push('!');
                let altered = unit.edit(&work, 1, edited, 2).unwrap();
                assert!(matches!(
                    authorize_publish(PublishFacts {
                        unit: Some(&altered),
                        ..admitted()
                    }),
                    Err(PublishDenial::OutsideApprovedSample)
                ));
            }
        }
        bytes[0] ^= 1;
        assert!(matches!(
            ApprovedSampleSource::parse(&bytes),
            Err(SampleSourceError::DigestMismatch)
        ));
    }
}
