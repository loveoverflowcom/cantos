//! Listener-only boundary. There is no supported listener resource on this backend yet.
#[cfg(target_arch = "wasm32")]
pub mod browser;
pub mod request;
pub use request::*;

#[cfg(target_arch = "wasm32")]
pub mod account;
