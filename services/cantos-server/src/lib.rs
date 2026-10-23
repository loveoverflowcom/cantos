//! Modular Cantos backend: editorial ownership, source ingestion, host-created adaptations,
//! production guidance and inspection of externally produced media.
//! Neither this library nor the HTTP host executes text-to-speech or audio mixing.
pub mod adapters;
pub mod application;
pub mod contexts;
pub mod contract;
pub mod platform;
pub mod script_ir;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

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
pub use platform::operator;

#[doc(hidden)]
pub mod revisions {
    pub use crate::contexts::editorial::domain::revisions::*;
    pub use crate::contexts::editorial::{permits, Access, Action};
}

#[doc(hidden)]
pub mod store {
    pub use crate::platform::store::*;
}

#[cfg(test)]
mod playback_transport_tests;
#[cfg(test)]
mod theatre_library_http_tests;
