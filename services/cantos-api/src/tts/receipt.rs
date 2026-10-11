use cantos_contracts::{FrozenProductionDocument, ProductionCurrency};
use serde::{Deserialize, Serialize};

use super::attempt::{
    plan_recovery, AttemptFacts, DeliveryState, FailureClass, ProviderReferenceFacts,
    ReconciliationCapabilities, RecoveryPlan,
};
use super::audio::{inspect_output_for_profile, AudioError, InspectedTtsAudio, OutputProfile};
use super::{google_contract, local_contract, neural_contract};
use crate::production::{prepare_inputs, MAX_MONEY_MINOR, MAX_PRODUCTION_DOCUMENT_BYTES};

pub const RECEIPT_VERSION: &str = "cantos-tts-receipt-1";
pub const MAX_RECEIPT_BYTES: usize = 64 * 1024;

/// Provenance declarations, never proof that a provider was called or authorized.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptEvidence {
    SyntheticFixture,
    CallerReported,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TtsProviderIdentity {
    pub provider_id: String,
    pub model_id: String,
    pub voice_id: String,
    pub adapter_version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum UsageObservation {
    Unknown {},
    Reported { unit: String, count: u64 },
}

/// Usage alone establishes no charge. A reported charge remains a caller assertion.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChargeObservation {
    Unknown {},
    Reported {
        currency: ProductionCurrency,
        amount_minor: u64,
        reference: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReceiptOutcome {
    Output {
        audio_sha256: String,
    },
    Failure {
        class: FailureClass,
        delivery: DeliveryState,
    },
}

/// Raw local evidence contract. Admission happens only through `audit_receipt`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TtsReceipt {
    pub version: String,
    pub evidence: ReceiptEvidence,
    pub attempt_id: String,
    pub operation_id: String,
    pub provider_request_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub input_digest: String,
    pub dialogue_id: String,
    pub provider: TtsProviderIdentity,
    pub reconciliation: ReconciliationCapabilities,
    pub usage: UsageObservation,
    pub charge: ChargeObservation,
    pub outcome: ReceiptOutcome,
}

/// An inspection fact, not an accepted artifact, a receipt signature or dispatch permission.
/// Private fields and Serialize-only prevent decoding an unaudited result as evidence.
#[derive(Debug, Serialize)]
pub struct ReceiptAudit {
    version: &'static str,
    assessment: &'static str,
    receipt: TtsReceipt,
    script_id: String,
    script_revision: u64,
    export_digest: String,
    audio: Option<InspectedTtsAudio>,
    recovery: Option<RecoveryPlan>,
}

impl ReceiptAudit {
    pub fn audio(&self) -> Option<&InspectedTtsAudio> {
        self.audio.as_ref()
    }

    pub fn recovery(&self) -> Option<&RecoveryPlan> {
        self.recovery.as_ref()
    }

    pub fn receipt(&self) -> &TtsReceipt {
        &self.receipt
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum ReceiptError {
    DocumentTooLarge,
    ReceiptTooLarge,
    InvalidDocumentJson,
    InvalidReceiptJson,
    InvalidDocument,
    UnsupportedVersion,
    InvalidField { field: &'static str },
    InputDigestMismatch,
    DialogueMissing,
    ProviderIdentityMismatch,
    ReferenceOnlyOutputClaim,
    AudioRequired,
    AudioUnexpected,
    InvalidAudio { error: AudioError },
}

pub fn decode_document(bytes: &[u8]) -> Result<FrozenProductionDocument, ReceiptError> {
    if bytes.len() > MAX_PRODUCTION_DOCUMENT_BYTES {
        return Err(ReceiptError::DocumentTooLarge);
    }
    serde_json::from_slice(bytes).map_err(|_| ReceiptError::InvalidDocumentJson)
}

pub fn decode_receipt(bytes: &[u8]) -> Result<TtsReceipt, ReceiptError> {
    if bytes.len() > MAX_RECEIPT_BYTES {
        return Err(ReceiptError::ReceiptTooLarge);
    }
    serde_json::from_slice(bytes).map_err(|_| ReceiptError::InvalidReceiptJson)
}

pub fn audit_receipt(
    document: &FrozenProductionDocument,
    receipt: &TtsReceipt,
    audio_bytes: Option<&[u8]>,
) -> Result<ReceiptAudit, ReceiptError> {
    validate_receipt(receipt)?;
    // Reconstruct using the existing authority, including complete export and resolved controls.
    // Time/review only affect findings here; no current rights or approval claim is made.
    if serde_json::to_vec(document)
        .map_or(true, |bytes| bytes.len() > MAX_PRODUCTION_DOCUMENT_BYTES)
    {
        return Err(ReceiptError::DocumentTooLarge);
    }
    let admitted = prepare_inputs(
        &document.owner_id,
        &document.revision,
        &document.settings,
        &document.rights,
        true,
        0,
    )
    .map_err(|_| ReceiptError::InvalidDocument)?;
    if admitted.document() != *document {
        return Err(ReceiptError::InvalidDocument);
    }
    if receipt.input_digest != document.input_digest {
        return Err(ReceiptError::InputDigestMismatch);
    }
    let line = document
        .resolved
        .iter()
        .find(|line| line.dialogue_id == receipt.dialogue_id)
        .ok_or(ReceiptError::DialogueMissing)?;
    let provider = &receipt.provider;
    if provider.provider_id != line.provider_id
        || provider.model_id != line.model_id
        || provider.voice_id != line.voice_id
        || provider.adapter_version != line.adapter_version
    {
        return Err(ReceiptError::ProviderIdentityMismatch);
    }
    let (audio, recovery) = match &receipt.outcome {
        ReceiptOutcome::Output { audio_sha256 } => {
            // A reference-only candidate cannot acquire synthesis support through an upload.
            let profile = if line.provider_id == local_contract::PROVIDER_ID
                && line.adapter_version == local_contract::ADAPTER_VERSION
                && line.output_contract == local_contract::OUTPUT_CONTRACT
            {
                OutputProfile::Espeak22Khz
            } else if line.provider_id == neural_contract::PROVIDER_ID
                && line.model_id == neural_contract::MODEL_ID
                && line.adapter_version == neural_contract::ADAPTER_VERSION
                && line.output_contract == neural_contract::OUTPUT_CONTRACT
            {
                // This profile admits research bytes only. ReceiptAudit has no
                // dispatch, accepted-artifact or publication conversion path.
                OutputProfile::Neural24Khz
            } else if line.provider_id == google_contract::PROVIDER_ID
                && line.adapter_version == google_contract::ADAPTER_VERSION
                && line.output_contract == google_contract::OUTPUT_CONTRACT
            {
                OutputProfile::Google24Khz
            } else if receipt.evidence == ReceiptEvidence::SyntheticFixture {
                OutputProfile::Offline24Khz
            } else {
                return Err(ReceiptError::ReferenceOnlyOutputClaim);
            };
            let bytes = audio_bytes.ok_or(ReceiptError::AudioRequired)?;
            let audio = inspect_output_for_profile(bytes, audio_sha256, profile)
                .map_err(|error| ReceiptError::InvalidAudio { error })?;
            (Some(audio), None)
        }
        ReceiptOutcome::Failure { class, delivery } => {
            if audio_bytes.is_some() {
                return Err(ReceiptError::AudioUnexpected);
            }
            let references = ProviderReferenceFacts::new(
                Some(receipt.operation_id.clone()),
                receipt.provider_request_id.clone(),
            )
            .map_err(|_| ReceiptError::InvalidField {
                field: "provider_reference",
            })?;
            let facts = AttemptFacts::new(*class, *delivery, references);
            (None, Some(plan_recovery(&facts, &receipt.reconciliation)))
        }
    };
    Ok(ReceiptAudit {
        version: RECEIPT_VERSION,
        assessment: "offline-inspection-only-no-dispatch-or-acceptance",
        receipt: receipt.clone(),
        script_id: document.revision.script_id.clone(),
        script_revision: document.revision.revision,
        export_digest: document.revision.export_digest.clone(),
        audio,
        recovery,
    })
}

fn validate_receipt(receipt: &TtsReceipt) -> Result<(), ReceiptError> {
    if serde_json::to_vec(receipt).map_or(true, |bytes| bytes.len() > MAX_RECEIPT_BYTES) {
        return Err(ReceiptError::ReceiptTooLarge);
    }
    if receipt.version != RECEIPT_VERSION {
        return Err(ReceiptError::UnsupportedVersion);
    }
    for (field, value) in [
        ("attempt_id", receipt.attempt_id.as_str()),
        ("operation_id", receipt.operation_id.as_str()),
        ("dialogue_id", receipt.dialogue_id.as_str()),
        ("provider_id", receipt.provider.provider_id.as_str()),
        ("model_id", receipt.provider.model_id.as_str()),
        ("voice_id", receipt.provider.voice_id.as_str()),
        ("adapter_version", receipt.provider.adapter_version.as_str()),
    ] {
        validate_identifier(field, value)?;
    }
    for (field, value) in [
        ("provider_request_id", &receipt.provider_request_id),
        ("idempotency_key", &receipt.idempotency_key),
    ] {
        if let Some(value) = value {
            validate_identifier(field, value)?;
        }
    }
    if let UsageObservation::Reported { unit, count } = &receipt.usage {
        validate_identifier("usage.unit", unit)?;
        if *count > MAX_MONEY_MINOR {
            return Err(ReceiptError::InvalidField {
                field: "usage.count",
            });
        }
    }
    if let ChargeObservation::Reported {
        amount_minor,
        reference,
        ..
    } = &receipt.charge
    {
        validate_identifier("charge.reference", reference)?;
        if *amount_minor > MAX_MONEY_MINOR {
            return Err(ReceiptError::InvalidField {
                field: "charge.amount_minor",
            });
        }
    }
    Ok(())
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), ReceiptError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(ReceiptError::InvalidField { field });
    }
    Ok(())
}
