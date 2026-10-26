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

#[cfg(any(target_arch = "wasm32", test))]
mod content_kind;
pub mod copy;
pub mod dashboard;
mod dashboard_copy;
#[cfg(any(target_arch = "wasm32", test))]
mod field_issues;

mod consent;
pub use consent::ReviewConsent;

pub mod catalog_controls;
pub mod drafts;
mod facts;
pub use facts::Facts;
mod pages;
pub use pages::ReadPages;

pub mod delegation;

pub mod brief_controls;
pub mod publication;
#[cfg(test)]
mod publication_commands_tests;

pub mod audio_preview;

pub mod ancillary;
