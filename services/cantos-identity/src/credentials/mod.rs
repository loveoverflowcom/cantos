//! Password hashing and verification with RustCrypto Argon2id, run on bounded workers.

pub mod hasher;
pub mod pool;

pub use hasher::{hash_password, verify_password, KdfFault, Verification};
pub use pool::{KdfPool, PoolFault};
