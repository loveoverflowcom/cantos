//! Offline failure and reconciliation advice over immutable, caller-recorded facts.
//! This module neither sends requests nor authorizes a retry. Contract evidence is a
//! recorded assertion; its contents are not fetched or authenticated by this planner.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const MAX_PROVIDER_REFERENCE_BYTES: usize = 256;
pub const MAX_CONTRACT_EVIDENCE_BYTES: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    Timeout,
    RateLimit,
    UnsupportedVoice,
    UnsupportedControl,
    Permanent,
    MalformedAudio,
    Cancelled,
}

/// Failure class alone never establishes whether a provider executed or charged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    NotSent,
    ConfirmedNotExecuted,
    MayHaveExecuted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptFactsError {
    InvalidOperationId,
    InvalidProviderRequestId,
    InvalidContractEvidence,
}

impl fmt::Display for AttemptFactsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidOperationId => "invalid operation identifier",
            Self::InvalidProviderRequestId => "invalid provider request identifier",
            Self::InvalidContractEvidence => "invalid contract evidence reference",
        })
    }
}

impl Error for AttemptFactsError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawProviderReferenceFacts")]
pub struct ProviderReferenceFacts {
    operation_id: Option<String>,
    provider_request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProviderReferenceFacts {
    operation_id: Option<String>,
    provider_request_id: Option<String>,
}

fn valid_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROVIDER_REFERENCE_BYTES
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

impl ProviderReferenceFacts {
    pub fn new(
        operation_id: Option<String>,
        provider_request_id: Option<String>,
    ) -> Result<Self, AttemptFactsError> {
        if operation_id
            .as_deref()
            .is_some_and(|id| !valid_reference(id))
        {
            return Err(AttemptFactsError::InvalidOperationId);
        }
        if provider_request_id
            .as_deref()
            .is_some_and(|id| !valid_reference(id))
        {
            return Err(AttemptFactsError::InvalidProviderRequestId);
        }
        Ok(Self {
            operation_id,
            provider_request_id,
        })
    }

    pub fn operation_id(&self) -> Option<&str> {
        self.operation_id.as_deref()
    }

    pub fn provider_request_id(&self) -> Option<&str> {
        self.provider_request_id.as_deref()
    }
}

impl TryFrom<RawProviderReferenceFacts> for ProviderReferenceFacts {
    type Error = AttemptFactsError;

