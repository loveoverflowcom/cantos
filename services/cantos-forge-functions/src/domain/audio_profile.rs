//! The audio intake profiles the Cantos API accepts, as pure values.
//!
//! Forge never invents or defaults a profile: a producer names one and Forge only measures
//! against it. The numbers mirror `packages/contracts/studio-audio-intake-v1.md`; a test reads
//! that table so the two cannot drift silently. The server inspects the received bytes again
//! and stays the authority: passing here is a producer-side measurement, not admission.

use std::fmt;

/// Longest accepted duration on the Opus decoding timeline.
pub const MAX_DURATION_SECONDS: u64 = 3_600;
/// Largest accepted complete body (32 MiB).
pub const MAX_BODY_BYTES: u64 = 33_554_432;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioProfile {
    OpusMono20V1,
    OpusMono24V1,
    OpusMono32V1,
    OpusMono40V1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownProfile;

impl AudioProfile {
    pub const ALL: [Self; 4] = [
        Self::OpusMono20V1,
        Self::OpusMono24V1,
        Self::OpusMono32V1,
        Self::OpusMono40V1,
    ];

    pub fn parse(text: &str) -> Result<Self, UnknownProfile> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.id() == text)
            .ok_or(UnknownProfile)
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::OpusMono20V1 => "opus-mono-20-v1",
            Self::OpusMono24V1 => "opus-mono-24-v1",
            Self::OpusMono32V1 => "opus-mono-32-v1",
            Self::OpusMono40V1 => "opus-mono-40-v1",
        }
    }

    /// Encoding guidance for the producer. It is not a minimum: VBR and silence go lower.
    pub fn nominal_bitrate_bps(self) -> u64 {
        match self {
            Self::OpusMono20V1 => 20_000,
            Self::OpusMono24V1 => 24_000,
            Self::OpusMono32V1 => 32_000,
            Self::OpusMono40V1 => 40_000,
        }
    }

    /// The engineering ceiling for the observed average coded audio bitrate: 120% of nominal.
    pub fn maximum_bitrate_bps(self) -> u64 {
        self.nominal_bitrate_bps() / 5 * 6
    }
}

impl fmt::Display for AudioProfile {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_unknown_ids_are_refused_exactly() {
        for profile in AudioProfile::ALL {
            assert_eq!(AudioProfile::parse(profile.id()), Ok(profile));
        }
        for text in [
            "",
            "opus-mono-48-v1",
            "opus-mono-24-v2",
            "OPUS-MONO-24-V1",
            "aac-mono-32",
        ] {
            assert_eq!(AudioProfile::parse(text), Err(UnknownProfile), "{text}");
        }
    }

    #[test]
    fn the_ceiling_is_exactly_120_percent_of_nominal() {
        let ceilings = AudioProfile::ALL.map(AudioProfile::maximum_bitrate_bps);
        assert_eq!(ceilings, [24_000, 28_800, 38_400, 48_000]);
    }
}
