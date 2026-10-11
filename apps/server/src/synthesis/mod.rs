//! Provider-neutral vocabulary for one speech-synthesis attempt and its accountable outcome.
//!
//! These values describe what an adapter observed and what may happen next. They persist
//! nothing, reserve no budget and never repeat a request on their own; durable attempts,
//! leases, reservations and retry schedules belong to the production job slice (#7).

pub mod google;
pub mod google_http;

use serde::Serialize;

use crate::wav_inspection::StructuralError;

/// What the shell saw after handing one request to a transport. Built by transports only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportObservation {
    /// The connection (TCP and TLS) was never established, so no request byte left this host.
    NotSent,
    /// The request may have reached the provider; its outcome was then lost.
    LostAfterSend(LostCause),
    /// The provider answered with an HTTP status.
    Responded { status: u16, body: ResponseBody },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LostCause {
    Timeout,
    ConnectionError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseBody {
    Complete(Vec<u8>),
    /// Reading stopped at the adapter's response limit; the remainder was never buffered.
    TooLarge,
    /// The connection failed or timed out after the status line arrived.
    Interrupted,
}

/// Why one attempt produced no candidate audio. Each adapter maps its provider's observations
/// into these classes and owns the policy for each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    NotSent,
    InvalidInput,
    AccountRejected,
    RateLimited,
    ProviderError,
    Ambiguous,
    MalformedAudio,
    UnexpectedResponse,
}

impl FailureClass {
    pub const ALL: [FailureClass; 8] = [
        FailureClass::NotSent,
        FailureClass::InvalidInput,
        FailureClass::AccountRejected,
        FailureClass::RateLimited,
        FailureClass::ProviderError,
        FailureClass::Ambiguous,
        FailureClass::MalformedAudio,
        FailureClass::UnexpectedResponse,
    ];
}

/// Whether this attempt can have been billed. No variant is a settled charge: settlement needs
/// the provider's billing data, which no synthesis response carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeExposure {
    /// Nothing reached the provider.
    None,
    /// The provider executed the request; usage is the counted billable characters.
    UsageObserved,
    /// The request reached or may have reached the provider; billing is undocumented or lost.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryDecision {
    /// Repeating cannot duplicate an unknown charge; still bounded by the caller's attempt
    /// limit and budget.
    RetryAllowed,
    /// A repeat could duplicate a charge or output; resolve this attempt explicitly first.
    ResolveBeforeRetry,
    /// The identical request cannot succeed; change its input, casting or account.
    DoNotRetry,
}

/// The useful next step offered to the creator or operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    CheckConnectivity,
    FixLineOrCasting,
    FixProviderAccount,
    WaitForQuotaThenResolve,
    ResolveUncertainAttempt,
    RetryAndReportIfRepeated,
    ReportAdapterMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FailurePolicy {
    pub charge: ChargeExposure,
    pub retry: RetryDecision,
    pub recovery: RecoveryAction,
}

/// Returned audio that cannot become a candidate artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AudioDefect {
    ResponseTooLarge {
        limit_bytes: usize,
    },
    ResponseInterrupted,
    NotJson,
    MissingAudioContent,
    InvalidBase64,
    EmptyAudio,
    InvalidWav {
        errors: Vec<StructuralError>,
    },
    UnexpectedFormat {
        format_tag: u16,
        channels: u16,
        sample_rate_hz: u32,
        bits_per_sample: u16,
    },
    NoFrames,
}

/// What a failed attempt is known from. Provider error messages are never kept: they can
/// echo private dialogue text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FailureEvidence {
    NotConnected,
    Lost {
        cause: LostCause,
    },
    Status {
        http_status: u16,
        provider_status: Option<String>,
    },
    Audio {
        http_status: u16,
        defect: AudioDefect,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttemptFailure {
    pub class: FailureClass,
    pub evidence: FailureEvidence,
    #[serde(flatten)]
    pub policy: FailurePolicy,
}

/// Structural facts of accepted-candidate audio. Listening quality is not assessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioFacts {
    pub bytes: u64,
    pub sha256: String,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub frames: u64,
}

/// Probed audio bytes, constructible only by an adapter's validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAudio {
    bytes: Vec<u8>,
    facts: AudioFacts,
}

impl ValidatedAudio {
    pub(crate) fn new(bytes: Vec<u8>, facts: AudioFacts) -> ValidatedAudio {
        ValidatedAudio { bytes, facts }
    }

    pub fn facts(&self) -> &AudioFacts {
        &self.facts
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptOutcome {
    Succeeded(ValidatedAudio),
    Failed(AttemptFailure),
}

impl AttemptOutcome {
    pub fn charge(&self) -> ChargeExposure {
        match self {
            AttemptOutcome::Succeeded(_) => ChargeExposure::UsageObserved,
            AttemptOutcome::Failed(failure) => failure.policy.charge,
        }
    }
}
