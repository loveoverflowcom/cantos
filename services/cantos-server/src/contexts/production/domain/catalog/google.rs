//! Exact Google Cloud TTS instruction identity and declared vi-VN capabilities that an external
//! producer follows; Cantos has no network dependency or dispatch. The voice list and control
//! ranges were read from Google's documentation on [`CAPABILITIES_VERIFIED_ON`]; they are not
//! evidence that a voice is enabled for an account.
use crate::contract::studio::ProductionCapability;
use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

pub const PROVIDER_ID: &str = "google-cloud-tts";
/// Bump for any change to the request mapping, controls, output contract or validation. The
/// committed `-synthesize-` spelling names Google's API method and stays in serialized lines.
pub const INSTRUCTION_VERSION: &str = "google-cloud-tts-synthesize-3";
/// Frozen v2 inputs remain inspectable history; this identity never enables current dispatch.
pub const HISTORICAL_INSTRUCTION_VERSION: &str = "google-cloud-tts-synthesize-2";
pub const OUTPUT_CONTRACT: &str = "wav-pcm-s16le-mono-24000-v1";
/// Google's "total bytes per request" content limit, which cannot be raised.
pub const MAX_TEXT_BYTES: usize = 5_000;
pub const CAPABILITIES_VERIFIED_ON: &str = "2026-10-11";
/// Google's only Vietnamese locale; `vi` scripts are sent as `vi-VN`.
pub const LANGUAGE_CODE: &str = "vi-VN";
pub const LANGUAGES: [&str; 2] = ["vi", "vi-VN"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum VoiceFamily {
    Chirp3Hd,
    Neural2,
    Wavenet,
    Standard,
}

impl VoiceFamily {
    pub const ALL: [VoiceFamily; 4] = [
        VoiceFamily::Chirp3Hd,
        VoiceFamily::Neural2,
        VoiceFamily::Wavenet,
        VoiceFamily::Standard,
    ];

    /// Google selects the model through the voice name; this is Cantos' casting model ID.
    pub const fn model_id(self) -> &'static str {
        match self {
            VoiceFamily::Chirp3Hd => "google-chirp3-hd",
            VoiceFamily::Neural2 => "google-neural2",
            VoiceFamily::Wavenet => "google-wavenet",
            VoiceFamily::Standard => "google-standard",
        }
    }

    /// Chirp 3: HD voices support neither the speaking-rate nor the pitch parameter.
    pub const fn supports_rate_and_pitch(self) -> bool {
        match self {
            VoiceFamily::Standard | VoiceFamily::Wavenet | VoiceFamily::Neural2 => true,
            VoiceFamily::Chirp3Hd => false,
        }
    }

    /// Documented `speakingRate` [0.25, 2.0] as permille; fixed at 1,000 when unsupported.
    pub const fn rate_permille(self) -> (u16, u16) {
        if self.supports_rate_and_pitch() {
            (250, 2_000)
        } else {
            (1_000, 1_000)
        }
    }

    /// Documented `pitch` [-20.0, 20.0] semitones; fixed at 0 when unsupported.
    pub const fn pitch_semitones(self) -> (i16, i16) {
        if self.supports_rate_and_pitch() {
            (-20, 20)
        } else {
            (0, 0)
        }
    }

    /// Published list price after the free monthly allowance, in US cents per million characters.
    pub const fn cents_per_million_characters(self) -> u64 {
        match self {
            VoiceFamily::Standard | VoiceFamily::Wavenet => 400,
            VoiceFamily::Neural2 => 1_600,
            VoiceFamily::Chirp3Hd => 3_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceGender {
    Female,
    Male,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct GoogleVoice {
    pub name: &'static str,
    pub family: VoiceFamily,
    pub gender: VoiceGender,
}

const fn voice(name: &'static str, family: VoiceFamily, gender: VoiceGender) -> GoogleVoice {
    GoogleVoice {
        name,
        family,
        gender,
    }
}

use VoiceFamily::{Chirp3Hd, Neural2, Standard, Wavenet};
use VoiceGender::{Female, Male};

/// Every vi-VN voice Google listed on [`CAPABILITIES_VERIFIED_ON`].
pub static VI_VN_VOICES: [GoogleVoice; 40] = [
    voice("vi-VN-Chirp3-HD-Achernar", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Achird", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Algenib", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Algieba", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Alnilam", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Aoede", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Autonoe", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Callirrhoe", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Charon", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Despina", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Enceladus", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Erinome", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Fenrir", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Gacrux", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Iapetus", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Kore", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Laomedeia", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Leda", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Orus", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Puck", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Pulcherrima", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Rasalgethi", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Sadachbia", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Sadaltager", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Schedar", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Sulafat", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Umbriel", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Vindemiatrix", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Zephyr", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Zubenelgenubi", Chirp3Hd, Male),
    voice("vi-VN-Neural2-A", Neural2, Female),
    voice("vi-VN-Neural2-D", Neural2, Male),
    voice("vi-VN-Standard-A", Standard, Female),
    voice("vi-VN-Standard-B", Standard, Male),
    voice("vi-VN-Standard-C", Standard, Female),
    voice("vi-VN-Standard-D", Standard, Male),
    voice("vi-VN-Wavenet-A", Wavenet, Female),
    voice("vi-VN-Wavenet-B", Wavenet, Male),
    voice("vi-VN-Wavenet-C", Wavenet, Female),
    voice("vi-VN-Wavenet-D", Wavenet, Male),
];

pub fn find_voice(name: &str) -> Option<&'static GoogleVoice> {
    VI_VN_VOICES.iter().find(|voice| voice.name == name)
}

/// Shared admission for the exact text a producer sends to Google, after resolved pronunciation
/// replacements: nonblank NFC plain text without control or line/paragraph separators.
pub fn valid_text(text: &str) -> bool {
    !text.trim().is_empty()
        && text.len() <= MAX_TEXT_BYTES
        && !text
            .chars()
            .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
        && text.nfc().eq(text.chars())
}

/// Instructable support per voice family. Google has no emotion or intensity control, so only
/// the explicit `neutral` delivery at intensity 0 is castable; it means the voice's default
/// delivery.
pub fn capabilities() -> Vec<ProductionCapability> {
    VoiceFamily::ALL
        .into_iter()
        .map(|family| {
            let (rate_min_permille, rate_max_permille) = family.rate_permille();
            let (pitch_min_semitones, pitch_max_semitones) = family.pitch_semitones();
            ProductionCapability {
                provider_id: PROVIDER_ID.into(),
                model_id: family.model_id().into(),
                voice_ids: VI_VN_VOICES
                    .iter()
                    .filter(|voice| voice.family == family)
                    .map(|voice| voice.name.into())
                    .collect(),
                languages: LANGUAGES
                    .iter()
                    .map(|language| (*language).into())
                    .collect(),
                emotions: vec!["neutral".into()],
                rate_min_permille,
                rate_max_permille,
                pitch_min_semitones,
                pitch_max_semitones,
                intensity_supported: false,
                pronunciation_supported: true,
                reference_only: false,
                billable_dispatch_available: false,
            }
        })
        .collect()
}
