//! Admission and inspection of externally produced audio; no synthesis engines.
pub mod domain;

mod application;
pub mod audio_intake;
mod image_application;
pub mod image_intake;
pub use image_application::{
    ArtworkApplicationError, ArtworkReceipt, ArtworkService, ImageByteStream, ImageObjectError,
    ImageUploadTicket, PrivateImagePreview,
};
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
    #[cfg(test)]
    pub(crate) use super::domain::object_storage::StorageCapability;
    pub(crate) use super::domain::object_storage::{
        ByteRange, NormalizedByteRange, RangeError, StorageError,
    };
    pub(crate) use super::read_application::AudioReadScope;
    pub(crate) use super::storage::port::StorageStream;
}
mod publication_application;
pub(crate) mod publication_delivery;
pub use publication_application::{PublicationService, PublicationStoreError};
pub use publication_delivery::{
    PublicationDelivery, PublicationDeliveryFuture, PublicationDependencyError, PublicationObject,
};

mod listener_application;
mod listener_cursor;
mod listener_delivery;
pub use listener_application::{ListenerError, ListenerService};
pub use listener_delivery::{MediaGrant, PlaybackDelivery, PlaybackDeliveryFuture, PlaybackPermit};

pub(crate) use postgres::listener::{library_public_work, library_publications};
