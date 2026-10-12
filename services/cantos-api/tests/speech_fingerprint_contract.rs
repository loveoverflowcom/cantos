//! Speech fingerprint evidence that does not depend on the implementation's own encoder:
//! vectors written by the independent Python oracle, and laws over generated requests.
use std::collections::BTreeMap;

use cantos_api::speech_fingerprint::{SpeechFingerprint, SpeechInput};
use cantos_contracts::{ResolvedPronunciation, ResolvedSynthesisInput, SynthesisPerformance};
use proptest::prelude::*;
use serde::Deserialize;
use serde_json::Value;

const VECTORS: &str =
    include_str!("../../../packages/contracts/fixtures/clip-reuse/v1/speech-vectors.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorFile {
    scheme: String,
    domain_tag: String,
    vectors: Vec<Vector>,
    rejected: Vec<Rejected>,
    relations: Vec<Relation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    id: String,
    #[allow(dead_code)]
    note: String,
    input: ResolvedSynthesisInput,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    canonical_json: String,
    fingerprint: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rejected {
    id: String,
    #[allow(dead_code)]
    note: String,
    input: ResolvedSynthesisInput,
    error: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Relation {
    left: String,
    right: String,
    same: bool,
}

fn file() -> VectorFile {
    serde_json::from_str(VECTORS).expect("committed vectors parse")
}

fn input_of(file: &VectorFile, id: &str) -> ResolvedSynthesisInput {
    file.vectors
        .iter()
        .find(|vector| vector.id == id)
        .unwrap_or_else(|| panic!("vector {id} is committed"))
        .input
        .clone()
}

#[test]
fn rust_encoder_reproduces_every_oracle_document_and_digest() {
    let file = file();
    assert_eq!(file.scheme, "speech-v1");
    assert_eq!(file.domain_tag, "cantos/speech-fingerprint/v1\n");
    assert!(file.vectors.len() >= 16, "the oracle corpus did not shrink");
    for vector in &file.vectors {
        let input = SpeechInput::from_resolved(&vector.input)
            .unwrap_or_else(|error| panic!("{}: {error}", vector.id));
        assert_eq!(
            String::from_utf8(input.canonical_bytes()).unwrap(),
            vector.expected.canonical_json,
            "{} canonical document",
            vector.id
        );
        assert_eq!(
            input.fingerprint().as_str(),
            vector.expected.fingerprint,
            "{} digest",
            vector.id
        );
        assert_eq!(
            vector
                .expected
                .fingerprint
                .parse::<SpeechFingerprint>()
                .unwrap(),
            input.fingerprint()
        );
    }
}

#[test]
fn rust_refuses_exactly_the_inputs_the_oracle_refuses_with_the_same_codes() {
    let file = file();
    assert!(file.rejected.len() >= 5);
    for rejected in &file.rejected {
        let error = SpeechInput::from_resolved(&rejected.input)
            .expect_err(&format!("{} must not have a fingerprint", rejected.id));
        assert_eq!(error.code(), rejected.error, "{}", rejected.id);
    }
}

#[test]
fn rust_and_oracle_agree_on_which_changes_keep_the_fingerprint() {
    let file = file();
    let base = SpeechInput::from_resolved(&input_of(&file, "base"))
        .unwrap()
        .fingerprint();
    let mut seen_same = 0;
    let mut seen_different = 0;
    for relation in &file.relations {
        assert_eq!(relation.left, "base");
        let other = SpeechInput::from_resolved(&input_of(&file, &relation.right))
            .unwrap()
            .fingerprint();
        assert_eq!(
            other == base,
            relation.same,
            "{} versus base",
            relation.right
        );
        if relation.same {
            seen_same += 1;
        } else {
            seen_different += 1;
        }
    }
    assert_eq!((seen_same, seen_different), (2, 13));
}

// ---- generated requests -------------------------------------------------------------------

const TEXT_ALPHABET: &str = "[a-zA-Zàáạâấăắ ,.?!\"\\\\\\t\u{1}]{1,24}";

fn pronunciation() -> impl Strategy<Value = Vec<ResolvedPronunciation>> {
    proptest::collection::btree_map("[a-zA-ZĐọ ]{1,6}", "[a-zA-Zâ ]{1,6}", 0..4).prop_map(
        |entries: BTreeMap<String, String>| {
            entries
                .into_iter()
                .map(|(surface, replacement)| ResolvedPronunciation {
                    surface,
                    replacement,
                })
                .collect()
        },
    )
}

fn request() -> impl Strategy<Value = ResolvedSynthesisInput> {
    (
        TEXT_ALPHABET,
        prop_oneof![Just("vi"), Just("vi-VN")],
        prop_oneof![Just("vi"), Just("vi-vn-x-central"), Just("vi-vn-x-south")],
        prop_oneof![Just(900_u16), Just(1_000), Just(1_100)],
        prop_oneof![Just(-1_i16), Just(0), Just(1)],
        proptest::option::of(prop_oneof![Just("neutral"), Just("calm")]),
        proptest::option::of(prop_oneof![Just(0_u16), Just(300)]),
        pronunciation(),
    )
        .prop_map(
            |(text, language, voice, rate, pitch, emotion, intensity, pronunciation)| {
                ResolvedSynthesisInput {
                    dialogue_id: "dialogue-01".into(),
                    character_id: "an".into(),
                    text,
                    language: language.into(),
                    provider_id: "espeak-ng-local".into(),
                    model_id: "espeak-ng-1.52.0".into(),
                    voice_id: voice.into(),
                    voice_rights_record_id: "voice-espeak".into(),
                    adapter_version: "espeak-ng-local-v1".into(),
                    output_contract: "wav-pcm-s16le-mono-22050-v1".into(),
                    performance: SynthesisPerformance {
                        rate_permille: rate,
                        pitch_semitones: pitch,
                        emotion: emotion.map(str::to_owned),
                        intensity_permille: intensity,
                    },
                    pronunciation,
                }
            },
        )
}

/// Independent projection of "what a provider would receive": plain data, no encoder.
type Effective = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    (u16, i16, Option<String>, Option<u16>),
    Vec<(String, String)>,
);

fn effective(input: &ResolvedSynthesisInput) -> Effective {
    let mut pronunciation: Vec<_> = input
        .pronunciation
        .iter()
        .map(|entry| (entry.surface.clone(), entry.replacement.clone()))
        .collect();
    pronunciation.sort();
    (
        input.text.clone(),
        input.language.clone(),
        input.provider_id.clone(),
        input.model_id.clone(),
        input.voice_id.clone(),
        input.adapter_version.clone(),
        input.output_contract.clone(),
        (
            input.performance.rate_permille,
            input.performance.pitch_semitones,
            input.performance.emotion.clone(),
            input.performance.intensity_permille,
        ),
        pronunciation,
    )
}

fn fingerprint(input: &ResolvedSynthesisInput) -> SpeechFingerprint {
    SpeechInput::from_resolved(input).unwrap().fingerprint()
}

#[derive(Debug, Clone, Copy)]
enum Change {
    Text,
    Language,
    Voice,
    Adapter,
    OutputContract,
    Rate,
    Pitch,
    Emotion,
    Intensity,
    PronunciationEdited,
}

/// Every variant is guaranteed to alter the field it names, so a law failure is a real gap.
fn apply(change: Change, input: &mut ResolvedSynthesisInput) {
    match change {
        Change::Text => input.text.push('x'),
        Change::Language => input.language.push('x'),
        Change::Voice => input.voice_id.push('x'),
        Change::Adapter => input.adapter_version.push('x'),
        Change::OutputContract => input.output_contract.push('x'),
        Change::Rate => input.performance.rate_permille ^= 1,
        Change::Pitch => input.performance.pitch_semitones ^= 1,
        Change::Emotion => {
            input.performance.emotion = match input.performance.emotion.take() {
                Some(_) => None,
                None => Some("neutral".into()),
            }
        }
        Change::Intensity => {
            input.performance.intensity_permille = match input.performance.intensity_permille {
                Some(_) => None,
                None => Some(0),
            }
        }
        Change::PronunciationEdited => match input.pronunciation.first_mut() {
            Some(entry) => entry.replacement.push('x'),
            None => input.pronunciation.push(ResolvedPronunciation {
                surface: "zz".into(),
                replacement: "zz".into(),
            }),
        },
    }
}

fn change() -> impl Strategy<Value = Change> {
    prop_oneof![
        Just(Change::Text),
        Just(Change::Language),
        Just(Change::Voice),
        Just(Change::Adapter),
        Just(Change::OutputContract),
        Just(Change::Rate),
        Just(Change::Pitch),
        Just(Change::Emotion),
        Just(Change::Intensity),
        Just(Change::PronunciationEdited),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn equal_requests_have_equal_fingerprints_whatever_their_position_or_owner(
        input in request(),
        dialogue in "[a-z0-9_-]{1,12}",
        character in "[a-z0-9_-]{1,12}",
        rights in "[a-z0-9_-]{1,12}",
        seed in any::<u64>(),
    ) {
        let expected = fingerprint(&input);
        let mut moved = input.clone();
        moved.dialogue_id = dialogue;
        moved.character_id = character;
        moved.voice_rights_record_id = rights;
        // A deterministic rotation stands in for "the list arrived in another order".
        if !moved.pronunciation.is_empty() {
            let by = (seed as usize) % moved.pronunciation.len();
            moved.pronunciation.rotate_left(by);
        }
        prop_assert_eq!(fingerprint(&moved), expected);
    }

    #[test]
    fn any_effective_change_changes_the_fingerprint(input in request(), change in change()) {
        let mut changed = input.clone();
        apply(change, &mut changed);
        prop_assert_ne!(effective(&changed), effective(&input));
        prop_assert_ne!(fingerprint(&changed), fingerprint(&input), "{:?}", change);
    }

    #[test]
    fn fingerprints_agree_exactly_when_the_effective_requests_agree(
        left in request(),
        right in request(),
    ) {
        prop_assert_eq!(
            fingerprint(&left) == fingerprint(&right),
            effective(&left) == effective(&right)
        );
    }

    #[test]
    fn canonical_document_is_valid_json_that_round_trips_every_effective_field(
        input in request(),
    ) {
        let bytes = SpeechInput::from_resolved(&input).unwrap().canonical_bytes();
        let parsed: Value = serde_json::from_slice(&bytes).expect("valid JSON");
        prop_assert_eq!(parsed["text"].as_str().unwrap(), input.text.as_str());
        prop_assert_eq!(parsed["voice_id"].as_str().unwrap(), input.voice_id.as_str());
        prop_assert_eq!(
            parsed["performance"]["rate_permille"].as_u64().unwrap(),
            u64::from(input.performance.rate_permille)
        );
        prop_assert_eq!(
            parsed["performance"]["emotion"].as_str().map(str::to_owned),
            input.performance.emotion.clone()
        );
        let sorted_first_key = bytes.starts_with(br#"{"adapter_version":"#);
        prop_assert!(sorted_first_key, "keys are emitted in sorted order");
        prop_assert!(!parsed.as_object().unwrap().contains_key("dialogue_id"));
        prop_assert!(!parsed.as_object().unwrap().contains_key("voice_rights_record_id"));
    }

    #[test]
    fn a_model_without_a_recorded_pin_never_gets_a_fingerprint(
        mut input in request(),
        provider in "[a-z-]{1,12}",
        model in "[a-z0-9.-]{1,12}",
    ) {
        prop_assume!(!(provider == "espeak-ng-local" && model == "espeak-ng-1.52.0"));
        input.provider_id = provider;
        input.model_id = model;
        prop_assert!(SpeechInput::from_resolved(&input).is_err());
    }
}
