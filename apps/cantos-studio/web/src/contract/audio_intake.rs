//! Development-only Studio owner audio intake. Raw transport values are not authorization,
//! media-inspection, rights, quality-review or publication witnesses.
use serde::{Deserialize, Serialize};

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
