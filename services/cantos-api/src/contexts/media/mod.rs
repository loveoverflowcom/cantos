//! Admission and inspection of externally produced audio; no synthesis engines.
pub mod domain;

mod application;
pub mod audio_intake;
mod postgres;
mod process;
pub mod storage;
mod structure;

mod lifecycle_application;
mod read_application;

pub(crate) use lifecycle_application::upload_token_registered;

/// Reviewed private-read interface for the HTTP shell: application scope, fallible range
/// admission, safe failure codes and the bounded byte transport. Authorization and storage
/// effects stay behind Store use cases; adapters and internal object locations are not exposed.
pub(crate) mod read {
    pub(crate) use super::domain::object_storage::{
        ByteRange, NormalizedByteRange, RangeError, StorageError,
    };
    pub(crate) use super::read_application::AudioReadScope;
    pub(crate) use super::storage::port::StorageStream;
}
