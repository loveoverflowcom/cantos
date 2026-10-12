//! Pure rules for the bounded local durable-worker foundation.
//! A completed run establishes execution only, not editorial or publication approval.
//! Fingerprints identify one entire frozen dialogue input; they are not clip-cache keys.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use cantos_contracts::ResolvedSynthesisInput;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::tts::attempt::{DeliveryState, FailureClass};
use crate::tts::local_contract;

pub const INPUT_FINGERPRINT_SCHEME: &str = "durable-input-v1";
pub const MAX_ATTEMPTS: u32 = 8;
pub const MAX_LEASE_SECONDS: u32 = 300;
pub const MAX_BACKOFF_SECONDS: u32 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Paused,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Paused => "paused",
            Self::Cancelled => "cancelled",
        }
    }
}

impl FromStr for RunStatus {
    type Err = JobsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "paused" => Ok(Self::Paused),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(JobsError::InvalidRunStatus),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEvent {
    Start,
    Complete,
    Pause,
    Resume,
    Fail,
    Retry,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionError {
    pub status: RunStatus,
    pub event: RunEvent,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid run transition: {:?} + {:?}",
            self.status, self.event
        )
    }
}

impl Error for TransitionError {}

/// The shell checks completion facts and scoped authorization before committing a transition.
/// Resume and explicit retry requeue work; a worker claim starts it under a new fence.
/// Completed and cancelled are terminal. A replacement is a separate run.
pub fn evolve_run(status: RunStatus, event: RunEvent) -> Result<RunStatus, TransitionError> {
    let next = match status {
        RunStatus::Queued => match event {
            RunEvent::Start => Some(RunStatus::Running),
            RunEvent::Pause => Some(RunStatus::Paused),
            RunEvent::Cancel => Some(RunStatus::Cancelled),
            RunEvent::Complete | RunEvent::Resume | RunEvent::Fail | RunEvent::Retry => None,
        },
        RunStatus::Running => match event {
            RunEvent::Complete => Some(RunStatus::Completed),
            RunEvent::Pause => Some(RunStatus::Paused),
            RunEvent::Fail => Some(RunStatus::Failed),
            RunEvent::Cancel => Some(RunStatus::Cancelled),
            RunEvent::Start | RunEvent::Resume | RunEvent::Retry => None,
        },
        RunStatus::Paused => match event {
            RunEvent::Resume => Some(RunStatus::Queued),
            RunEvent::Cancel => Some(RunStatus::Cancelled),
            RunEvent::Start
            | RunEvent::Complete
            | RunEvent::Pause
            | RunEvent::Fail
            | RunEvent::Retry => None,
        },
        RunStatus::Failed => match event {
            RunEvent::Retry => Some(RunStatus::Queued),
            RunEvent::Cancel => Some(RunStatus::Cancelled),
            RunEvent::Start
            | RunEvent::Complete
            | RunEvent::Pause
            | RunEvent::Resume
            | RunEvent::Fail => None,
        },
        RunStatus::Completed | RunStatus::Cancelled => match event {
            RunEvent::Start
            | RunEvent::Complete
            | RunEvent::Pause
            | RunEvent::Resume
            | RunEvent::Fail
            | RunEvent::Retry
            | RunEvent::Cancel => None,
        },
    };
    next.ok_or(TransitionError { status, event })
}

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

/// No Deserialize or public fields can bypass construction from raw configuration or SQL rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_attempts: u32,
    base_seconds: u32,
    cap_seconds: u32,
}

impl RetryPolicy {
    pub fn new(max_attempts: u32, base_seconds: u32, cap_seconds: u32) -> Result<Self, JobsError> {
        if !(1..=MAX_ATTEMPTS).contains(&max_attempts) {
            return Err(JobsError::InvalidMaxAttempts);
        }
        if !(1..=MAX_BACKOFF_SECONDS).contains(&base_seconds) {
            return Err(JobsError::InvalidBaseDelay);
        }
        if cap_seconds < base_seconds || cap_seconds > MAX_BACKOFF_SECONDS {
            return Err(JobsError::InvalidBackoffCap);
        }
        Ok(Self {
            max_attempts,
            base_seconds,
            cap_seconds,
        })
    }

