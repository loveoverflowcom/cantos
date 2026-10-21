//! Provider-neutral actor registry. A mapping is not authentication or authorization.
mod application;
pub mod domain;
mod ports;
mod postgres;
#[cfg(any(test, feature = "turso-candidate"))]
mod turso;

pub use application::IdentityService;
pub use ports::{IdentityRegistry, RegistryError};
pub use postgres::PostgresIdentityRegistry;
#[cfg(any(test, feature = "turso-candidate"))]
pub use turso::TursoIdentityRegistry;

#[cfg(test)]
mod tests;
