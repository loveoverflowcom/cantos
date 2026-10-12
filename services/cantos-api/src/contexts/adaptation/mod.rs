//! Adaptation: caller-owned proposals that turn an imported manuscript into a Script IR
//! revision only after explicit review. Cantos prepares inert requests and admits output; it
//! performs no inference.
//!
//! Downstream of ingestion (the pinned source) and editorial (base revision, evidence records
//! and the revision an acceptance appends).
mod application;
pub mod domain;
mod postgres;

pub use application::admit_output;
pub use domain::*;
