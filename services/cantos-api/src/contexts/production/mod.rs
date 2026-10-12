//! Production: casting, rights claims, planning estimates, frozen production inputs and the
//! owner approval that authorizes their use. No synthesis is dispatched here.
//!
//! Downstream of editorial (script access, accepted revisions, reviews and rights evidence);
//! synthesis runs only under a snapshot this context reports as currently approved.
mod application;
pub mod domain;
mod postgres;

pub(crate) use application::{current_snapshot, production_actor};
pub use domain::*;
pub(crate) use postgres::stored_document;
