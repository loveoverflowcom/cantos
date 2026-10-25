//! Owner public-display decision for one exact admitted Work image (ADR 0033 rule 2). JSON
//! only; no Protobuf message is selected. A raw DTO is not authority, consent or an image.
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoverDisclosureKind {
    Cleared,
    Revoked,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordCoverDisclosureRequest {
    pub operation_id: String,
    pub image_asset_digest: String,
    pub decision: CoverDisclosureKind,
    /// Required for `cleared` (exclusive expiry, Unix ms); explicit `null` for `revoked`.
    /// Omission is rejected: a clearance is never perpetual by default.
    #[serde(deserialize_with = "required_nullable")]
    pub valid_until_ms: Option<u64>,
}

/// The immutable recorded decision. `sequence` is the server's per-Work order; the latest
/// sequence for a Work and image digest is the current decision.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CoverDisclosureResponse {
    pub work_id: String,
    pub sequence: u64,
    pub image_asset_digest: String,
    pub decision: CoverDisclosureKind,
    #[serde(deserialize_with = "required_nullable")]
    pub valid_until_ms: Option<u64>,
    pub recorded_at_ms: u64,
}

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
