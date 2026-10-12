//! Admission and exact frozen-input identity for the bounded local execution modes.
use super::JobsError;
use crate::contexts::production::domain::catalog::local as local_contract;
use cantos_contracts::ResolvedSynthesisInput;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;

pub const INPUT_FINGERPRINT_SCHEME: &str = "durable-input-v1";

/// Synthetic execution is an explicit simulation mode, never a catalog provider or live proof.
/// Both modes consume the same exact, admitted local eSpeak frozen input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    LocalEspeak,
    SyntheticFake,
}

impl ExecutionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalEspeak => "local_espeak",
            Self::SyntheticFake => "synthetic_fake",
        }
    }
}

impl FromStr for ExecutionMode {
    type Err = JobsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "local_espeak" => Ok(Self::LocalEspeak),
            "synthetic_fake" => Ok(Self::SyntheticFake),
            _ => Err(JobsError::InvalidExecutionMode),
        }
    }
}

/// Admission is a defense in depth after complete frozen-document reconstruction.
/// This pure gate uses the existing local capability contract. The local process adapter
/// additionally constructs its existing validated request before invocation.
pub fn validate_execution_mode(
    mode: ExecutionMode,
    input: &ResolvedSynthesisInput,
) -> Result<(), JobsError> {
    match mode {
        ExecutionMode::LocalEspeak | ExecutionMode::SyntheticFake => {
            let capability = local_contract::capability();
            if input.provider_id != local_contract::PROVIDER_ID
                || input.model_id != local_contract::MODEL_ID
                || input.adapter_version != local_contract::ADAPTER_VERSION
                || input.output_contract != local_contract::OUTPUT_CONTRACT
                || !capability.voice_ids.contains(&input.voice_id)
                || !capability.languages.contains(&input.language)
                || input.performance.rate_permille != 1_000
                || input.performance.pitch_semitones != 0
                || input.performance.emotion.as_deref() != Some("neutral")
                || input.performance.intensity_permille != Some(0)
                || !local_contract::valid_text(&input.text)
            {
                return Err(JobsError::UnsupportedExecutionInput);
            }
            Ok(())
        }
    }
}

/// Identity over every serialized resolved field, including dialogue, character and rights
/// references. This deliberately scoped identity does not implement cross-run speech reuse.
pub fn input_fingerprint(input: &ResolvedSynthesisInput) -> Result<String, JobsError> {
    let bytes = serde_json::to_vec(input).map_err(|_| JobsError::FingerprintEncoding)?;
    let mut hash = Sha256::new();
    hash.update(b"cantos/durable-input/v1\n");
    hash.update(bytes);
    Ok(format!(
        "{INPUT_FINGERPRINT_SCHEME}:sha256:{:x}",
        hash.finalize()
    ))
}
