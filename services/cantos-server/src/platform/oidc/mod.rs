//! Shared crypto/transport only; purpose-specific sessions remain separate.
pub(crate) mod config;
pub(crate) mod core;
pub(crate) mod provider;
pub(crate) mod trust;

#[cfg(test)]
#[path = "../../../tests/support/oidc_provider.rs"]
pub(crate) mod mock;

#[cfg(test)]
mod tests;
