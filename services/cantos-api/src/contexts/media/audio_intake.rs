//! Read-only byte inspection before private storage acceptance. This is never an encoder.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub use super::domain::audio_profile::DeliveryProfile;
use super::{process, structure};

pub const MAX_AUDIO_BYTES: usize = 32 * 1024 * 1024;
const MAX_DURATION_SECONDS: f64 = 3600.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioIntakeError {
    EmptyAudio,
    AudioTooLarge,
    MalformedAudio,
    UnsupportedContainer,
    CodecMismatch,
    ChannelMismatch,
    SampleRateMismatch,
    DurationLimit,
    BitrateLimit,
    InspectionUnavailable,
    InspectionTimeout,
    InspectionLimit,
}

impl AudioIntakeError {
    pub fn code(self) -> &'static str {
        match self {
            Self::EmptyAudio => "empty_audio",
            Self::AudioTooLarge => "audio_too_large",
            Self::MalformedAudio => "malformed_audio",
            Self::UnsupportedContainer => "unsupported_container",
            Self::CodecMismatch => "codec_mismatch",
            Self::ChannelMismatch => "channel_mismatch",
            Self::SampleRateMismatch => "sample_rate_mismatch",
            Self::DurationLimit => "duration_limit",
            Self::BitrateLimit => "bitrate_limit",
            Self::InspectionUnavailable => "inspection_unavailable",
            Self::InspectionTimeout => "inspection_timeout",
            Self::InspectionLimit => "inspection_limit",
        }
    }

    pub fn is_unavailable(self) -> bool {
        matches!(
            self,
            Self::InspectionUnavailable | Self::InspectionTimeout | Self::InspectionLimit
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioMetadata {
    pub sha256: String,
    pub byte_length: u64,
    pub container: String,
    pub codec: String,
    pub channels: u32,
    pub sample_rate_hz: u32,
    pub duration_ms: u64,
    pub audio_bitrate_bps: u64,
    pub profile_id: String,
}

/// Only successful actual-byte inspection can construct this witness; no serde or raw constructor.
#[derive(Debug)]
pub struct VerifiedAudio {
    bytes: Vec<u8>,
    metadata: AudioMetadata,
}

impl VerifiedAudio {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn metadata(&self) -> &AudioMetadata {
        &self.metadata
    }
}

/// Caller must run this synchronous bounded adapter off the async executor and cap concurrency.
/// Missing tools/non-POSIX hosts fail closed; decoder output is discarded, never saved/transcoded.
pub fn inspect_audio(
    bytes: &[u8],
    profile: DeliveryProfile,
) -> Result<VerifiedAudio, AudioIntakeError> {
    if bytes.is_empty() {
        return Err(AudioIntakeError::EmptyAudio);
    }
    if bytes.len() > MAX_AUDIO_BYTES {
        return Err(AudioIntakeError::AudioTooLarge);
    }
    let container = structure::check(bytes)?;
    if container.name != profile.container() {
        return Err(AudioIntakeError::UnsupportedContainer);
    }
    let temporary = Quarantine::create(bytes)?;
    let facts = probe(temporary.path())?;
    let (packet_duration, bitrate) = packets(temporary.path())?;
    if (packet_duration - container.duration_seconds).abs() > 0.125 {
        return Err(AudioIntakeError::MalformedAudio);
    }
    let metadata = validate_facts(facts, container.duration_seconds, bitrate, profile, bytes)?;
    decode(temporary.path())?;
    Ok(VerifiedAudio {
        bytes: bytes.to_vec(),
        metadata,
    })
}

#[derive(Deserialize)]
struct Probe {
    streams: Vec<Stream>,
    format: Format,
}
#[derive(Deserialize)]
struct Stream {
    codec_name: String,
    codec_type: String,
    channels: Option<u32>,
    sample_rate: Option<String>,
}
#[derive(Deserialize)]
struct Format {
    format_name: String,
}

fn command(tool: &str, path: &Path) -> Command {
    let mut command = Command::new(tool);
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-threads",
        "1",
        "-protocol_whitelist",
        "file",
        "-format_whitelist",
        "ogg",
    ]);
    command.current_dir(path.parent().unwrap_or_else(|| Path::new("/")));
    command
}

fn probe(path: &Path) -> Result<Probe, AudioIntakeError> {
    let mut tool = command("ffprobe", path);
    tool.args([
        "-show_entries",
        "stream=codec_name,codec_type,channels,sample_rate:format=format_name",
        "-of",
        "json",
    ])
    .arg(path);
    let output = process::run(&mut tool)?;
    serde_json::from_slice(&output).map_err(|_| AudioIntakeError::MalformedAudio)
}

