//! Development-only Studio owner audio intake. Raw transport values are not authorization,
//! media-inspection, rights, quality-review or publication witnesses.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateAudioIntentRequest {
    pub operation_id: String,
    pub expected_revision: u64,
    pub expected_content_digest: String,
    /// Explicit canonical versioned policy; there is no default or automatic selection.
    pub profile_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioIntentResponse {
    pub id: String,
    pub script_id: String,
    pub script_revision: u64,
    pub content_digest: String,
    pub profile_id: String,
    pub operation_id: String,
    pub created_by: String,
    pub created_at: String,
    /// Authenticated same-origin API path, never an object path or public/signed URL.
    pub upload_path: String,
    pub status_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioReceiptResponse {
    pub id: String,
    pub intent_id: String,
    pub script_id: String,
    pub script_revision: u64,
    pub content_digest: String,
    pub profile_id: String,
    pub operation_id: String,
    pub accepted_by: String,
    pub accepted_at: String,
    pub media: AudioMediaMetadata,
}

/// Server-observed private media facts. No file bytes, storage keys or playback URLs.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioMediaMetadata {
    pub sha256: String,
    pub byte_length: u64,
    pub container: String,
    pub codec: String,
    pub channels: u32,
    pub sample_rate_hz: u32,
    pub duration_ms: u64,
    /// Average coded audio payload bitrate, excluding container overhead.
    pub audio_bitrate_bps: u64,
    pub profile_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioIntentStatusResponse {
    pub intent: AudioIntentResponse,
    /// None means no accepted receipt; it conveys no review or publication decision.
    pub receipt: Option<AudioReceiptResponse>,
}
