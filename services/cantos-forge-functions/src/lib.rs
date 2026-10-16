//! Cantos Forge: the producer-side CLI for local production records.
//!
//! Forge runs in the producer's environment, outside the Cantos API runtime. It never links
//! Cantos API implementation code or a product client; a future handoff uses HTTP only.

#![forbid(unsafe_code)]

pub mod adapters;
pub mod application;
pub mod domain;
pub mod interfaces;
pub mod wire;
