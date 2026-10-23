//! Security-state and mailbox-proof plans for the native Identity owner.
//!
//! This library does not verify credentials, mint authentication facts or issue sessions.
//! The optional `persistence` shell stores security state in one owned Turso file. The default
//! core remains dependency-free; credentials, authentication facts and private RPC are absent.

// ACCEPTANCE-PENDING(D1 Kani): the three registered kernels below this crate's `domain` module
// have never run on the pinned Ubuntu x86_64 runner; this work changes none of them.

#![forbid(unsafe_code)]

pub mod domain;

#[cfg(feature = "persistence")]
pub mod persistence;

#[cfg(feature = "service")]
pub mod application;
#[cfg(feature = "service")]
pub mod config;
#[cfg(feature = "service")]
pub mod credentials;
#[cfg(feature = "service")]
pub mod events;
#[cfg(feature = "service")]
pub mod firebase;
#[cfg(feature = "service")]
pub mod limits;
#[cfg(feature = "service")]
pub mod mail;
#[cfg(feature = "service")]
pub mod policy;
#[cfg(feature = "service")]
pub mod rpc;
#[cfg(feature = "service")]
pub mod server;
#[cfg(feature = "service")]
pub mod verification;

// Test-only event observer of the pure cores for the registered Quint capability. It lives
// outside `domain` because it reads files and the environment; no release build links it.
#[cfg(test)]
mod quint_conformance_tests;
