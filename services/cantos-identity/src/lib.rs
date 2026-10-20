//! Security-state and mailbox-proof plans for the native Identity owner.
//!
//! This library does not verify credentials, mint authentication facts or issue sessions.
//! The optional `persistence` shell stores security state in one owned Turso file. The default
//! core remains dependency-free; credentials, authentication facts and private RPC are absent.

#![forbid(unsafe_code)]

pub mod domain;

#[cfg(feature = "persistence")]
pub mod persistence;
