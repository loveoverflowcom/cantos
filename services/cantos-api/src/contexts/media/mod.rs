//! Admission and inspection of externally produced audio; no synthesis engines.
pub mod domain;

mod application;
pub mod audio_intake;
mod postgres;
mod process;
pub mod storage;
mod structure;

mod lifecycle_application;

pub(crate) use lifecycle_application::upload_token_registered;
