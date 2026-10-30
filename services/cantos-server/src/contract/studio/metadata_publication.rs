//! Raw owner requests and state for Work/Unit metadata publication generations (ADR 0054).
//! Canonical schema and fixtures: `contracts/studio/metadata-publication/v1/`.
use serde::{Deserialize, Deserializer, Serialize};

/// One selected Unit revision. Its index in `units` is its published position.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MetadataPublicationUnit {
    pub unit_id: String,
    pub revision: u64,
}

/// Every command names the pointer version it read (`0` before the first publication).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum MetadataPublicationRequest {
    /// The complete ordered selection of exact saved revisions.
    Publish {
        operation_id: String,
        expected_version: u64,
        work_revision: u64,
        units: Vec<MetadataPublicationUnit>,
        /// Owner's explicit claim that this metadata may be shown publicly; not legal clearance
        /// and never consent for audio, covers or transcripts.
        confirm_metadata_rights: bool,
    },
    /// Add or replace one Unit at a one-based published position, copying every other entry.
    PublishUnit {
        operation_id: String,
        expected_version: u64,
        unit_id: String,
        revision: u64,
        position: u32,
        confirm_metadata_rights: bool,
    },
    WithdrawUnit {
        operation_id: String,
        expected_version: u64,
        unit_id: String,
    },
    Withdraw {
        operation_id: String,
        expected_version: u64,
    },
    /// Associate one published Unit with the release its audio pointer names now. An exact pin
    /// needs no confirmation; a display-only difference needs `confirm_display_only_drift`.
    AssociateAudio {
        operation_id: String,
        expected_version: u64,
        unit_id: String,
        release_id: String,
        confirm_display_only_drift: bool,
    },
}

impl MetadataPublicationRequest {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::Publish { operation_id, .. }
            | Self::PublishUnit { operation_id, .. }
            | Self::WithdrawUnit { operation_id, .. }
            | Self::Withdraw { operation_id, .. }
            | Self::AssociateAudio { operation_id, .. } => operation_id,
        }
    }

    pub fn expected_version(&self) -> u64 {
        match self {
            Self::Publish {
                expected_version, ..
            }
            | Self::PublishUnit {
                expected_version, ..
            }
            | Self::WithdrawUnit {
                expected_version, ..
            }
            | Self::Withdraw {
                expected_version, ..
            }
            | Self::AssociateAudio {
                expected_version, ..
            } => *expected_version,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetadataPublicationLifecycle {
    NeverPublished,
    Active,
    Withdrawn,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetadataAudioBasis {
    Exact,
    DisplayOnly,
}

/// The exact release an entry is associated with. A fact, not a playback state: the release
/// may since have been withdrawn or superseded, which the live listener gate applies.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MetadataGenerationAudio {
    pub release_id: String,
    pub release_digest: String,
    pub basis: MetadataAudioBasis,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MetadataGenerationEntry {
    pub position: u32,
    pub unit_id: String,
    pub revision: u64,
    pub digest: String,
    /// `null` is metadata-only.
    #[serde(deserialize_with = "required_nullable")]
    pub audio: Option<MetadataGenerationAudio>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MetadataGeneration {
    pub number: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub parent: Option<u64>,
    pub work_revision: u64,
    pub work_digest: String,
    pub recorded_at_ms: i64,
    pub entries: Vec<MetadataGenerationEntry>,
}

/// The pointer as one read or one settled command sees it. `never_published` has version `0`
/// and null generation/anchor; an unread or failed state is an error response, never this.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MetadataPublicationState {
    pub work_id: String,
    pub lifecycle: MetadataPublicationLifecycle,
    pub version: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub first_visible_at_ms: Option<i64>,
    #[serde(deserialize_with = "required_nullable")]
    pub generation: Option<MetadataGeneration>,
}

/// Omission is rejected: `null` is the only spelling of "no generation" or "no anchor".
fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
