//! Position-free speech fingerprint: the identity used for cross-run clip reuse.
//!
//! `jobs::input_fingerprint` (`durable-input-v1`) links one step to one frozen dialogue and
//! deliberately includes dialogue, character and rights identifiers. This module answers a
//! different question: "would a provider receive the same synthesis request?". It therefore
//! carries every field that can change the audio and nothing that only says where the line
//! sits or who owns it. Rights, tenant scope and artifact integrity are checked separately by
//! `clip_reuse`; they are never keyed, so a lapsed licence blocks instead of regenerating.

use std::{error::Error, fmt, str::FromStr};

use cantos_contracts::{ResolvedPronunciation, ResolvedSynthesisInput, SynthesisPerformance};
use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::contexts::production::catalog::local as local_contract;

pub const SPEECH_FINGERPRINT_SCHEME: &str = "speech-v1";
const DOMAIN_TAG: &[u8] = b"cantos/speech-fingerprint/v1\n";
const HEX_LEN: usize = 64;
/// Admission bounds an effective line at 64 KiB and the settings that feed its pronunciation list
/// at 256 KiB; this leaves headroom for escaping. The registry's `CHECK` uses the same number, so a
/// request that has a fingerprint can always be registered and acceptance can never fail on size.
pub const MAX_CANONICAL_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FingerprintError {
    /// The provider/model pair has no recorded pin policy, so equal strings today could mean
    /// different audio tomorrow. Cross-run reuse is undefined until a refresh epoch exists.
    ModelNotPinned,
    EmptyField(&'static str),
    DuplicatePronunciationSurface,
    /// The canonical document exceeds [`MAX_CANONICAL_BYTES`]. Such a request is never registered.
    InputTooLarge,
    InvalidFingerprint,
}

impl FingerprintError {
    /// Stable machine code shared with the contract vectors and the plan output.
    pub fn code(&self) -> String {
        match self {
            Self::ModelNotPinned => "model_not_pinned".into(),
            Self::EmptyField(name) => format!("empty_field:{name}"),
            Self::DuplicatePronunciationSurface => "duplicate_pronunciation_surface".into(),
            Self::InputTooLarge => "input_too_large".into(),
            Self::InvalidFingerprint => "invalid_fingerprint".into(),
        }
    }
}

impl fmt::Display for FingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelNotPinned => formatter
                .write_str("provider model identity is not pinned; cross-run reuse is undefined"),
            Self::EmptyField(name) => write!(formatter, "speech input field {name} is empty"),
            Self::DuplicatePronunciationSurface => {
                formatter.write_str("pronunciation surfaces must be unique")
            }
            Self::InputTooLarge => formatter.write_str("canonical speech input is too large"),
            Self::InvalidFingerprint => formatter.write_str("malformed speech fingerprint"),
        }
    }
}

impl Error for FingerprintError {}

/// `speech-v1:sha256:<64 lowercase hex>`. Only this module and its parser construct one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpeechFingerprint(String);

impl SpeechFingerprint {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for SpeechFingerprint {
    type Err = FingerprintError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let digest = value
            .strip_prefix(SPEECH_FINGERPRINT_SCHEME)
            .and_then(|rest| rest.strip_prefix(":sha256:"))
            .ok_or(FingerprintError::InvalidFingerprint)?;
        if digest.len() != HEX_LEN
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(FingerprintError::InvalidFingerprint);
        }
        Ok(Self(value.to_owned()))
    }
}

impl fmt::Display for SpeechFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for SpeechFingerprint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// The exact effective synthesis request. No public fields and no `Deserialize`: the only way
/// to obtain one is [`SpeechInput::from_resolved`], which classifies every resolved field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechInput {
    adapter_version: String,
    emotion: Option<String>,
    intensity_permille: Option<u16>,
    language: String,
    model_id: String,
    output_contract: String,
    pitch_semitones: i16,
    pronunciation: Vec<ResolvedPronunciation>,
    provider_id: String,
    rate_permille: u16,
    text: String,
    voice_id: String,
}

