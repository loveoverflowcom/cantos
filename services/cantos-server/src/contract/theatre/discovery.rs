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

fn bounded<'de, D: Deserializer<'de>, const MAX: u64>(deserializer: D) -> Result<u64, D::Error> {
    let value = u64::deserialize(deserializer)?;
    if (1..=MAX).contains(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "expected an integer within the cover bounds",
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
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkSummary {
    pub work_id: String,
    pub kind: WorkKind,
    pub title: String,
    pub description: String,
    pub language: String,
    /// First actual publication, immutable across replacement; Unix milliseconds.
    pub first_published_at_ms: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkPage {
    pub items: Vec<WorkSummary>,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitSummary {
    pub unit_id: String,
    pub kind: UnitKind,
    pub position: u32,
    pub title: String,
    pub description: String,
    pub publication_id: String,
    pub published_at_ms: i64,
    pub duration_ms: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitPage {
    pub work_id: String,
    pub items: Vec<UnitSummary>,
    pub next_cursor: Option<String>,
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
/// Accepted Discovery v1.2 cover (ADR 0033, `presentation=2`); not served until #171.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum CoverMime {
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/jpeg")]
    Jpeg,
}
/// Public image facts only: no asset ID/digest, rights, provenance, owner or storage key.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CoverImage {
    /// Origin delivery URL, used exactly as given; revalidated on every use.
    pub url: String,
    pub sha256: String,
    pub content_type: CoverMime,
    #[serde(deserialize_with = "bounded::<_, 4096>")]
    pub width_px: u64,
    #[serde(deserialize_with = "bounded::<_, 4096>")]
    pub height_px: u64,
    #[serde(deserialize_with = "bounded::<_, 8_388_608>")]
    pub byte_length: u64,
    pub alt_text: String,
}
/// `PresentedWorkSummary` plus a required nullable cover; `null` is the only no-cover state.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CoveredWorkSummary {
    pub work_id: String,
    pub kind: WorkKind,
    pub title: String,
    pub description: String,
    pub language: String,
    pub first_published_at_ms: i64,
    #[serde(deserialize_with = "positive_safe_integer")]
    pub unit_count: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub cover: Option<CoverImage>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CoveredWorkPage {
    pub items: Vec<CoveredWorkSummary>,
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

/// Opt-in metadata disclosure presentation. No field here implies audio publication.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MetadataWorkSummary {
    pub work_id: String,
    pub kind: WorkKind,
    pub title: String,
    pub description: String,
    pub language: String,
    pub first_visible_at_ms: i64,
    pub unit_count: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub cover: Option<CoverImage>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MetadataWorkPage {
    pub items: Vec<MetadataWorkSummary>,
    #[serde(deserialize_with = "required_nullable")]
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitPlayback {
    pub publication_id: String,
    pub published_at_ms: i64,
    pub duration_ms: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MetadataUnitSummary {
    pub unit_id: String,
    pub kind: UnitKind,
    pub position: u32,
    pub title: String,
    pub description: String,
    #[serde(deserialize_with = "positive_safe_integer")]
    pub ordinal: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub playback: Option<UnitPlayback>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MetadataUnitPage {
    pub work_id: String,
    pub items: Vec<MetadataUnitSummary>,
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
