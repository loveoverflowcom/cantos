//! Typed live Catalog/Content/Brief/private-audio/delegation client boundary.
pub mod artwork;
mod command;
mod state;
pub use command::*;
pub use state::*;
pub const DEFAULT_WORK_KIND: &str = "story";
#[cfg(target_arch = "wasm32")]
pub mod browser;
#[cfg(test)]
mod tests;
#[cfg(target_arch = "wasm32")]
pub mod workspace;

pub mod copy;
pub mod dashboard;
mod dashboard_copy;

mod consent;
pub use consent::ReviewConsent;

mod facts;
pub use facts::Facts;

pub mod delegation;
