//! Bounded retry, stable operation jitter and lease configuration.
use super::JobsError;
use sha2::{Digest, Sha256};

pub const MAX_ATTEMPTS: u32 = 8;
pub const MAX_LEASE_SECONDS: u32 = 300;
pub const MAX_BACKOFF_SECONDS: u32 = 86_400;

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
