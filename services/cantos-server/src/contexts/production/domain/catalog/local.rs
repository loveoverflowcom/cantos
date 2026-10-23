//! Exact local eSpeak instruction identity for an external producer; no process, provider SDK
//! or network dependency.
use crate::contract::studio::ProductionCapability;
use unicode_normalization::UnicodeNormalization;

pub const PROVIDER_ID: &str = "espeak-ng-local";
pub const MODEL_ID: &str = "espeak-ng-1.52.0";
pub const INSTRUCTION_VERSION: &str = "espeak-ng-local-v1";
pub const OUTPUT_CONTRACT: &str = "wav-pcm-s16le-mono-22050-v1";
pub const MAX_TEXT_BYTES: usize = 2_000;

/// Shared admission for the exact text sent to the local runtime, after resolved
/// pronunciation replacements. eSpeak parses `[[` as phoneme input by default;
/// rejecting the opening marker also rejects incomplete or split-origin syntax.
pub fn valid_text(text: &str) -> bool {
    !text.trim().is_empty()
        && text.len() <= MAX_TEXT_BYTES
        && !text
            .chars()
            .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
        && text.nfc().eq(text.chars())
        && !text.contains("[[")
}

/// Instructable support, not a claim that the runtime is installed on any server.
pub fn capability() -> ProductionCapability {
    ProductionCapability {
        provider_id: PROVIDER_ID.into(),
        model_id: MODEL_ID.into(),
        voice_ids: ["vi", "vi-vn-x-central", "vi-vn-x-south"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
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