    fn try_from(raw: RawProviderReferenceFacts) -> Result<Self, Self::Error> {
        Self::new(raw.operation_id, raw.provider_request_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptFacts {
    failure: FailureClass,
    delivery: DeliveryState,
    provider_reference: ProviderReferenceFacts,
}

impl AttemptFacts {
    pub fn new(
        failure: FailureClass,
        delivery: DeliveryState,
        provider_reference: ProviderReferenceFacts,
    ) -> Self {
        Self {
            failure,
            delivery,
            provider_reference,
        }
    }

    pub fn failure(&self) -> FailureClass {
        self.failure
    }

    pub fn delivery(&self) -> DeliveryState {
        self.delivery
    }

    pub fn provider_reference(&self) -> &ProviderReferenceFacts {
        &self.provider_reference
    }
}

/// Reference metadata only; no credentials, source text or provider error body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct ContractEvidence(String);

impl ContractEvidence {
    pub fn new(value: String) -> Result<Self, AttemptFactsError> {
        if value.trim().is_empty()
            || value.len() > MAX_CONTRACT_EVIDENCE_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(AttemptFactsError::InvalidContractEvidence);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ContractEvidence {
    type Error = AttemptFactsError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// A declaration is insufficient for a lookup recommendation. `Verified` records
/// the caller's evidence level; it does not establish actual provider support here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    try_from = "RawContractCapability"
)]
pub enum ContractCapability {
    Unavailable,
    Declared { evidence: ContractEvidence },
    Verified { evidence: ContractEvidence },
}

// Serde's internally tagged unit variants ignore extra fields even with
// `deny_unknown_fields`. An empty struct variant closes that wire-only bypass.
#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum RawContractCapability {
    Unavailable {},
    Declared { evidence: ContractEvidence },
    Verified { evidence: ContractEvidence },
}

impl TryFrom<RawContractCapability> for ContractCapability {
    type Error = AttemptFactsError;

    fn try_from(raw: RawContractCapability) -> Result<Self, Self::Error> {
        Ok(match raw {
            RawContractCapability::Unavailable {} => Self::Unavailable,
            RawContractCapability::Declared { evidence } => Self::Declared { evidence },
            RawContractCapability::Verified { evidence } => Self::Verified { evidence },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationCapabilities {
    lookup_by_operation_id: ContractCapability,
    lookup_by_provider_request_id: ContractCapability,
    same_key_idempotency: ContractCapability,
}

impl ReconciliationCapabilities {
    pub fn new(
        lookup_by_operation_id: ContractCapability,
        lookup_by_provider_request_id: ContractCapability,
        same_key_idempotency: ContractCapability,
    ) -> Self {
        Self {
            lookup_by_operation_id,
            lookup_by_provider_request_id,
            same_key_idempotency,
        }
    }

    pub fn lookup_by_operation_id(&self) -> &ContractCapability {
        &self.lookup_by_operation_id
    }

    pub fn lookup_by_provider_request_id(&self) -> &ContractCapability {
        &self.lookup_by_provider_request_id
    }

    pub fn same_key_idempotency(&self) -> &ContractCapability {
        &self.same_key_idempotency
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum LookupReference {
    OperationId(String),
    ProviderRequestId(String),
}

/// Advice only: no variant permits an automatic resend. A lookup's `not found`
/// response is not evaluated here and cannot itself establish non-execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RecoveryPlan {
    Lookup {
        reference: LookupReference,
        contract_evidence: ContractEvidence,
    },
    ManualReconciliation,
    BoundedRetryNeedsFreshAuthorization,
    FixInput,
    SelectVoiceExplicitly,
    Stop,
    InspectOutput,
}

/// Ambiguity takes precedence over the failure class, including cancellation,
/// unsupported input and invalid audio. Charges must remain separate facts.
/// Idempotency evidence alone omits the verified retention/scope needed to safely
/// resend, so this bounded offline planner never recommends resubmission.
pub fn plan_recovery(
    attempt: &AttemptFacts,
    capabilities: &ReconciliationCapabilities,
) -> RecoveryPlan {
    match attempt.delivery {
        DeliveryState::MayHaveExecuted => plan_lookup(attempt, capabilities),
        DeliveryState::NotSent | DeliveryState::ConfirmedNotExecuted => match attempt.failure {
            FailureClass::Timeout | FailureClass::RateLimit => {
                RecoveryPlan::BoundedRetryNeedsFreshAuthorization
            }
            FailureClass::UnsupportedVoice => RecoveryPlan::SelectVoiceExplicitly,
            FailureClass::UnsupportedControl | FailureClass::Permanent => RecoveryPlan::FixInput,
            FailureClass::MalformedAudio => RecoveryPlan::InspectOutput,
            FailureClass::Cancelled => RecoveryPlan::Stop,
        },
    }
}

fn plan_lookup(attempt: &AttemptFacts, capabilities: &ReconciliationCapabilities) -> RecoveryPlan {
    if let (ContractCapability::Verified { evidence }, Some(id)) = (
        &capabilities.lookup_by_provider_request_id,
        attempt.provider_reference.provider_request_id(),
    ) {
        return RecoveryPlan::Lookup {
            reference: LookupReference::ProviderRequestId(id.into()),
            contract_evidence: evidence.clone(),
        };
    }
    if let (ContractCapability::Verified { evidence }, Some(id)) = (
        &capabilities.lookup_by_operation_id,
        attempt.provider_reference.operation_id(),
    ) {
        return RecoveryPlan::Lookup {
            reference: LookupReference::OperationId(id.into()),
            contract_evidence: evidence.clone(),
        };
    }
    RecoveryPlan::ManualReconciliation
}