fn packets(path: &Path) -> Result<(f64, u64), AudioIntakeError> {
    let mut tool = command("ffprobe", path);
    tool.args([
        "-show_packets",
        "-show_entries",
        "packet=size,duration_time",
        "-of",
        "compact=p=0:nk=0",
    ])
    .arg(path);
    let output = process::run(&mut tool)?;
    let text = std::str::from_utf8(&output).map_err(|_| AudioIntakeError::MalformedAudio)?;
    packet_facts(text)
}

fn packet_facts(text: &str) -> Result<(f64, u64), AudioIntakeError> {
    let mut duration = 0.0;
    let mut bytes = 0_u64;
    let mut packet_count = 0_u32;
    // ffprobe6 emits empty separators after packet side-data blocks. These contain
    // no packet facts; nonempty lines must still have exact positive size/duration.
    for line in text.lines().filter(|line| !line.is_empty()) {
        let mut size = None;
        let mut seconds = None;
        for item in line.split('|') {
            if let Some(value) = item.strip_prefix("size=") {
                size = value.parse::<u64>().ok();
            }
            if let Some(value) = item.strip_prefix("duration_time=") {
                seconds = value.parse::<f64>().ok();
            }
        }
        let size = size
            .filter(|size| *size > 0)
            .ok_or(AudioIntakeError::MalformedAudio)?;
        let seconds = seconds
            .filter(|seconds| seconds.is_finite() && *seconds > 0.0 && *seconds <= 0.12)
            .ok_or(AudioIntakeError::MalformedAudio)?;
        bytes = bytes
            .checked_add(size)
            .ok_or(AudioIntakeError::MalformedAudio)?;
        duration += seconds;
        packet_count += 1;
        if duration > MAX_DURATION_SECONDS + 0.125 || packet_count > 1_440_000 {
            return Err(AudioIntakeError::DurationLimit);
        }
    }
    if !duration.is_finite() || duration <= 0.0 {
        return Err(AudioIntakeError::MalformedAudio);
    }
    // Demuxed coded payload excludes Ogg pages, tags and headers; use conservative ceil.
    Ok((duration, ((bytes as f64 * 8.0) / duration).ceil() as u64))
}

fn validate_facts(
    facts: Probe,
    duration: f64,
    bitrate: u64,
    profile: DeliveryProfile,
    bytes: &[u8],
) -> Result<AudioMetadata, AudioIntakeError> {
    if facts.format.format_name != "ogg" {
        return Err(AudioIntakeError::UnsupportedContainer);
    }
    if facts.streams.len() != 1 {
        return Err(AudioIntakeError::MalformedAudio);
    }
    let stream = &facts.streams[0];
    if stream.codec_type != "audio" || stream.codec_name != profile.codec() {
        return Err(AudioIntakeError::CodecMismatch);
    }
    let channels = stream.channels.ok_or(AudioIntakeError::MalformedAudio)?;
    if channels != 1 {
        return Err(AudioIntakeError::ChannelMismatch);
    }
    let rate = stream
        .sample_rate
        .as_ref()
        .and_then(|value| value.parse().ok())
        .ok_or(AudioIntakeError::MalformedAudio)?;
    if rate != 48_000 {
        return Err(AudioIntakeError::SampleRateMismatch);
    }
    if !duration.is_finite() || duration <= 0.0 || duration > MAX_DURATION_SECONDS {
        return Err(AudioIntakeError::DurationLimit);
    }
    if bitrate > profile.maximum_bitrate_bps() {
        return Err(AudioIntakeError::BitrateLimit);
    }
    Ok(AudioMetadata {
        sha256: format!("{:x}", Sha256::digest(bytes)),
        byte_length: bytes.len() as u64,
        container: "ogg".to_owned(),
        codec: "opus".to_owned(),
        channels,
        sample_rate_hz: rate,
        duration_ms: (duration * 1000.0).ceil() as u64,
        audio_bitrate_bps: bitrate,
        profile_id: profile.id().to_owned(),
    })
}

fn decode(path: &Path) -> Result<(), AudioIntakeError> {
    let mut tool = command("ffmpeg", path);
    tool.args(["-xerror", "-err_detect", "explode", "-i"])
        .arg(path)
        .args(["-map", "0:a:0", "-threads", "1", "-f", "null", "-"]);
    process::run(&mut tool).map(|_| ())
}

