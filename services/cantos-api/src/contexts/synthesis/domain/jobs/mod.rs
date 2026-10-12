//! Pure durable execution rules. A completed run establishes execution only, never approval.
//! Durable-input fingerprints preserve every frozen field and remain distinct from clip keys.
mod execution;
mod retry;
mod state;
mod step;
#[cfg(test)]
mod tests;

use std::{error::Error, fmt};

pub use execution::{
    input_fingerprint, validate_execution_mode, ExecutionMode, INPUT_FINGERPRINT_SCHEME,
};
pub use retry::{LeaseDuration, RetryPolicy, MAX_ATTEMPTS, MAX_BACKOFF_SECONDS, MAX_LEASE_SECONDS};
pub use state::{evolve_run, RunEvent, RunStatus, TransitionError};
pub use step::{decide_next_step, AttemptOutcome, StepDecision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobsError {
    InvalidRunStatus,
    InvalidExecutionMode,
    InvalidMaxAttempts,
    InvalidBaseDelay,
    InvalidBackoffCap,
    InvalidLeaseDuration,
    UnsupportedExecutionInput,
    FingerprintEncoding,
}

impl fmt::Display for JobsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRunStatus => "unknown run status",
            Self::InvalidExecutionMode => "unknown execution mode",
            Self::InvalidMaxAttempts => "max attempts must be between 1 and 8",
            Self::InvalidBaseDelay => "base retry delay must be between 1 and 86400 seconds",
            Self::InvalidBackoffCap => {
                "retry delay cap must be at least the base delay and at most 86400 seconds"
            }
            Self::InvalidLeaseDuration => "lease duration must be between 1 and 300 seconds",
            Self::UnsupportedExecutionInput => {
                "execution requires the exact admitted local eSpeak input"
            }
            Self::FingerprintEncoding => "cannot encode durable input fingerprint",
        })
    }
}

impl Error for JobsError {}
