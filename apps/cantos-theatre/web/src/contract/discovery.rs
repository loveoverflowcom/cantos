//! Theatre Discovery/Playback v1. Public projections only; no Studio audit or private assets.
use serde::{Deserialize, Deserializer, Serialize};

fn positive_safe_integer<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let value = u64::deserialize(deserializer)?;
    if (1..=9_007_199_254_740_991).contains(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "expected a positive JSON-safe integer",
        ))
    }
}

fn required_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkKind {
    Novel,
    Story,
    Podcast,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnitKind {
    Chapter,
    Episode,
}
/// Opt-in presentation v1.1; the default v1 summary remains unchanged.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentedWorkSummary {
    pub work_id: String,
    pub kind: WorkKind,
    pub title: String,
    pub description: String,
    pub language: String,
    pub first_published_at_ms: i64,
    /// Currently eligible public units, not editorial/draft units or an authored total.
    #[serde(deserialize_with = "positive_safe_integer")]
    pub unit_count: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentedWorkPage {
    pub items: Vec<PresentedWorkSummary>,
    #[serde(deserialize_with = "required_nullable")]
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentedUnitSummary {
    pub unit_id: String,
    pub kind: UnitKind,
    pub position: u32,
    pub title: String,
    pub description: String,
    pub publication_id: String,
    pub published_at_ms: i64,
    pub duration_ms: u64,
    /// One-based rank among currently eligible units in immutable public-anchor order.
    #[serde(deserialize_with = "positive_safe_integer")]
    pub ordinal: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentedUnitPage {
    pub work_id: String,
    pub items: Vec<PresentedUnitSummary>,
    #[serde(deserialize_with = "required_nullable")]
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublicationManifest {
    pub schema_version: String,
    pub publication_id: String,
    pub work_id: String,
    pub unit_id: String,
    pub rendition_id: String,
    pub manifest_digest: String,
    pub sha256: String,
    pub byte_length: u64,
    pub duration_ms: u64,
    pub container: String,
    pub codec: String,
    pub channels: u32,
    pub sample_rate_hz: u32,
    pub profile_id: String,
    pub language: String,
    pub published_at_ms: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlaybackAuthorization {
    pub publication_id: String,
    pub rendition_id: String,
    pub sha256: String,
    pub url: String,
    pub expires_at_ms: i64,
    pub content_type: String,
    pub byte_range_supported: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ListenerErrorCode {
    InvalidRequest,
    InvalidCursor,
    NotFound,
    RenditionUnavailable,
    Unavailable,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ListenerError {
    pub code: ListenerErrorCode,
    pub retryable: bool,
}
