//! Provider-neutral actor registry. A mapping is not authentication or authorization.
mod application;
pub mod domain;
mod ports;
pub(crate) mod turso;

pub use application::IdentityService;
pub use ports::{IdentityRegistry, RegistryError};
pub use turso::TursoIdentityRegistry;

#[cfg(test)]
mod tests;