/// Only models whose identity is exact in the frozen input may be reused across runs. Google's
/// voice names carry no revision and VieNeu is research-only, so both are refused here.
fn model_is_pinned(provider_id: &str, model_id: &str) -> bool {
    provider_id == local_contract::PROVIDER_ID && model_id == local_contract::MODEL_ID
}

impl SpeechInput {
    /// Exhaustive destructuring without `..`: a new `ResolvedSynthesisInput` field does not
    /// compile until it is named here as effective (encoded) or identity-only (ignored).
    pub fn from_resolved(input: &ResolvedSynthesisInput) -> Result<Self, FingerprintError> {
        let ResolvedSynthesisInput {
            // Identity of where the line sits and who owns the voice record: not synthesis input.
            dialogue_id: _,
            character_id: _,
            voice_rights_record_id: _,
            text,
            language,
            provider_id,
            model_id,
            voice_id,
            adapter_version,
            output_contract,
            performance:
                SynthesisPerformance {
                    rate_permille,
                    pitch_semitones,
                    emotion,
                    intensity_permille,
                },
            pronunciation,
        } = input;
        for (name, value) in [
            ("text", text),
            ("language", language),
            ("provider_id", provider_id),
            ("model_id", model_id),
            ("voice_id", voice_id),
            ("adapter_version", adapter_version),
            ("output_contract", output_contract),
        ] {
            if value.is_empty() {
                return Err(FingerprintError::EmptyField(name));
            }
        }
        if !model_is_pinned(provider_id, model_id) {
            return Err(FingerprintError::ModelNotPinned);
        }
        // Each entry is destructured too: a new `ResolvedPronunciation` field must be classified.
        let mut pronunciation: Vec<ResolvedPronunciation> = pronunciation
            .iter()
            .map(
                |ResolvedPronunciation {
                     surface,
                     replacement,
                 }| ResolvedPronunciation {
                    surface: surface.clone(),
                    replacement: replacement.clone(),
                },
            )
            .collect();
        pronunciation.sort_by(|left, right| left.surface.cmp(&right.surface));
        if pronunciation
            .windows(2)
            .any(|pair| pair[0].surface == pair[1].surface)
        {
            return Err(FingerprintError::DuplicatePronunciationSurface);
        }
        let input = Self {
            adapter_version: adapter_version.clone(),
            emotion: emotion.clone(),
            intensity_permille: *intensity_permille,
            language: language.clone(),
            model_id: model_id.clone(),
            output_contract: output_contract.clone(),
            pitch_semitones: *pitch_semitones,
            pronunciation,
            provider_id: provider_id.clone(),
            rate_permille: *rate_permille,
            text: text.clone(),
            voice_id: voice_id.clone(),
        };
        if input.canonical_bytes().len() > MAX_CANONICAL_BYTES {
            return Err(FingerprintError::InputTooLarge);
        }
        Ok(input)
    }

    /// Compact UTF-8 JSON, ASCII keys in sorted order, minimal escaping, integers only. Equal
    /// requests produce equal bytes whatever order the pronunciation list arrived in. The
    /// registry stores these bytes beside the digest so an audit can name the changed field.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        out.push('{');
        string_field(&mut out, "adapter_version", &self.adapter_version, false);
        string_field(&mut out, "language", &self.language, true);
        string_field(&mut out, "model_id", &self.model_id, true);
        string_field(&mut out, "output_contract", &self.output_contract, true);
        out.push_str(",\"performance\":{\"emotion\":");
        match &self.emotion {
            Some(emotion) => push_string(&mut out, emotion),
            None => out.push_str("null"),
        }
        out.push_str(",\"intensity_permille\":");
        match self.intensity_permille {
            Some(value) => out.push_str(&value.to_string()),
            None => out.push_str("null"),
        }
        out.push_str(&format!(
            ",\"pitch_semitones\":{},\"rate_permille\":{}}}",
            self.pitch_semitones, self.rate_permille
        ));
        out.push_str(",\"pronunciation\":[");
        for (index, entry) in self.pronunciation.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str("{\"replacement\":");
            push_string(&mut out, &entry.replacement);
            out.push_str(",\"surface\":");
            push_string(&mut out, &entry.surface);
            out.push('}');
        }
        out.push(']');
        string_field(&mut out, "provider_id", &self.provider_id, true);
        string_field(&mut out, "text", &self.text, true);
        string_field(&mut out, "voice_id", &self.voice_id, true);
        out.push('}');
        out.into_bytes()
    }

    pub fn fingerprint(&self) -> SpeechFingerprint {
        let mut hash = Sha256::new();
        hash.update(DOMAIN_TAG);
        hash.update(self.canonical_bytes());
        SpeechFingerprint(format!(
            "{SPEECH_FINGERPRINT_SCHEME}:sha256:{:x}",
            hash.finalize()
        ))
    }
}

