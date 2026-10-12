//! Conservative step decisions from observed attempt facts.
use super::RetryPolicy;
use crate::contexts::synthesis::domain::attempt::{DeliveryState, FailureClass};

/// Only an already audited, exactly linked output may be supplied as Output by the shell.
/// These are recovery facts, not dispatch authorization or proof of provider support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptOutcome {
    None,
    Unresolved,
    Output,
    Failure {
        class: FailureClass,
        delivery: DeliveryState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepDecision {
    Dispatch,
    Accept,
    Reconcile,
    Fail,
    RetryAfter(u32),
}

/// A possibly executed attempt always requires reconciliation, including malformed output,
/// cancellation and an exhausted retry policy. Only definitive non-execution plus a transient
/// class can lead to a bounded retry. None with existing history also fails closed to reconcile.
pub fn decide_next_step(
    outcome: AttemptOutcome,
    attempt_count: u32,
    policy: &RetryPolicy,
) -> StepDecision {
    match outcome {
        AttemptOutcome::None => {
            if attempt_count == 0 {
                StepDecision::Dispatch
            } else {
                StepDecision::Reconcile
            }
        }
        AttemptOutcome::Unresolved => StepDecision::Reconcile,
        AttemptOutcome::Output => StepDecision::Accept,
        AttemptOutcome::Failure { class, delivery } => match delivery {
            DeliveryState::MayHaveExecuted => StepDecision::Reconcile,
            DeliveryState::NotSent | DeliveryState::ConfirmedNotExecuted => match class {
                FailureClass::Timeout | FailureClass::RateLimit => {
                    if attempt_count == 0 || attempt_count >= policy.max_attempts() {
                        StepDecision::Fail
                    } else {
                        StepDecision::RetryAfter(policy.backoff_seconds(attempt_count))
                    }
                }
                FailureClass::UnsupportedVoice
                | FailureClass::UnsupportedControl
                | FailureClass::Permanent
                | FailureClass::MalformedAudio
                | FailureClass::Cancelled => StepDecision::Fail,
            },
        },
    }
}
