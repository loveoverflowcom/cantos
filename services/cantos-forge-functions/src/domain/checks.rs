//! Deterministic checks of exact bytes: a result that does not depend on judgement.
//!
//! A passing check is a producer-side technical measurement. It is not listening, not a rights
//! verification and not server admission. Each check names the rule set it applied, so a later
//! change of rules is a new check, never a silent reinterpretation of old evidence.

use std::collections::BTreeMap;

use super::{
    audio_profile::{AudioProfile, MAX_BODY_BYTES, MAX_DURATION_SECONDS},
    evaluation::CheckOutcome,
    ogg_opus::{self, OggOpusFacts},
};

const SAMPLE_RATE_HZ: u64 = 48_000;

/// A deterministic check Forge can run. The version is part of the check's identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Ogg Opus framing and the named intake profile's envelope, v1 rules.
    OggOpusProfile(AudioProfile),
    /// The bytes are non-blank UTF-8 text without stray control characters, v1 rules.
    Utf8Text,
}

impl Check {
    /// The text recorded as the check's name in evidence.
    pub fn label(self) -> String {
        match self {
            Self::OggOpusProfile(profile) => format!("ogg-opus-profile-v1:{profile}"),
            Self::Utf8Text => "utf8-text-v1".to_owned(),
        }
    }
}

/// One measured value. Numbers stay numbers so a consumer never parses prose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DetailValue {
    Text(String),
    Count(u64),
    Codes(Vec<String>),
}

/// The measurements and reasons behind a check outcome, in stable key order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Detail(BTreeMap<String, DetailValue>);

impl Detail {
    pub fn new(entries: impl IntoIterator<Item = (String, DetailValue)>) -> Self {
        Self(entries.into_iter().collect())
    }

    fn put(&mut self, key: &str, value: DetailValue) {
        self.0.insert(key.to_owned(), value);
    }

    fn count(&mut self, key: &str, value: u64) {
        self.put(key, DetailValue::Count(value));
    }

    pub fn get(&self, key: &str) -> Option<&DetailValue> {
        self.0.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &DetailValue)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckResult {
    pub check: Check,
    pub outcome: CheckOutcome,
    pub detail: Detail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileViolation {
    NotMono { channels: u8 },
    BodyTooLarge { byte_length: u64 },
    DurationTooLong { duration_samples: u64 },
    BitrateAboveLimit { observed_bps: u64, maximum_bps: u64 },
}

impl ProfileViolation {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotMono { .. } => "not_mono",
            Self::BodyTooLarge { .. } => "body_too_large",
            Self::DurationTooLong { .. } => "duration_too_long",
            Self::BitrateAboveLimit { .. } => "bitrate_above_limit",
        }
    }
}

/// Every way measured facts fall outside `profile`'s envelope; empty means inside it.
pub fn profile_violations(facts: &OggOpusFacts, profile: AudioProfile) -> Vec<ProfileViolation> {
    let mut violations = Vec::new();
    if facts.channels != 1 {
        violations.push(ProfileViolation::NotMono {
            channels: facts.channels,
        });
    }
    if facts.byte_length > MAX_BODY_BYTES {
        violations.push(ProfileViolation::BodyTooLarge {
            byte_length: facts.byte_length,
        });
    }
    if facts.duration_samples > MAX_DURATION_SECONDS * SAMPLE_RATE_HZ {
        violations.push(ProfileViolation::DurationTooLong {
            duration_samples: facts.duration_samples,
        });
    }
    let observed_bps = facts.average_bitrate_bps();
    if observed_bps > profile.maximum_bitrate_bps() {
        violations.push(ProfileViolation::BitrateAboveLimit {
            observed_bps,
            maximum_bps: profile.maximum_bitrate_bps(),
        });
    }
    violations
}

