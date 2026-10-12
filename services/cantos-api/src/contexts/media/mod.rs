//! Admission and inspection of externally produced audio; no synthesis engines.
pub mod domain;

pub mod audio_intake;
mod application;
mod postgres;
mod process;
mod structure;
pub(crate) mod storage;
