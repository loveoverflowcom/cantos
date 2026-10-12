//! Production guidance: casting, rights claims, planning estimates, frozen production inputs and
//! the owner approval that records them. Cantos dispatches no synthesis; an external producer
//! reads an approved snapshot and submits audio back through the media boundary.
//!
//! Downstream of editorial (script access, accepted revisions, reviews and rights evidence).
mod application;
pub mod domain;
mod postgres;

pub use domain::*;
