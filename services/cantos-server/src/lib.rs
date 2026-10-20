//! Modular Cantos backend: editorial ownership, source ingestion, host-created adaptations,
//! production guidance and inspection of externally produced media.
//! Neither this library nor the HTTP host executes text-to-speech or audio mixing.
pub mod adapters;
pub mod application;
pub mod contexts;
pub mod platform;
pub mod script_ir;

// Compatibility aliases for the retained editorial and metadata contracts.
#[doc(hidden)]
pub use adapters::inbound::http;
#[doc(hidden)]
pub use contexts::adaptation;
#[doc(hidden)]
pub use contexts::ingestion as imports;
#[doc(hidden)]
pub use contexts::media::domain::wav_inspection;
#[doc(hidden)]
pub use contexts::production;
#[doc(hidden)]
pub use platform::{incremental, operator};

#[doc(hidden)]
pub mod revisions {
    pub use crate::contexts::editorial::domain::revisions::*;
    pub use crate::contexts::editorial::{permits, Access, Action};
}

#[doc(hidden)]
pub mod postgres {
    pub use crate::platform::postgres::*;
}

// Reuse the canonical disposable Studio→Publication fixture in ignored Library unit/Router tests.
// This test-only alias creates no production authentication shortcut or feature flag.
#[cfg(test)]
extern crate self as cantos_server;
#[cfg(test)]
#[path = "../tests/catalog_http_postgres.rs"]
mod listener_postgres_test_fixture;

#[cfg(test)]
mod playback_transport_tests;
