// Application-level races and mappings are exercised through the production route in
// `playback_transport_tests.rs`; this module keeps the pure storage-error translation exact.
use super::*;
use crate::contexts::media::domain::object_storage::StorageCapability;

#[test]
fn object_failures_translate_to_exact_listener_errors() {
    for (error, expected) in [
        (StorageError::NotFound, ListenerError::RenditionUnavailable),
        (
            StorageError::UnsupportedCapability(StorageCapability::RangeRead),
            ListenerError::RenditionUnavailable,
        ),
        (StorageError::DigestMismatch, ListenerError::Corrupt),
        (StorageError::LengthMismatch, ListenerError::Corrupt),
        (StorageError::InvalidIntegrity, ListenerError::Corrupt),
        (StorageError::InvalidTarget, ListenerError::Unavailable),
        (StorageError::InvalidKey, ListenerError::Unavailable),
        (StorageError::Conflict, ListenerError::Unavailable),
        (StorageError::Forbidden, ListenerError::Unavailable),
        (StorageError::Timeout, ListenerError::Unavailable),
        (StorageError::Cancelled, ListenerError::Unavailable),
        (StorageError::Unavailable, ListenerError::Unavailable),
    ] {
        assert_eq!(storage_error(error), expected, "{error:?}");
    }
}
