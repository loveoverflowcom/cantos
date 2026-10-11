//! Exact research-only VieNeu Nano identity and pure input admission.
use cantos_contracts::{ProductionCapability, ResolvedSynthesisInput};
use unicode_normalization::UnicodeNormalization;

pub const PROVIDER_ID: &str = "vieneu-nano-local";
pub const MODEL_REVISION: &str = "aba295eb96a6fa6003ebe417cc1f2802a7adc1dc";
pub const MODEL_ID: &str = "vieneu-nano-aba295eb96a6fa6003ebe417cc1f2802a7adc1dc";
pub const ADAPTER_VERSION: &str = "vieneu-nano-local-v1";
pub const OUTPUT_CONTRACT: &str = "wav-pcm-s16le-mono-24000-vieneu-v1";
pub const MAX_TEXT_BYTES: usize = 2_000;
/// Explicit SDK presets Đức Trí and Mỹ Duyên; no arbitrary speaker or cloning input.
pub const VOICE_IDS: [&str; 2] = ["duc-tri", "my-duyen"];

const VIETNAMESE_LETTERS: &str = "àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđÀÁẢÃẠĂẰẮẲẴẶÂẦẤẨẪẬÈÉẺẼẸÊỀẾỂỄỆÌÍỈĨỊÒÓỎÕỌÔỒỐỔỖỘƠỜỚỞỠỢÙÚỦŨỤƯỪỨỬỮỰỲÝỶỸỴĐ";

pub fn valid_text(text: &str) -> bool {
    !text.trim().is_empty()
        && text.len() <= MAX_TEXT_BYTES
        && text.nfc().eq(text.chars())
        && text.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || VIETNAMESE_LETTERS.contains(character)
                || " .,!?;:-'\"()".contains(character)
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeuralRequestError {
    InvalidDocument,
    DialogueMissing,
    WrongProvider,
    WrongModel,
    WrongAdapter,
    WrongOutputContract,
    UnsupportedVoice,
    UnsupportedLanguage,
    UnsupportedRate,
    UnsupportedPitch,
    UnsupportedEmotion,
    UnsupportedIntensity,
    InvalidText,
}

/// Validate effective text and controls without a process, filesystem or provider SDK.
pub fn validate_resolved(input: &ResolvedSynthesisInput) -> Result<(), NeuralRequestError> {
    for (matches, error) in [
        (
            input.provider_id == PROVIDER_ID,
            NeuralRequestError::WrongProvider,
        ),
        (input.model_id == MODEL_ID, NeuralRequestError::WrongModel),
        (
            input.adapter_version == ADAPTER_VERSION,
            NeuralRequestError::WrongAdapter,
        ),
        (
            input.output_contract == OUTPUT_CONTRACT,
            NeuralRequestError::WrongOutputContract,
        ),
        (
            VOICE_IDS.contains(&input.voice_id.as_str()),
            NeuralRequestError::UnsupportedVoice,
        ),
        (
            ["vi", "vi-VN"].contains(&input.language.as_str()),
            NeuralRequestError::UnsupportedLanguage,
        ),
        (
            input.performance.rate_permille == 1_000,
            NeuralRequestError::UnsupportedRate,
        ),
        (
            input.performance.pitch_semitones == 0,
            NeuralRequestError::UnsupportedPitch,
        ),
        (
            input.performance.emotion.as_deref() == Some("neutral"),
            NeuralRequestError::UnsupportedEmotion,
        ),
        (
            input.performance.intensity_permille == Some(0),
            NeuralRequestError::UnsupportedIntensity,
        ),
        (valid_text(&input.text), NeuralRequestError::InvalidText),
    ] {
        if !matches {
            return Err(error);
        }
    }
    Ok(())
}

/// Executable research support; neither this catalog nor a snapshot grants production use.
pub fn capability() -> ProductionCapability {
    ProductionCapability {
        provider_id: PROVIDER_ID.into(),
        model_id: MODEL_ID.into(),
        voice_ids: VOICE_IDS.into_iter().map(str::to_owned).collect(),
        languages: ["vi", "vi-VN"].into_iter().map(str::to_owned).collect(),
        emotions: vec!["neutral".into()],
        rate_min_permille: 1_000,
        rate_max_permille: 1_000,
        pitch_min_semitones: 0,
        pitch_max_semitones: 0,
        intensity_supported: false,
        pronunciation_supported: true,
        reference_only: false,
        billable_dispatch_available: false,
    }
}