struct Quarantine {
    directory: PathBuf,
    path: PathBuf,
}
impl Quarantine {
    #[cfg(unix)]
    fn create(bytes: &[u8]) -> Result<Self, AudioIntakeError> {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        let directory =
            std::env::temp_dir().join(format!("cantos-inspect-{}", Uuid::new_v4().simple()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|_| AudioIntakeError::InspectionUnavailable)?;
        let quarantine = Self {
            path: directory.join("audio.bin"),
            directory,
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&quarantine.path)
            .map_err(|_| AudioIntakeError::InspectionUnavailable)?;
        file.write_all(bytes)
            .map_err(|_| AudioIntakeError::InspectionUnavailable)?;
        Ok(quarantine)
    }
    #[cfg(not(unix))]
    fn create(_: &[u8]) -> Result<Self, AudioIntakeError> {
        Err(AudioIntakeError::InspectionUnavailable)
    }
    fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for Quarantine {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(channels: u32, codec: &str, sample_rate: &str) -> Probe {
        Probe {
            streams: vec![Stream {
                codec_name: codec.to_owned(),
                codec_type: "audio".to_owned(),
                channels: Some(channels),
                sample_rate: Some(sample_rate.to_owned()),
            }],
            format: Format {
                format_name: "ogg".to_owned(),
            },
        }
    }

    #[test]
    fn metadata_is_checked_against_the_selected_versioned_profile() {
        let profile = DeliveryProfile::OpusMono24V1;
        let bytes = b"test content identity";
        assert_eq!(
            validate_facts(facts(2, "opus", "48000"), 1.0, 24_000, profile, bytes),
            Err(AudioIntakeError::ChannelMismatch)
        );
        assert_eq!(
            validate_facts(facts(1, "vorbis", "48000"), 1.0, 24_000, profile, bytes),
            Err(AudioIntakeError::CodecMismatch)
        );
        assert_eq!(
            validate_facts(facts(1, "opus", "44100"), 1.0, 24_000, profile, bytes),
            Err(AudioIntakeError::SampleRateMismatch)
        );
        assert_eq!(
            validate_facts(facts(1, "opus", "48000"), 1.0, 28_801, profile, bytes),
            Err(AudioIntakeError::BitrateLimit)
        );
        assert_eq!(
            validate_facts(facts(1, "opus", "48000"), 3600.001, 24_000, profile, bytes),
            Err(AudioIntakeError::DurationLimit)
        );
        let metadata =
            validate_facts(facts(1, "opus", "48000"), 1.0, 28_800, profile, bytes).unwrap();
        assert_eq!(metadata.sha256, format!("{:x}", Sha256::digest(bytes)));
        assert_eq!(metadata.audio_bitrate_bps, 28_800);
        assert_eq!(metadata.profile_id, "opus-mono-24-v1");
    }

    #[test]
    fn exactly_one_hour_allows_encoder_pre_skip_without_relaxing_audible_duration() {
        let mut packet_text = "duration_time=0.020000|size=60\n".repeat(180_000);
        packet_text.push_str("duration_time=0.006500|size=60\n");
        let (duration, _) = packet_facts(&packet_text).unwrap();
        assert!((duration - 3600.0065).abs() < 0.0001);
        let metadata = validate_facts(
            facts(1, "opus", "48000"),
            3600.0,
            24_000,
            DeliveryProfile::OpusMono24V1,
            b"reference",
        )
        .unwrap();
        assert_eq!(metadata.duration_ms, 3_600_000);
        assert_eq!(
            validate_facts(
                facts(1, "opus", "48000"),
                3600.001,
                24_000,
                DeliveryProfile::OpusMono24V1,
                b"reference"
            ),
            Err(AudioIntakeError::DurationLimit)
        );
    }

    #[test]
    fn empty_oversized_and_spoofed_audio_never_reach_a_decoder() {
        let profile = DeliveryProfile::OpusMono24V1;
        assert_eq!(
            inspect_audio(&[], profile).unwrap_err(),
            AudioIntakeError::EmptyAudio
        );
        assert_eq!(
            inspect_audio(&vec![0; MAX_AUDIO_BYTES + 1], profile).unwrap_err(),
            AudioIntakeError::AudioTooLarge
        );
        assert_eq!(
            inspect_audio(b"renamed file.opus", profile).unwrap_err(),
            AudioIntakeError::UnsupportedContainer
        );
    }
}

#[cfg(test)]
mod packet_output_tests {
    use super::*;
    #[test]
    fn ffprobe_packet_side_data_empty_separators_do_not_add_or_remove_packet_facts() {
        assert_eq!(packet_facts("duration_time=0.020000|size=60|side_data_type=Skip Samples\n\nduration_time=0.020000|size=60\n"),Ok((0.04,24000)));
        assert_eq!(packet_facts("\n\n"), Err(AudioIntakeError::MalformedAudio));
        assert_eq!(
            packet_facts("duration_time=0.020000|size=60\nunknown_nonempty_data\n"),
            Err(AudioIntakeError::MalformedAudio)
        );
    }
}
