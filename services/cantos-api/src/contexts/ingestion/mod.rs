//! Ingestion: bounded, deterministic extraction of private manuscripts into reviewable blocks
//! and their immutable import receipts.
pub mod domain;
mod postgres;

pub use domain::extraction::*;
pub(crate) use postgres::{decode_import, IMPORT_COLUMNS};
