//! Every finite limit the executable enforces, in one reviewed table. Nothing here is read
//! from the environment or unbounded: a deployment cannot silently weaken a security floor.
//! Values marked NEW in the README decision table need owner review before production use.

use std::time::Duration;

// ACCEPTANCE-PENDING(owner review): every value marked NEW in the README policy table is a
// conservative qualification choice, not an accepted product or security policy.

/// Lifetime of a mailed signup proof. Shorter limits the window a leaked link is useful.
pub const PROOF_TTL_SECONDS: u64 = 900;
/// Minimum time after a proof's issue before the same operation may be re-mailed.
pub const RESEND_COOLDOWN_SECONDS: u32 = 60;

// Argon2id: OWASP password-storage minimum m=19 MiB, t=2, p=1. A stored hash below these is
// refused rather than verified; above the ceilings it is refused before any KDF allocation.
pub const KDF_MEMORY_KIB: u32 = 19_456;
pub const KDF_ITERATIONS: u32 = 2;
pub const KDF_LANES: u32 = 1;
pub const KDF_OUTPUT_BYTES: usize = 32;
pub const KDF_SALT_BYTES: usize = 16;
pub const KDF_MAX_MEMORY_KIB: u32 = 65_536;
pub const KDF_MAX_ITERATIONS: u32 = 6;
pub const KDF_MAX_LANES: u32 = 4;

/// Concurrent hash/verify jobs. Peak KDF memory is `KDF_CONCURRENCY × KDF_MEMORY_KIB` KiB.
pub const KDF_CONCURRENCY: usize = 2;
/// Jobs allowed to wait for a permit beyond the running ones; more are refused at once.
pub const KDF_QUEUE: usize = 6;
pub const KDF_QUEUE_DEADLINE: Duration = Duration::from_secs(5);

pub const RPC_REQUEST_DEADLINE: Duration = Duration::from_secs(20);
pub const RPC_IN_FLIGHT: usize = 16;
pub const RPC_CONCURRENT_STREAMS: u32 = 8;
pub const SHUTDOWN_DRAIN: Duration = Duration::from_secs(10);
pub const SMTP_DEADLINE: Duration = Duration::from_secs(10);

/// Trusted caller allowlist ceiling: at most this many configured Server uids.
pub const MAX_CALLER_UIDS: usize = 8;

/// One counting rule: at most `limit` events per fixed `window_seconds` window per key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub limit: u32,
    pub window_seconds: u64,
}

/// Signup starts per mailbox. Counted per request, whether or not mail is sent.
pub const SIGNUP_PER_EMAIL: Rule = Rule {
    limit: 3,
    window_seconds: 3600,
};
/// Signup starts per trusted client address.
pub const SIGNUP_PER_IP: Rule = Rule {
    limit: 20,
    window_seconds: 3600,
};
/// Completion attempts per trusted client address.
pub const COMPLETE_PER_IP: Rule = Rule {
    limit: 20,
    window_seconds: 900,
};
/// Completion attempts per (signup operation, client address): bounds guessing against one
/// pending proof without letting the operation's initiator exhaust the mailbox owner's attempts.
pub const COMPLETE_PER_OPERATION: Rule = Rule {
    limit: 10,
    window_seconds: 900,
};
/// Failed logins for one account from one client address; cleared by a success from it.
pub const LOGIN_FAILURES_PER_PAIR: Rule = Rule {
    limit: 5,
    window_seconds: 900,
};
/// Login attempts from one client address across all accounts.
pub const LOGIN_PER_IP: Rule = Rule {
    limit: 60,
    window_seconds: 900,
};
/// Failed logins for one account from every address together. A temporary denial of that
/// account's logins is the accepted cost; there is never a permanent lock.
pub const LOGIN_FAILURES_PER_ACCOUNT: Rule = Rule {
    limit: 40,
    window_seconds: 900,
};

/// Distinct keys a limiter remembers. Beyond it the oldest window is evicted first.
pub const LIMITER_MAX_KEYS: usize = 10_000;
