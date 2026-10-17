//! Deterministic session-only adapter. No timers, credentials, media or remote effects.
use super::model::{
    Completion, ForgePort, GenerationOutcome, Outcome, Request, RequestKind, SyncOutcome,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Success,
    Failure,
    Unknown,
    StorageFailure,
    Partial,
}
pub struct MockPort {
    scenario: Scenario,
}
impl MockPort {
    pub fn new(scenario: Scenario) -> Self {
        Self { scenario }
    }
    pub fn set_scenario(&mut self, scenario: Scenario) {
        self.scenario = scenario;
    }
}
impl ForgePort for MockPort {
    fn complete(&mut self, request: &Request) -> Completion {
        let outcome = match request.kind {
            RequestKind::Generate { .. } | RequestKind::ReconcileRun { .. } => {
                Outcome::Generated(match self.scenario {
                    Scenario::Success | Scenario::Partial => GenerationOutcome::Retained,
                    Scenario::Failure => GenerationOutcome::Failed,
                    Scenario::Unknown => GenerationOutcome::Uncertain,
                    Scenario::StorageFailure => GenerationOutcome::StorageFailed,
                })
            }
            RequestKind::Sync { .. } | RequestKind::ReconcileSync { .. } => {
                Outcome::Synced(match self.scenario {
                    Scenario::Success => SyncOutcome::Receipt,
                    Scenario::Partial => SyncOutcome::Partial,
                    Scenario::Failure | Scenario::Unknown | Scenario::StorageFailure => {
                        SyncOutcome::Unknown
                    }
                })
            }
        };
        Completion {
            request: request.clone(),
            outcome,
        }
    }
}
