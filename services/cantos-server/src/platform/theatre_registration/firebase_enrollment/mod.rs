//! Default-disabled Firebase proof and retained Kanidm enrollment; never session admission.
pub mod application;
pub mod config;
pub mod domain;
pub mod postgres;
pub mod runtime;
#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
mod emulator_tests;
