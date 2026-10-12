//! Ingestion: bounded, deterministic extraction of private manuscripts into reviewable blocks
//! and their immutable import receipts in the source registry.
//!
//! Downstream of editorial, whose evidence registry records each imported source.
mod application;
pub mod domain;
mod postgres;

pub use domain::extraction::{
    extract, EXTRACTOR_VERSION, MAX_BLOCKS, MAX_DECODED_BYTES, MAX_SOURCE_BYTES, MAX_XML_DEPTH,
    MAX_XML_EVENTS, MAX_ZIP_ENTRIES, MAX_ZIP_RATIO,
};
pub(crate) use postgres::imported_source;