/// The effective fields of a request, named so a plan can say why a line regenerates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechField {
    Text,
    Language,
    Provider,
    Model,
    Voice,
    Adapter,
    OutputContract,
    Rate,
    Pitch,
    Emotion,
    Intensity,
    Pronunciation,
}

impl SpeechInput {
    /// Fields whose values differ, in declaration order. Empty exactly when the fingerprints
    /// are equal. Exhaustive destructuring keeps this in step with the encoder.
    pub fn differing_fields(&self, other: &Self) -> Vec<SpeechField> {
        let Self {
            adapter_version,
            emotion,
            intensity_permille,
            language,
            model_id,
            output_contract,
            pitch_semitones,
            pronunciation,
            provider_id,
            rate_permille,
            text,
            voice_id,
        } = self;
        [
            (SpeechField::Text, text != &other.text),
            (SpeechField::Language, language != &other.language),
            (SpeechField::Provider, provider_id != &other.provider_id),
            (SpeechField::Model, model_id != &other.model_id),
            (SpeechField::Voice, voice_id != &other.voice_id),
            (
                SpeechField::Adapter,
                adapter_version != &other.adapter_version,
            ),
            (
                SpeechField::OutputContract,
                output_contract != &other.output_contract,
            ),
            (SpeechField::Rate, rate_permille != &other.rate_permille),
            (
                SpeechField::Pitch,
                pitch_semitones != &other.pitch_semitones,
            ),
            (SpeechField::Emotion, emotion != &other.emotion),
            (
                SpeechField::Intensity,
                intensity_permille != &other.intensity_permille,
            ),
            (
                SpeechField::Pronunciation,
                pronunciation != &other.pronunciation,
            ),
        ]
        .into_iter()
        .filter_map(|(field, differs)| differs.then_some(field))
        .collect()
    }
}

fn string_field(out: &mut String, name: &str, value: &str, comma: bool) {
    if comma {
        out.push(',');
    }
    out.push('"');
    out.push_str(name);
    out.push_str("\":");
    push_string(out, value);
}

