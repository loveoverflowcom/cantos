//! Bounded structural admission of bytes for the offline TTS receipt audit.
//!
//! Explicit profiles admit ordinary RIFF/WAVE signed 16-bit mono PCM at the offline fixture
//! rate, the eSpeak output rate or the requested Google rate. They apply no conversion,
//! mastering or delivery policy. Admission proves only that the inspected bytes fit the
//! selected profile, have complete non-empty decoded frames and match the expected file
//! checksum. It does not approve speech fidelity, listening quality, loudness,
//! true peak or publication.

use serde::Serialize;

use crate::wav_inspection::{inspect_wav, StructuralError, INSPECTOR_VERSION};

/// Changing admission rules requires a new version; this is not a mastering/QC profile.
pub const OUTPUT_PROFILE_VERSION: &str = "cantos-offline-tts-pcm-1";
pub const ESPEAK_OUTPUT_PROFILE_VERSION: &str = "cantos-espeak-tts-pcm-1";
pub const GOOGLE_OUTPUT_PROFILE_VERSION: &str = "cantos-google-tts-pcm-1";
pub const NEURAL_OUTPUT_PROFILE_VERSION: &str = "cantos-vieneu-nano-tts-pcm-1";
/// Checked against actual bytes before hashing or walking any RIFF chunks.
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_OUTPUT_SECONDS: u64 = 120;
pub const OUTPUT_SAMPLE_RATE_HZ: u32 = 24_000;
pub const ESPEAK_OUTPUT_SAMPLE_RATE_HZ: u32 = 22_050;
pub const OUTPUT_CHANNELS: u16 = 1;
pub const OUTPUT_BITS_PER_SAMPLE: u16 = 16;

const WAVE_FORMAT_PCM: u16 = 1;

/// Explicit output contracts; selecting one never converts or silently falls back to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputProfile {
    Offline24Khz,
    Espeak22Khz,
    Google24Khz,
    Neural24Khz,
}

impl OutputProfile {
    pub const fn sample_rate_hz(self) -> u32 {
        match self {
            Self::Offline24Khz => OUTPUT_SAMPLE_RATE_HZ,
            Self::Espeak22Khz => ESPEAK_OUTPUT_SAMPLE_RATE_HZ,
            Self::Google24Khz => OUTPUT_SAMPLE_RATE_HZ,
            Self::Neural24Khz => OUTPUT_SAMPLE_RATE_HZ,
        }
    }

    pub const fn version(self) -> &'static str {
        match self {
            Self::Offline24Khz => OUTPUT_PROFILE_VERSION,
            Self::Espeak22Khz => ESPEAK_OUTPUT_PROFILE_VERSION,
            Self::Google24Khz => GOOGLE_OUTPUT_PROFILE_VERSION,
            Self::Neural24Khz => NEURAL_OUTPUT_PROFILE_VERSION,
        }
    }
}

/// Constructed only after inspecting the actual bytes. Deserialization cannot forge admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InspectedTtsAudio {
    file_sha256: String,
    frames: u64,
    sample_rate_hz: u32,
    channels: u16,
    bits_per_sample: u16,
    inspector_version: &'static str,
    output_profile_version: &'static str,
}

impl InspectedTtsAudio {
    pub fn file_sha256(&self) -> &str {
        &self.file_sha256
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn bits_per_sample(&self) -> u16 {
        self.bits_per_sample
    }

    pub fn inspector_version(&self) -> &str {
        self.inspector_version
    }

    pub fn output_profile_version(&self) -> &str {
        self.output_profile_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AudioError {
    OutputTooLarge {
        actual_bytes: usize,
        maximum_bytes: usize,
    },
    InvalidExpectedSha256,
    InvalidWav {
        errors: Vec<StructuralError>,
    },
    MissingInspectionFacts,
    UnsupportedProfile {
        format_tag: u16,
        sample_rate_hz: u32,
        channels: u16,
        bits_per_sample: u16,
    },
    EmptyFrames,
    DurationExceeded {
        frames: u64,
        maximum_frames: u64,
    },
    Sha256Mismatch {
        expected: String,
        actual: String,
    },
}

/// Inspect output against the original 24 kHz offline fixture profile.
pub fn inspect_output(
    bytes: &[u8],
    expected_sha256: &str,
) -> Result<InspectedTtsAudio, AudioError> {
    inspect_output_for_profile(bytes, expected_sha256, OutputProfile::Offline24Khz)
}

/// Inspect bounded output against an explicit profile and an exact lowercase SHA-256 receipt.
///
/// Size and digest syntax are checked before inspection. Structural defects are preserved from
/// the WAV inspector; profile, frame/duration and checksum errors follow in that order. Frame
/// counts stay integer facts so a rounded duration cannot admit an output above the limit.
pub fn inspect_output_for_profile(
    bytes: &[u8],
    expected_sha256: &str,
    profile: OutputProfile,
) -> Result<InspectedTtsAudio, AudioError> {
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(AudioError::OutputTooLarge {
            actual_bytes: bytes.len(),
            maximum_bytes: MAX_OUTPUT_BYTES,
        });
    }
    if !is_lowercase_sha256(expected_sha256) {
        return Err(AudioError::InvalidExpectedSha256);
    }

    let report = inspect_wav(bytes);
    if !report.errors().is_empty() {
        return Err(AudioError::InvalidWav {
            errors: report.errors().to_vec(),
        });
    }
    let (Some(format), Some(measurements)) = (report.fmt(), report.measurements()) else {
        // Preserve fail-closed behavior if the inspector's fact contract changes.
        return Err(AudioError::MissingInspectionFacts);
    };
    if format.format_tag != WAVE_FORMAT_PCM
        || format.sample_rate_hz != profile.sample_rate_hz()
        || format.channels != OUTPUT_CHANNELS
        || format.bits_per_sample != OUTPUT_BITS_PER_SAMPLE
    {
        return Err(AudioError::UnsupportedProfile {
            format_tag: format.format_tag,
            sample_rate_hz: format.sample_rate_hz,
            channels: format.channels,
            bits_per_sample: format.bits_per_sample,
        });
    }
    if measurements.frames == 0 {
        return Err(AudioError::EmptyFrames);
    }
    let maximum_frames = u64::from(profile.sample_rate_hz()) * MAX_OUTPUT_SECONDS;
    if measurements.frames > maximum_frames {
        return Err(AudioError::DurationExceeded {
            frames: measurements.frames,
            maximum_frames,
        });
    }
    if report.file().sha256 != expected_sha256 {
        return Err(AudioError::Sha256Mismatch {
            expected: expected_sha256.to_owned(),
            actual: report.file().sha256.clone(),
        });
    }

    Ok(InspectedTtsAudio {
        file_sha256: report.file().sha256.clone(),
        frames: measurements.frames,
        sample_rate_hz: format.sample_rate_hz,
        channels: format.channels,
        bits_per_sample: format.bits_per_sample,
        inspector_version: INSPECTOR_VERSION,
        output_profile_version: profile.version(),
    })
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
