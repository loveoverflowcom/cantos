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

fn safe_integer<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let value = u64::deserialize(deserializer)?;
    if value <= 9_007_199_254_740_991 {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("expected a JSON-safe integer"))
    }
}
fn safe_timestamp<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    let value = i64::deserialize(deserializer)?;
    if (0..=9_007_199_254_740_991).contains(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom("expected a JSON-safe timestamp"))
    }
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
fn cover_bound<'de, D: Deserializer<'de>, const MAX: u64>(
    deserializer: D,
) -> Result<u64, D::Error> {
    let value = u64::deserialize(deserializer)?;
    if (1..=MAX).contains(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "expected an integer within the cover bounds",
        ))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum CoverMime {
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/avif")]
    Avif,
}

/// Public image facts only (Discovery v1.2, ADR 0033): no asset ID, rights, provenance, owner or
/// storage key. `url` is used exactly as given; the application layer admits only the shape the
/// contract promises (see `application::public_cover`).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CoverImage {
    pub url: String,
    pub sha256: String,
    pub content_type: CoverMime,
    #[serde(deserialize_with = "cover_bound::<_, 4096>")]
    pub width_px: u64,
    #[serde(deserialize_with = "cover_bound::<_, 4096>")]
    pub height_px: u64,
    #[serde(deserialize_with = "cover_bound::<_, 8_388_608>")]
    pub byte_length: u64,
    pub alt_text: String,
}

/// Discovery v1.2 (`presentation=2`): every presentation v1.1 field plus a **required nullable**
/// `cover`. `null` is the only no-cover state; a missing `cover` key is a protocol error.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CoveredWorkSummary {
    pub work_id: String,
    pub kind: WorkKind,
    pub title: String,
    pub description: String,
    pub language: String,
    pub first_published_at_ms: i64,
    /// Currently eligible public units, not editorial/draft units or an authored total.
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

/// Opt-in public metadata, with no implied audio release.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MetadataWorkSummary {
    pub work_id: String,
    pub kind: WorkKind,
    pub title: String,
    pub description: String,
    pub language: String,
    #[serde(deserialize_with = "safe_timestamp")]
    pub first_visible_at_ms: i64,
    #[serde(deserialize_with = "safe_integer")]
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
    #[serde(deserialize_with = "safe_timestamp")]
    pub published_at_ms: i64,
    #[serde(deserialize_with = "positive_safe_integer")]
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
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
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

/// `url` is a short-lived playback capability: a `{:?}` of the grant (or of anything holding it)
/// must not print it. The admitted `MediaUrl` redacts the same way once the grant is checked.
impl std::fmt::Debug for PlaybackAuthorization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlaybackAuthorization")
            .field("publication_id", &self.publication_id)
            .field("rendition_id", &self.rendition_id)
            .field("sha256", &self.sha256)
            .field("url", &"[redacted]")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("content_type", &self.content_type)
            .field("byte_range_supported", &self.byte_range_supported)
            .finish()
    }
}

#[cfg(test)]
mod playback_debug_tests {
    use super::*;

    #[test]
    fn debug_output_never_contains_the_capability_url() {
        let grant = PlaybackAuthorization {
            publication_id: "p".into(),
            rendition_id: "r".into(),
            sha256: "s".into(),
            url: "https://media.example/pg1_SECRET-CAPABILITY".into(),
            expires_at_ms: 1,
            content_type: "audio/ogg".into(),
            byte_range_supported: true,
        };
        let shown = format!("{grant:?}");
        assert!(!shown.contains("SECRET-CAPABILITY"), "{shown}");
        assert!(shown.contains("[redacted]"));
    }
}
