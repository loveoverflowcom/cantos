//! Immutable, explicitly selected admission envelopes, not playback/listening certification.
//! Nominal encoder targets are producer guidance; VBR is checked by observed payload average.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryProfile {
    OpusMono20V1,
    OpusMono24V1,
    OpusMono32V1,
    OpusMono40V1,
}

impl DeliveryProfile {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "opus-mono-20-v1" => Some(Self::OpusMono20V1),
            "opus-mono-24-v1" => Some(Self::OpusMono24V1),
            "opus-mono-32-v1" => Some(Self::OpusMono32V1),
            "opus-mono-40-v1" => Some(Self::OpusMono40V1),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::OpusMono20V1 => "opus-mono-20-v1",
            Self::OpusMono24V1 => "opus-mono-24-v1",
            Self::OpusMono32V1 => "opus-mono-32-v1",
            Self::OpusMono40V1 => "opus-mono-40-v1",
        }
    }

    pub fn container(self) -> &'static str {
        "ogg"
    }

    pub fn codec(self) -> &'static str {
        "opus"
    }

    pub fn target_bitrate_bps(self) -> u64 {
        match self {
            Self::OpusMono20V1 => 20_000,
            Self::OpusMono24V1 => 24_000,
            Self::OpusMono32V1 => 32_000,
            Self::OpusMono40V1 => 40_000,
        }
    }

    /// Versioned v1 ceiling: 20% over nominal producer target, measured on coded packets.
    /// There is no lower floor because silence and VBR legitimately reduce average bitrate.
    pub fn maximum_bitrate_bps(self) -> u64 {
        self.target_bitrate_bps() * 6 / 5
    }
}

#[cfg(test)]
mod tests {
    use super::DeliveryProfile;

    #[test]
    fn profiles_are_closed_versioned_and_have_no_default() {
        for (id, target) in [
            ("opus-mono-20-v1", 20_000),
            ("opus-mono-24-v1", 24_000),
            ("opus-mono-32-v1", 32_000),
            ("opus-mono-40-v1", 40_000),
        ] {
            let profile = DeliveryProfile::from_id(id).unwrap();
            assert_eq!(profile.id(), id);
            assert_eq!(profile.target_bitrate_bps(), target);
            assert_eq!(profile.maximum_bitrate_bps(), target * 6 / 5);
        }
        for id in ["", "default", "opus-mono-24", "opus-mono-24-v2", "../opus"] {
            assert_eq!(DeliveryProfile::from_id(id), None);
        }
    }
}
