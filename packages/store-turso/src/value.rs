//! Storage conventions every Cantos Turso schema shares.
//!
//! The engine stores `INTEGER` as a signed 64-bit value, so an unsigned domain number or a
//! timestamp is accepted only when it round-trips exactly. Nothing here wraps, saturates or
//! guesses.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueError {
    /// The value does not fit the signed 64-bit storage range.
    OutsideStorageRange,
    /// A stored integer that must be non-negative was negative.
    NegativeStored,
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OutsideStorageRange => "value is outside the signed 64-bit storage range",
            Self::NegativeStored => "stored integer is negative",
        })
    }
}

impl std::error::Error for ValueError {}

/// `u64` domain numbers (revisions, positions, byte counts) are stored as non-negative INTEGER.
pub const fn u64_to_integer(value: u64) -> Result<i64, ValueError> {
    if value > i64::MAX as u64 {
        return Err(ValueError::OutsideStorageRange);
    }
    Ok(value as i64)
}

/// Reading back a negative stored integer means a row was written outside the typed path.
pub const fn integer_to_u64(stored: i64) -> Result<u64, ValueError> {
    if stored < 0 {
        return Err(ValueError::NegativeStored);
    }
    Ok(stored as u64)
}

/// Microseconds since the Unix epoch (UTC), stored as INTEGER. Microseconds match the
/// resolution of the PostgreSQL `timestamptz` it replaces, and `i64` reaches year 294246.
///
/// ```compile_fail
/// use cantos_store_turso::value::UnixMicros;
/// let forged = UnixMicros(-1);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnixMicros(i64);

impl UnixMicros {
    pub const fn new(micros: i64) -> Result<Self, ValueError> {
        if micros < 0 {
            return Err(ValueError::NegativeStored);
        }
        Ok(Self(micros))
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    pub fn from_system_time(time: SystemTime) -> Result<Self, ValueError> {
        let since = time
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ValueError::NegativeStored)?;
        let micros =
            i64::try_from(since.as_micros()).map_err(|_| ValueError::OutsideStorageRange)?;
        Self::new(micros)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn unsigned_numbers_round_trip_only_inside_the_signed_range() {
        for raw in [0, 1, 1 << 32, i64::MAX as u64 - 1, i64::MAX as u64] {
            let stored = u64_to_integer(raw).unwrap();
            assert_eq!(integer_to_u64(stored), Ok(raw));
        }
        for raw in [i64::MAX as u64 + 1, 1 << 63, u64::MAX] {
            assert_eq!(u64_to_integer(raw), Err(ValueError::OutsideStorageRange));
        }
        for stored in [-1, i64::MIN] {
            assert_eq!(integer_to_u64(stored), Err(ValueError::NegativeStored));
        }
    }

    #[test]
    fn timestamps_reject_negative_and_pre_epoch_inputs() {
        assert_eq!(UnixMicros::new(0).unwrap().get(), 0);
        assert_eq!(UnixMicros::new(-1), Err(ValueError::NegativeStored));
        let after = UNIX_EPOCH + Duration::from_micros(1_700_000_000_123_456);
        assert_eq!(
            UnixMicros::from_system_time(after).unwrap().get(),
            1_700_000_000_123_456
        );
        assert_eq!(
            UnixMicros::from_system_time(UNIX_EPOCH - Duration::from_micros(1)),
            Err(ValueError::NegativeStored)
        );
    }
}
