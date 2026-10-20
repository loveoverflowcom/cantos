//! Storage-compatible positive security generations; exhaustion must stop a mutation.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityVersionError {
    OutsideStorageRange,
    Exhausted,
}

impl fmt::Display for SecurityVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OutsideStorageRange => "security version is outside the storage range",
            Self::Exhausted => "security version is exhausted",
        })
    }
}

impl std::error::Error for SecurityVersionError {}

/// Positive generation that fits a Turso INTEGER; no unchecked deserialization or Default.
///
/// ```compile_fail
/// use cantos_identity::domain::security_version::SecurityVersion;
/// let forged = SecurityVersion(0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecurityVersion(u64);

impl SecurityVersion {
    pub const fn new(value: u64) -> Result<Self, SecurityVersionError> {
        if value == 0 || value > i64::MAX as u64 {
            return Err(SecurityVersionError::OutsideStorageRange);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    /// Plan the next generation. The shell must persist it with the change and event atomically.
    pub const fn successor(self) -> Result<Self, SecurityVersionError> {
        if self.0 == i64::MAX as u64 {
            return Err(SecurityVersionError::Exhausted);
        }
        Ok(Self(self.0 + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::{SecurityVersion, SecurityVersionError};

    #[test]
    fn raw_storage_boundaries_have_exact_errors() {
        for raw in [0, i64::MAX as u64 + 1, u64::MAX] {
            assert_eq!(
                SecurityVersion::new(raw),
                Err(SecurityVersionError::OutsideStorageRange)
            );
        }
        let first = SecurityVersion::new(1).unwrap();
        assert_eq!(first.successor().unwrap().get(), 2);
        let last = SecurityVersion::new(i64::MAX as u64).unwrap();
        assert_eq!(last.successor(), Err(SecurityVersionError::Exhausted));
        assert_eq!(last.get(), i64::MAX as u64);
    }

    #[test]
    fn successors_match_wider_arithmetic_at_every_power_of_two_and_storage_edge() {
        let edges = [1, 2, i64::MAX as u64 - 1, i64::MAX as u64];
        let powers = (1..63).flat_map(|bit| {
            let power = 1_u64 << bit;
            [power - 1, power, power + 1]
        });
        for raw in edges.into_iter().chain(powers) {
            let version = SecurityVersion::new(raw).unwrap();
            let expected = u128::from(raw) + 1;
            if expected <= i64::MAX as u128 {
                assert_eq!(u128::from(version.successor().unwrap().get()), expected);
            } else {
                assert_eq!(version.successor(), Err(SecurityVersionError::Exhausted));
            }
            assert_eq!(version.get(), raw);
        }
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::{SecurityVersion, SecurityVersionError};

    /// Full symbolic u64, no assumptions/stubs/loops. Oracle uses wider u128 arithmetic.
    /// Calls production constructors/successor; unwind 2 keeps unwinding checks enabled.
    #[kani::proof]
    #[kani::unwind(2)]
    fn security_version_never_wraps_or_leaves_storage_range() {
        let raw: u64 = kani::any();
        let admitted = SecurityVersion::new(raw);
        let valid = raw >= 1 && raw <= 9_223_372_036_854_775_807;
        assert_eq!(admitted.is_ok(), valid);
        if let Ok(version) = admitted {
            assert_eq!(version.get(), raw);
            let expected = u128::from(raw) + 1;
            if expected <= 9_223_372_036_854_775_807 {
                assert_eq!(u128::from(version.successor().unwrap().get()), expected);
            } else {
                assert_eq!(version.successor(), Err(SecurityVersionError::Exhausted));
            }
            assert_eq!(version.get(), raw);
        } else {
            assert_eq!(admitted, Err(SecurityVersionError::OutsideStorageRange));
        }
        kani::cover!(raw == 0, "zero rejected");
        kani::cover!(raw == 1, "first generation");
        kani::cover!(raw == i64::MAX as u64 - 1, "last successful successor");
        kani::cover!(raw == i64::MAX as u64, "exhaustion");
        kani::cover!(raw == i64::MAX as u64 + 1, "storage overflow refused");
        kani::cover!(raw == u64::MAX, "machine maximum refused");
    }
}
