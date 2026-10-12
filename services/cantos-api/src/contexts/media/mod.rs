//! Admission and inspection of externally produced audio; no synthesis engines.
pub mod domain;

mod application;
pub mod audio_intake;
mod postgres;
mod process;
pub(crate) mod storage;
mod structure;
