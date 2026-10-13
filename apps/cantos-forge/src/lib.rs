//! Cantos Forge: the producer-side CLI for local production records.
//!
//! Forge runs in the producer's environment, outside the Cantos API runtime. It never links
//! `services/` code or another app; a future handoff talks to the API over HTTP only.

#![forbid(unsafe_code)]

pub mod domain;
pub mod interfaces;