fn push_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            control if control < ' ' => out.push_str(&format!("\\u{:04x}", control as u32)),
            other => out.push(other),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use cantos_contracts::SynthesisPerformance;

    use super::*;

    type Mutate = fn(&mut ResolvedSynthesisInput);

    fn input() -> ResolvedSynthesisInput {
        ResolvedSynthesisInput {
            dialogue_id: "dialogue-02".into(),
            character_id: "an".into(),
            text: "Ngày mai, mình có diễn tiếp không?".into(),
            language: "vi-VN".into(),
            provider_id: local_contract::PROVIDER_ID.into(),
            model_id: local_contract::MODEL_ID.into(),
            voice_id: "vi-vn-x-south".into(),
            voice_rights_record_id: "voice-espeak".into(),
            adapter_version: local_contract::ADAPTER_VERSION.into(),
            output_contract: local_contract::OUTPUT_CONTRACT.into(),
            performance: SynthesisPerformance {
                rate_permille: 1_000,
                pitch_semitones: 0,
                emotion: Some("neutral".into()),
                intensity_permille: Some(0),
            },
            pronunciation: vec![
                ResolvedPronunciation {
                    surface: "Vọng Đài".into(),
                    replacement: "Vọng Đài".into(),
                },
                ResolvedPronunciation {
                    surface: "An".into(),
                    replacement: "Ân".into(),
                },
            ],
        }
    }

    fn fingerprint(input: &ResolvedSynthesisInput) -> SpeechFingerprint {
        SpeechInput::from_resolved(input).unwrap().fingerprint()
    }

    #[test]
    fn canonical_document_is_exact_sorted_and_orders_pronunciation_by_surface() {
        let document = String::from_utf8(
            SpeechInput::from_resolved(&input())
                .unwrap()
                .canonical_bytes(),
        )
        .unwrap();
        assert_eq!(
            document,
            concat!(
                r#"{"adapter_version":"espeak-ng-local-v1","language":"vi-VN","#,
                r#""model_id":"espeak-ng-1.52.0","output_contract":"wav-pcm-s16le-mono-22050-v1","#,
                r#""performance":{"emotion":"neutral","intensity_permille":0,"#,
                r#""pitch_semitones":0,"rate_permille":1000},"#,
                r#""pronunciation":[{"replacement":"Ân","surface":"An"},"#,
                r#"{"replacement":"Vọng Đài","surface":"Vọng Đài"}],"#,
                r#""provider_id":"espeak-ng-local","text":"Ngày mai, mình có diễn tiếp không?","#,
                r#""voice_id":"vi-vn-x-south"}"#,
            )
        );
    }

    #[test]
    fn identity_only_fields_and_pronunciation_order_do_not_change_the_fingerprint() {
        let base = input();
        let expected = fingerprint(&base);
        let mut moved = base.clone();
        moved.dialogue_id = "dialogue-99".into();
        moved.character_id = "someone-else".into();
        moved.voice_rights_record_id = "another-record".into();
        moved.pronunciation.reverse();
        assert_eq!(fingerprint(&moved), expected);
    }

    #[test]
    fn every_effective_field_changes_the_fingerprint() {
        let base = input();
        let expected = fingerprint(&base);
        let mutations: [(&str, Mutate); 12] = [
            ("text", |i| i.text.push('.')),
            ("language", |i| i.language = "vi".into()),
            ("voice_id", |i| i.voice_id = "vi".into()),
            ("adapter_version", |i| i.adapter_version.push('2')),
            ("output_contract", |i| i.output_contract.push('2')),
            ("rate", |i| i.performance.rate_permille = 999),
            ("pitch", |i| i.performance.pitch_semitones = 1),
            ("emotion", |i| i.performance.emotion = None),
            ("intensity", |i| i.performance.intensity_permille = None),
            ("pronunciation_replacement", |i| {
                i.pronunciation[0].replacement.push('x')
            }),
            ("pronunciation_surface", |i| {
                i.pronunciation[0].surface.push('x')
            }),
            ("pronunciation_removed", |i| {
                i.pronunciation.pop();
            }),
        ];
        for (field, mutate) in mutations {
            let mut changed = base.clone();
            mutate(&mut changed);
            assert_ne!(
                fingerprint(&changed),
                expected,
                "{field} is not fingerprinted"
            );
        }
    }

    #[test]
    fn differing_fields_names_exactly_the_changed_effective_fields() {
        let base = SpeechInput::from_resolved(&input()).unwrap();
        assert_eq!(base.differing_fields(&base), Vec::<SpeechField>::new());
        let cases: [(SpeechField, Mutate); 10] = [
            (SpeechField::Text, |i| i.text.push('.')),
            (SpeechField::Language, |i| i.language = "vi".into()),
            (SpeechField::Voice, |i| i.voice_id = "vi".into()),
            (SpeechField::Adapter, |i| i.adapter_version.push('2')),
            (SpeechField::OutputContract, |i| i.output_contract.push('2')),
            (SpeechField::Rate, |i| i.performance.rate_permille = 999),
            (SpeechField::Pitch, |i| i.performance.pitch_semitones = 1),
            (SpeechField::Emotion, |i| i.performance.emotion = None),
            (SpeechField::Intensity, |i| {
                i.performance.intensity_permille = None
            }),
            (SpeechField::Pronunciation, |i| {
                i.pronunciation.pop().map(drop).unwrap_or(())
            }),
        ];
        for (field, mutate) in cases {
            let mut changed = input();
            mutate(&mut changed);
            let changed = SpeechInput::from_resolved(&changed).unwrap();
            assert_eq!(base.differing_fields(&changed), vec![field]);
            assert_eq!(changed.differing_fields(&base), vec![field]);
        }
    }

    #[test]
    fn a_canonical_document_beyond_the_registry_bound_has_no_fingerprint() {
        let mut huge = input();
        huge.pronunciation = vec![ResolvedPronunciation {
            surface: "An".into(),
            replacement: "a".repeat(MAX_CANONICAL_BYTES),
        }];
        assert_eq!(
            SpeechInput::from_resolved(&huge),
            Err(FingerprintError::InputTooLarge)
        );
        // Just under the bound is still a valid request.
        let mut large = input();
        large.pronunciation = vec![ResolvedPronunciation {
            surface: "An".into(),
            replacement: "a".repeat(MAX_CANONICAL_BYTES - 1024),
        }];
        assert!(SpeechInput::from_resolved(&large).is_ok());
    }

    #[test]
    fn unpinned_or_unknown_models_have_no_fingerprint() {
        for (provider, model) in [
            ("google-cloud-tts", "vi-VN-Chirp3-HD-Aoede"),
            (
                "vieneu-nano-local",
                "vieneu-nano-aba295eb96a6fa6003ebe417cc1f2802a7adc1dc",
            ),
            (local_contract::PROVIDER_ID, "espeak-ng-latest"),
            ("reference-only", local_contract::MODEL_ID),
        ] {
            let mut changed = input();
            changed.provider_id = provider.into();
            changed.model_id = model.into();
            assert_eq!(
                SpeechInput::from_resolved(&changed),
                Err(FingerprintError::ModelNotPinned)
            );
        }
    }

    #[test]
    fn empty_fields_and_duplicate_surfaces_are_rejected_with_exact_errors() {
        let cases: [(&str, Mutate); 3] = [
            ("text", |i| i.text.clear()),
            ("voice_id", |i| i.voice_id.clear()),
            ("output_contract", |i| i.output_contract.clear()),
        ];
        for (field, mutate) in cases {
            let mut changed = input();
            mutate(&mut changed);
            assert_eq!(
                SpeechInput::from_resolved(&changed),
                Err(FingerprintError::EmptyField(field))
            );
        }
        let mut duplicate = input();
        duplicate.pronunciation[1].surface = duplicate.pronunciation[0].surface.clone();
        assert_eq!(
            SpeechInput::from_resolved(&duplicate),
            Err(FingerprintError::DuplicatePronunciationSurface)
        );
    }

    #[test]
    fn identifier_parses_only_the_exact_scheme_and_is_distinct_from_the_durable_identity() {
        let fingerprint = fingerprint(&input());
        assert_eq!(fingerprint.as_str().parse(), Ok(fingerprint.clone()));
        let durable = crate::contexts::synthesis::jobs::input_fingerprint(&input()).unwrap();
        assert_ne!(fingerprint.as_str(), durable);
        assert!(durable.parse::<SpeechFingerprint>().is_err());
        let digest = fingerprint.as_str().trim_start_matches("speech-v1:sha256:");
        for bad in [
            String::new(),
            format!("speech-v2:sha256:{digest}"),
            format!("speech-v1:sha512:{digest}"),
            format!("speech-v1:sha256:{}", digest.to_uppercase()),
            format!("speech-v1:sha256:{}", &digest[1..]),
            format!("speech-v1:sha256:{digest}0"),
            format!(" speech-v1:sha256:{digest}"),
        ] {
            assert_eq!(
                bad.parse::<SpeechFingerprint>(),
                Err(FingerprintError::InvalidFingerprint),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn strings_escape_quotes_backslashes_and_controls_like_a_standard_json_encoder() {
        let mut out = String::new();
        push_string(&mut out, "a\"b\\c\n\r\t\u{8}\u{c}\u{1}\u{1f}\u{7f}é");
        assert_eq!(out, "\"a\\\"b\\\\c\\n\\r\\t\\b\\f\\u0001\\u001f\u{7f}é\"");
    }
}
