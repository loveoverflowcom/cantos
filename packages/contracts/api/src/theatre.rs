//! Theatre Discovery/Playback v1. Public projections only; no Studio audit or private assets.
use serde::{Deserialize, Serialize};

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
