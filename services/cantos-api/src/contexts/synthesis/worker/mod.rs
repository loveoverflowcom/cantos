//! Local development worker over the existing PostgreSQL production authority.
//!
//! Runtime owns claims and heartbeats, dispatch owns provider execution, recovery owns reuse
//! and recorded-output acceptance, and output owns the private filesystem journal. Durable
//! state transitions remain inside the same transactions; no commercial transport is reachable.
mod dispatch;
mod output;
mod recovery;
mod runtime;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use crate::contexts::synthesis::{
    jobs::{ExecutionMode, LeaseDuration},
    DurableError,
};

pub(crate) use output::{prepare_root, read_output};
pub use runtime::run_worker;

/// Explicit configuration is required; a default worker can never dispatch a provider.
#[derive(Clone, Debug)]
pub enum WorkerProvider {
    LocalEspeak { runtime: PathBuf },
    SyntheticFake,
}

impl WorkerProvider {
    pub fn mode(&self) -> ExecutionMode {
        match self {
            Self::LocalEspeak { .. } => ExecutionMode::LocalEspeak,
            Self::SyntheticFake => ExecutionMode::SyntheticFake,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failpoint {
    AfterClaim,
    /// After a clip's object was probed, immediately before the fenced reuse selection.
    BeforeReuse,
    AfterIntent,
    AfterProvider,
    AfterObservation,
    BeforeAccept,
    AfterAccept,
}

impl Failpoint {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "after_claim" => Some(Self::AfterClaim),
            "before_reuse" => Some(Self::BeforeReuse),
            "after_intent" => Some(Self::AfterIntent),
            "after_provider" => Some(Self::AfterProvider),
            "after_observation" => Some(Self::AfterObservation),
            "before_accept" => Some(Self::BeforeAccept),
            "after_accept" => Some(Self::AfterAccept),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AfterClaim => "after_claim",
            Self::BeforeReuse => "before_reuse",
            Self::AfterIntent => "after_intent",
            Self::AfterProvider => "after_provider",
            Self::AfterObservation => "after_observation",
            Self::BeforeAccept => "before_accept",
            Self::AfterAccept => "after_accept",
        }
    }
}

pub struct WorkerConfig {
    pub provider: WorkerProvider,
    pub output_root: PathBuf,
    pub worker_id: String,
    pub lease_duration: LeaseDuration,
    pub once: bool,
    /// Only valid for the explicitly enabled synthetic provider.
    pub failpoint: Option<Failpoint>,
}

#[derive(Debug)]
pub enum WorkerError {
    InvalidConfiguration,
    UnsafeOutput,
    OutputIo,
    InvalidReceipt,
    FailpointInput,
    Durable(DurableError),
}

impl From<DurableError> for WorkerError {
    fn from(value: DurableError) -> Self {
        Self::Durable(value)
    }
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No provider error bodies, dialogue text or database URLs enter operational logs.
        write!(formatter, "local worker failed: {self:?}")
    }
}

impl std::error::Error for WorkerError {}
