//! Pure, dependency-free security-state and mailbox-proof plans for the native Identity owner.
//!
//! This library does not verify credentials, mint authentication facts or issue sessions.
//! Storage, cryptographic verifiers, trusted clocks and private transport remain shell work.

#![forbid(unsafe_code)]

pub mod domain;
