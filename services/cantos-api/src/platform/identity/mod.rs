//! Provider-neutral actor registry. A mapping is not authentication or authorization.
mod application;
pub mod domain;
mod ports;
mod postgres;

pub use application::IdentityService;
pub use ports::{IdentityRegistry, RegistryError};
pub use postgres::PostgresIdentityRegistry;

#[cfg(test)]
mod tests;