/// Measures Ogg Opus bytes against one intake profile.
pub fn check_audio(bytes: &[u8], profile: AudioProfile) -> CheckResult {
    let check = Check::OggOpusProfile(profile);
    let mut detail = Detail::default();
    detail.put("profile", DetailValue::Text(profile.id().to_owned()));
    detail.count("byte_length", bytes.len() as u64);
    detail.count("maximum_bitrate_bps", profile.maximum_bitrate_bps());
    let facts = match ogg_opus::inspect(bytes) {
        Ok(facts) => facts,
        Err(error) => {
            detail.put("refusal", DetailValue::Text(error.code().to_owned()));
            return CheckResult {
                check,
                outcome: CheckOutcome::Failed,
                detail,
            };
        }
    };
    detail.put("container", DetailValue::Text("ogg".to_owned()));
    detail.put("codec", DetailValue::Text("opus".to_owned()));
    detail.count("channels", u64::from(facts.channels));
    detail.count(
        "input_sample_rate_hz",
        u64::from(facts.input_sample_rate_hz),
    );
    detail.count("pre_skip", u64::from(facts.pre_skip));
    detail.count("duration_samples", facts.duration_samples);
    detail.count("duration_ms", facts.duration_ms());
    detail.count("coded_payload_bytes", facts.coded_payload_bytes);
    detail.count("audio_bitrate_bps", facts.average_bitrate_bps());
    let violations: Vec<String> = profile_violations(&facts, profile)
        .into_iter()
        .map(|violation| violation.code().to_owned())
        .collect();
    let outcome = if violations.is_empty() {
        CheckOutcome::Passed
    } else {
        CheckOutcome::Failed
    };
    detail.put("violations", DetailValue::Codes(violations));
    CheckResult {
        check,
        outcome,
        detail,
    }
}

