//! The trusted time a write transaction receives. SQL never reads the wall clock: PostgreSQL's
//! ~150 `clock_timestamp()`/`now()` calls become one value chosen by the shell and passed in.

use std::fmt;
use std::time::SystemTime;

use crate::value::UnixMicros;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockFault;

impl fmt::Display for ClockFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("trusted clock is unavailable or outside the storage range")
    }
}

impl std::error::Error for ClockFault {}

pub trait Clock: Send + Sync {
    /// Fails closed: a transaction that cannot be timestamped does not begin.
    fn now(&self) -> Result<UnixMicros, ClockFault>;
}

/// The host wall clock. Monotonic semantics a context needs (for example `catalog_clock`) are
/// that context's decision over this value, not a property of the clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Result<UnixMicros, ClockFault> {
        UnixMicros::from_system_time(SystemTime::now()).map_err(|_| ClockFault)
    }
}