    pub fn max_attempts(self) -> u32 {
        self.max_attempts
    }

    pub fn base_seconds(self) -> u32 {
        self.base_seconds
    }

    pub fn cap_seconds(self) -> u32 {
        self.cap_seconds
    }

    /// Deterministic exponential delay after an append-only attempt count. Saturating arithmetic
    /// keeps corrupt or exhausted counts bounded. The shell supplies the database clock.
    pub fn backoff_seconds(self, completed_attempts: u32) -> u32 {
        let exponent = completed_attempts.saturating_sub(1);
        let delay = 1_u32
            .checked_shl(exponent)
            .map_or(u32::MAX, |factor| self.base_seconds.saturating_mul(factor));
        delay.min(self.cap_seconds)
    }

    /// Equal jitter over ceil(ceiling / 2)..=ceiling, always at least one second.
    /// The shell creates an operation ID before recording immutable attempt intent and uses
    /// that stored ID on recovery. Hashing it supplies stable per-attempt variation without
    /// introducing a clock or random generator into the decision core.
    pub fn retry_delay_seconds(self, completed_attempts: u32, operation_id: &str) -> u32 {
        let ceiling = self.backoff_seconds(completed_attempts);
        let floor = (ceiling / 2 + ceiling % 2).max(1);
        let mut hash = Sha256::new();
        hash.update(b"cantos/durable-retry-jitter/v1\n");
        hash.update(operation_id.as_bytes());
        let sample = hash
            .finalize()
            .iter()
            .take(8)
            .fold(0_u64, |sample, byte| (sample << 8) | u64::from(*byte));
        let range = u64::from(ceiling - floor) + 1;
        floor + (sample % range) as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaseDuration(u32);

impl LeaseDuration {
    pub fn new(seconds: u32) -> Result<Self, JobsError> {
        if !(1..=MAX_LEASE_SECONDS).contains(&seconds) {
            return Err(JobsError::InvalidLeaseDuration);
        }
        Ok(Self(seconds))
    }

    pub fn seconds(self) -> u32 {
        self.0
    }
}

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

#[cfg(test)]
mod tests {
    use cantos_contracts::{ResolvedPronunciation, SynthesisPerformance};

    use super::*;

    fn local_input() -> ResolvedSynthesisInput {
        ResolvedSynthesisInput {
            dialogue_id: "dialogue-01".into(),
            character_id: "narrator".into(),
            text: "Ánh đèn cuối cùng còn sáng trên sân khấu.".into(),
            language: "vi".into(),
            provider_id: local_contract::PROVIDER_ID.into(),
            model_id: local_contract::MODEL_ID.into(),
            voice_id: "vi".into(),
            voice_rights_record_id: "voice-espeak".into(),
            adapter_version: local_contract::ADAPTER_VERSION.into(),
            output_contract: local_contract::OUTPUT_CONTRACT.into(),
            performance: SynthesisPerformance {
                rate_permille: 1_000,
                pitch_semitones: 0,
                emotion: Some("neutral".into()),
                intensity_permille: Some(0),
            },
            pronunciation: vec![ResolvedPronunciation {
                surface: "Vọng Đài".into(),
                replacement: "Vọng Đài".into(),
            }],
        }
    }

    #[test]
    fn exhaustive_run_transition_table() {
        use RunEvent::{Cancel, Complete, Fail, Pause, Resume, Retry, Start};
        use RunStatus::{Cancelled, Completed, Failed, Paused, Queued, Running};
        let events = [Start, Complete, Pause, Resume, Fail, Retry, Cancel];
        let cases = [
            (
                Queued,
                [
                    Some(Running),
                    None,
                    Some(Paused),
                    None,
                    None,
                    None,
                    Some(Cancelled),
                ],
            ),
            (
                Running,
                [
                    None,
                    Some(Completed),
                    Some(Paused),
                    None,
                    Some(Failed),
                    None,
                    Some(Cancelled),
                ],
            ),
            (Completed, [None, None, None, None, None, None, None]),
            (
                Failed,
                [None, None, None, None, None, Some(Queued), Some(Cancelled)],
            ),
            (
                Paused,
                [None, None, None, Some(Queued), None, None, Some(Cancelled)],
            ),
            (Cancelled, [None, None, None, None, None, None, None]),
        ];
        for (status, expected) in cases {
            for (event, next) in events.into_iter().zip(expected) {
                let result = match next {
                    Some(next) => Ok(next),
                    None => Err(TransitionError { status, event }),
                };
                assert_eq!(evolve_run(status, event), result, "{status:?} + {event:?}");
            }
        }
    }

    #[test]
    fn persisted_status_and_mode_are_exact() {
        for status in [
            RunStatus::Queued,
            RunStatus::Running,
            RunStatus::Completed,
            RunStatus::Failed,
            RunStatus::Paused,
            RunStatus::Cancelled,
        ] {
            assert_eq!(status.as_str().parse(), Ok(status));
            assert_eq!(
                serde_json::to_string(&status).unwrap(),
                format!("\"{}\"", status.as_str())
            );
        }
        assert_eq!(
            "waiting_review".parse::<RunStatus>(),
            Err(JobsError::InvalidRunStatus)
        );
        for mode in [ExecutionMode::LocalEspeak, ExecutionMode::SyntheticFake] {
            assert_eq!(mode.as_str().parse(), Ok(mode));
            assert_eq!(
                serde_json::to_string(&mode).unwrap(),
                format!("\"{}\"", mode.as_str())
            );
        }
        for mode in [
            "google",
            "vieneu",
            "fake",
            "reference-only",
            "local_espeak ",
        ] {
            assert_eq!(
                mode.parse::<ExecutionMode>(),
                Err(JobsError::InvalidExecutionMode)
            );
        }
    }

    #[test]
    fn configuration_construction_rejects_exact_invalid_partitions() {
        for attempts in [0, 9, u32::MAX] {
            assert_eq!(
                RetryPolicy::new(attempts, 1, 30),
                Err(JobsError::InvalidMaxAttempts)
            );
        }
        for base in [0, MAX_BACKOFF_SECONDS + 1, u32::MAX] {
            assert_eq!(
                RetryPolicy::new(3, base, MAX_BACKOFF_SECONDS),
                Err(JobsError::InvalidBaseDelay)
            );
        }
        assert_eq!(RetryPolicy::new(3, 2, 1), Err(JobsError::InvalidBackoffCap));
        for cap in [MAX_BACKOFF_SECONDS + 1, u32::MAX] {
            assert_eq!(
                RetryPolicy::new(3, 1, cap),
                Err(JobsError::InvalidBackoffCap)
            );
        }
        let upper_bound = RetryPolicy::new(8, MAX_BACKOFF_SECONDS, MAX_BACKOFF_SECONDS).unwrap();
        assert_eq!(upper_bound.base_seconds(), MAX_BACKOFF_SECONDS);
        assert_eq!(upper_bound.cap_seconds(), MAX_BACKOFF_SECONDS);
        for attempts in 1..=MAX_ATTEMPTS {
            let policy = RetryPolicy::new(attempts, 1, 30).unwrap();
            assert_eq!(policy.max_attempts(), attempts);
            assert_eq!(policy.base_seconds(), 1);
            assert_eq!(policy.cap_seconds(), 30);
        }
        for seconds in [0, 301, u32::MAX] {
            assert_eq!(
                LeaseDuration::new(seconds),
                Err(JobsError::InvalidLeaseDuration)
            );
        }
        for seconds in 1..=MAX_LEASE_SECONDS {
            assert_eq!(LeaseDuration::new(seconds).unwrap().seconds(), seconds);
        }
    }

    #[test]
    fn exponential_backoff_is_deterministic_bounded_and_overflow_safe() {
        let policy = RetryPolicy::new(8, 3, 30).unwrap();
        let expected = [3, 3, 6, 12, 24, 30, 30, 30, 30];
        for (attempt, expected) in expected.into_iter().enumerate() {
            assert_eq!(policy.backoff_seconds(attempt as u32), expected);
        }
        for base in [1, 3, 30, MAX_BACKOFF_SECONDS / 2, MAX_BACKOFF_SECONDS] {
            for cap in [base, MAX_BACKOFF_SECONDS] {
                let policy = RetryPolicy::new(8, base, cap).unwrap();
                let mut previous = 0;
                for attempt in (0..=64).chain([u32::MAX]) {
                    let delay = policy.backoff_seconds(attempt);
                    assert!(delay >= base && delay <= cap);
                    assert!(delay >= previous);
                    assert_eq!(delay, policy.backoff_seconds(attempt));
                    previous = delay;
                }
            }
        }
    }

    #[test]
    fn retry_jitter_is_stable_bounded_and_varies_across_recorded_operation_ids() {
        let policy = RetryPolicy::new(8, 4, 120).unwrap();
        for attempt in 1..=MAX_ATTEMPTS {
            let ceiling = policy.backoff_seconds(attempt);
            let floor = ceiling / 2 + ceiling % 2;
            let mut observed = std::collections::BTreeSet::new();
            for seed in 0..=1_000 {
                let operation_id = format!("00000000-0000-4000-8000-{seed:012x}");
                let delay = policy.retry_delay_seconds(attempt, &operation_id);
                assert!(delay >= floor && delay <= ceiling);
                assert_eq!(delay, policy.retry_delay_seconds(attempt, &operation_id));
                observed.insert(delay);
            }
            // Exercise the complete integer jitter range, including each endpoint.
            assert_eq!(
                observed,
                (floor..=ceiling).collect::<std::collections::BTreeSet<_>>()
            );
        }
        for base in [1, 3, MAX_BACKOFF_SECONDS / 2, MAX_BACKOFF_SECONDS] {
            let policy = RetryPolicy::new(8, base, MAX_BACKOFF_SECONDS).unwrap();
            for attempt in [0, 1, 8, 32, u32::MAX] {
                let ceiling = policy.backoff_seconds(attempt);
                let floor = (ceiling / 2 + ceiling % 2).max(1);
                let delay = policy.retry_delay_seconds(attempt, "fixed-recorded-operation");
                assert!(delay >= floor && delay <= ceiling);
            }
        }
        let one_second = RetryPolicy::new(8, 1, 1).unwrap();
        assert_eq!(one_second.retry_delay_seconds(1, "first-operation"), 1);
        assert_eq!(one_second.retry_delay_seconds(8, "last-operation"), 1);
    }

    #[test]
    fn exhaustive_failure_delivery_count_and_limit_decisions() {
        let classes = [
            FailureClass::Timeout,
            FailureClass::RateLimit,
            FailureClass::UnsupportedVoice,
            FailureClass::UnsupportedControl,
            FailureClass::Permanent,
            FailureClass::MalformedAudio,
            FailureClass::Cancelled,
        ];
        let deliveries = [
            DeliveryState::NotSent,
            DeliveryState::ConfirmedNotExecuted,
            DeliveryState::MayHaveExecuted,
        ];
        for limit in 1..=MAX_ATTEMPTS {
            let policy = RetryPolicy::new(limit, 1, 30).unwrap();
            for attempt_count in 0..=MAX_ATTEMPTS + 1 {
                for class in classes {
                    for delivery in deliveries {
                        let expected = if delivery == DeliveryState::MayHaveExecuted {
                            StepDecision::Reconcile
                        } else if [FailureClass::Timeout, FailureClass::RateLimit].contains(&class)
                            && attempt_count > 0
                            && attempt_count < limit
                        {
                            StepDecision::RetryAfter((1_u32 << (attempt_count - 1)).min(30))
                        } else {
                            StepDecision::Fail
                        };
                        assert_eq!(
                            decide_next_step(
                                AttemptOutcome::Failure { class, delivery },
                                attempt_count,
                                &policy
                            ),
                            expected,
                            "{class:?}/{delivery:?}/{attempt_count}/{limit}"
                        );
                    }
                }
                assert_eq!(
                    decide_next_step(AttemptOutcome::Unresolved, attempt_count, &policy),
                    StepDecision::Reconcile
                );
                assert_eq!(
                    decide_next_step(AttemptOutcome::Output, attempt_count, &policy),
                    StepDecision::Accept
                );
                assert_eq!(
                    decide_next_step(AttemptOutcome::None, attempt_count, &policy),
                    if attempt_count == 0 {
                        StepDecision::Dispatch
                    } else {
                        StepDecision::Reconcile
                    }
                );
            }
        }
    }

    #[test]
    fn execution_modes_refuse_other_providers_and_mutated_local_contracts() {
        for mode in [ExecutionMode::LocalEspeak, ExecutionMode::SyntheticFake] {
            let input = local_input();
            assert_eq!(validate_execution_mode(mode, &input), Ok(()));
            for provider in [
                "google-cloud-tts",
                "vieneu-nano-local",
                "reference-only",
                "synthetic-fake",
            ] {
                let mut changed = input.clone();
                changed.provider_id = provider.into();
                assert_eq!(
                    validate_execution_mode(mode, &changed),
                    Err(JobsError::UnsupportedExecutionInput)
                );
            }
            let mutations: [fn(&mut ResolvedSynthesisInput); 11] = [
                |input| input.model_id.push('x'),
                |input| input.adapter_version.push('x'),
                |input| input.output_contract.push('x'),
                |input| input.voice_id = "unknown".into(),
                |input| input.language = "en".into(),
                |input| input.performance.rate_permille = 999,
                |input| input.performance.pitch_semitones = 1,
                |input| input.performance.emotion = None,
                |input| input.performance.intensity_permille = None,
                |input| input.text.clear(),
                |input| input.text.push_str("[[a]]"),
            ];
            for mutate in mutations {
                let mut changed = input.clone();
                mutate(&mut changed);
                assert_eq!(
                    validate_execution_mode(mode, &changed),
                    Err(JobsError::UnsupportedExecutionInput)
                );
            }
        }
    }

    #[test]
    fn fingerprint_covers_every_frozen_resolved_field() {
        let input = local_input();
        let fingerprint = input_fingerprint(&input).unwrap();
        assert_eq!(fingerprint, input_fingerprint(&input).unwrap());
        // Independently encoded UTF-8 JSON plus SHA-256, rather than a serializer round trip.
        assert_eq!(
            fingerprint,
            "durable-input-v1:sha256:57e1ca6c02e866d1e712f7fb14da63f7b07ae781e9b999d219c14c0bc67b0d6e"
        );
        let mutations: [fn(&mut ResolvedSynthesisInput); 17] = [
            |input| input.dialogue_id.push('x'),
            |input| input.character_id.push('x'),
            |input| input.text.push('x'),
            |input| input.language.push('x'),
            |input| input.provider_id.push('x'),
            |input| input.model_id.push('x'),
            |input| input.voice_id.push('x'),
            |input| input.voice_rights_record_id.push('x'),
            |input| input.adapter_version.push('x'),
            |input| input.output_contract.push('x'),
            |input| input.performance.rate_permille = 999,
            |input| input.performance.pitch_semitones = 1,
            |input| input.performance.emotion = None,
            |input| input.performance.intensity_permille = None,
            |input| input.pronunciation[0].surface.push('x'),
            |input| input.pronunciation[0].replacement.push('x'),
            |input| input.pronunciation.clear(),
        ];
        for mutate in mutations {
            let mut changed = input.clone();
            mutate(&mut changed);
            assert_ne!(input_fingerprint(&changed).unwrap(), fingerprint);
        }
    }
}
