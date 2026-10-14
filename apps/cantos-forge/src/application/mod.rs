//! Use cases: register, validate, inspect and prepare an export, over ports.
//!
//! Each use case reads recorded facts through a [`ports::Store`], asks the pure domain what the
//! facts mean and persists only what the domain allowed. Nothing here touches the filesystem,
//! the clock, randomness or serialization; adapters implement the ports and the CLI composes
//! them. A port exists because it has a real second implementation: the filesystem adapter and
//! the in-memory fake the tests use.

pub mod export;
pub mod ports;
pub mod register;
pub mod validate;
pub mod view;

#[cfg(test)]
pub(crate) mod memory;

#[cfg(test)]
mod tests;