/// Measures text bytes: valid UTF-8, not blank, no byte-order mark, no control characters other
/// than line breaks and tabs. It does not judge language, quality or originality.
pub fn check_text(bytes: &[u8]) -> CheckResult {
    let mut detail = Detail::default();
    detail.count("byte_length", bytes.len() as u64);
    let mut violations = Vec::new();
    match std::str::from_utf8(bytes) {
        Err(_) => violations.push("invalid_utf8".to_owned()),
        Ok(text) => {
            detail.count("char_count", text.chars().count() as u64);
            if text.trim().is_empty() {
                violations.push("blank".to_owned());
            }
            if text.starts_with('\u{feff}') {
                violations.push("byte_order_mark".to_owned());
            }
            let stray_control = text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'));
            if stray_control {
                violations.push("control_character".to_owned());
            }
        }
    }
    let outcome = if violations.is_empty() {
        CheckOutcome::Passed
    } else {
        CheckOutcome::Failed
    };
    detail.put("violations", DetailValue::Codes(violations));
    CheckResult {
        check: Check::Utf8Text,
        outcome,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ogg_opus::testing::Fixture;

    fn codes(result: &CheckResult) -> Vec<String> {
        match result.detail.get("violations") {
            Some(DetailValue::Codes(codes)) => codes.clone(),
            other => panic!("expected violation codes, found {other:?}"),
        }
    }

    /// 100 packets of 20 ms: exactly 2 s of coded audio before trimming and pre-skip.
    fn two_seconds(packet_bytes: usize) -> Vec<u8> {
        let mut fixture = Fixture::mono(vec![packet_bytes; 100]);
        fixture.pre_skip = 0;
        fixture.build()
    }

    #[test]
    fn labels_name_the_rule_set_and_profile() {
        assert_eq!(
            Check::OggOpusProfile(AudioProfile::OpusMono24V1).label(),
            "ogg-opus-profile-v1:opus-mono-24-v1"
        );
        assert_eq!(Check::Utf8Text.label(), "utf8-text-v1");
    }

    #[test]
    fn audio_inside_the_envelope_passes_with_exact_measurements() {
        // 100 packets x 60 bytes = 6_000 bytes over 2 s = 24_000 bits/s: exactly the ceiling.
        let result = check_audio(&two_seconds(60), AudioProfile::OpusMono20V1);
        assert_eq!(result.outcome, CheckOutcome::Passed, "{result:?}");
        assert_eq!(
            result.detail.get("audio_bitrate_bps"),
            Some(&DetailValue::Count(24_000))
        );
        assert_eq!(
            result.detail.get("duration_ms"),
            Some(&DetailValue::Count(2_000))
        );
        assert_eq!(result.detail.get("channels"), Some(&DetailValue::Count(1)));
        assert_eq!(
            result.detail.get("coded_payload_bytes"),
            Some(&DetailValue::Count(6_000))
        );
        assert!(codes(&result).is_empty());
    }

    #[test]
    fn one_coded_byte_over_the_ceiling_fails_with_the_bitrate_code() {
        let mut sizes = vec![60_usize; 100];
        sizes[0] = 61; // 6_001 bytes over 2 s = 24_004 bits/s
        let mut fixture = Fixture::mono(sizes);
        fixture.pre_skip = 0;
        let result = check_audio(&fixture.build(), AudioProfile::OpusMono20V1);
        assert_eq!(result.outcome, CheckOutcome::Failed);
        assert_eq!(codes(&result), ["bitrate_above_limit"]);
        assert_eq!(
            result.detail.get("audio_bitrate_bps"),
            Some(&DetailValue::Count(24_004))
        );
        // The same bytes are inside the next profile's envelope.
        let wider = check_audio(&fixture.build(), AudioProfile::OpusMono24V1);
        assert_eq!(wider.outcome, CheckOutcome::Passed);
    }

    #[test]
    fn stereo_fails_with_not_mono_even_when_the_bitrate_is_fine() {
        let mut fixture = Fixture::mono(vec![40; 100]);
        fixture.channels = 2;
        let result = check_audio(&fixture.build(), AudioProfile::OpusMono40V1);
        assert_eq!(result.outcome, CheckOutcome::Failed);
        assert_eq!(codes(&result), ["not_mono"]);
    }

    #[test]
    fn violations_accumulate_instead_of_stopping_at_the_first() {
        let facts = OggOpusFacts {
            byte_length: MAX_BODY_BYTES + 1,
            channels: 2,
            input_sample_rate_hz: 48_000,
            pre_skip: 0,
            duration_samples: MAX_DURATION_SECONDS * SAMPLE_RATE_HZ + 1,
            coded_payload_bytes: 100_000_000,
        };
        let violations = profile_violations(&facts, AudioProfile::OpusMono20V1);
        let found: Vec<&str> = violations
            .iter()
            .map(|violation| violation.code())
            .collect();
        assert_eq!(
            found,
            [
                "not_mono",
                "body_too_large",
                "duration_too_long",
                "bitrate_above_limit"
            ]
        );
    }

    #[test]
    fn the_body_limit_is_inclusive() {
        let facts = |byte_length| OggOpusFacts {
            byte_length,
            channels: 1,
            input_sample_rate_hz: 48_000,
            pre_skip: 0,
            duration_samples: SAMPLE_RATE_HZ,
            coded_payload_bytes: 1,
        };
        assert!(profile_violations(&facts(MAX_BODY_BYTES), AudioProfile::OpusMono20V1).is_empty());
        assert_eq!(
            profile_violations(&facts(MAX_BODY_BYTES + 1), AudioProfile::OpusMono20V1),
            [ProfileViolation::BodyTooLarge {
                byte_length: MAX_BODY_BYTES + 1
            }]
        );
    }

    #[test]
    fn the_duration_limit_is_inclusive() {
        let at_limit = OggOpusFacts {
            byte_length: 1,
            channels: 1,
            input_sample_rate_hz: 48_000,
            pre_skip: 0,
            duration_samples: MAX_DURATION_SECONDS * SAMPLE_RATE_HZ,
            coded_payload_bytes: 1,
        };
        assert!(profile_violations(&at_limit, AudioProfile::OpusMono20V1).is_empty());
    }

    #[test]
    fn unparseable_audio_fails_with_the_refusal_code_and_no_measurements() {
        let result = check_audio(b"RIFF....WAVEfmt ", AudioProfile::OpusMono24V1);
        assert_eq!(result.outcome, CheckOutcome::Failed);
        assert_eq!(
            result.detail.get("refusal"),
            Some(&DetailValue::Text("not_ogg".to_owned()))
        );
        assert_eq!(result.detail.get("duration_ms"), None);
        assert_eq!(result.detail.get("violations"), None);
    }

    #[test]
    fn text_passes_vietnamese_in_composed_and_decomposed_forms() {
        for text in ["Lời hẹn cuối mùa mưa.\n", "Lo\u{0300}i he\u{0323}n\r\n"] {
            let result = check_text(text.as_bytes());
            assert_eq!(result.outcome, CheckOutcome::Passed, "{text:?}");
            assert_eq!(
                result.detail.get("char_count"),
                Some(&DetailValue::Count(text.chars().count() as u64))
            );
        }
    }

    #[test]
    fn text_failures_are_exact() {
        let cases: [(&[u8], &[&str]); 5] = [
            (b"", &["blank"]),
            (b" \n\t ", &["blank"]),
            (b"\xff\xfe broken", &["invalid_utf8"]),
            ("\u{feff}Mở đầu".as_bytes(), &["byte_order_mark"]),
            (b"ok\0text", &["control_character"]),
        ];
        for (bytes, expected) in cases {
            let result = check_text(bytes);
            assert_eq!(result.outcome, CheckOutcome::Failed, "{bytes:?}");
            assert_eq!(codes(&result), expected, "{bytes:?}");
        }
    }
}
