//! Private audio storage capability used by intake and authorized Studio reads.
//! Provider details and transport failures are translated by concrete adapters.

use std::{fmt, future::Future, pin::Pin};

use bytes::Bytes;
use futures_util::Stream;

use crate::contexts::media::{
    audio_intake::VerifiedAudio,
    domain::object_storage::{NormalizedByteRange, ObjectIntegrity, ObjectKey, StorageError},
};

pub type StorageFuture<T> = Pin<Box<dyn Future<Output = Result<T, StorageError>> + Send + 'static>>;
pub type StorageStream = Pin<Box<dyn Stream<Item = Result<Bytes, StorageError>> + Send + 'static>>;

/// A bounded stream of the exact verified object, or its normalized byte range.
pub struct StorageRead {
    pub stream: StorageStream,
    pub byte_length: u64,
}

/// Transient bearer capability. Never persist or log the URL, nor use it as asset identity.
pub struct TemporaryRead {
    url: String,
    expires_at: u64,
}

impl TemporaryRead {
    pub fn new(url: String, expires_at: u64) -> Result<Self, StorageError> {
        let parsed = url::Url::parse(&url).map_err(|_| StorageError::Unavailable)?;
        if parsed.scheme() != "https"
            || url
                .bytes()
                .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
            || expires_at == 0
        {
            return Err(StorageError::Unavailable);
        }
        Ok(Self { url, expires_at })
    }

    pub fn into_parts(self) -> (String, u64) {
        (self.url, self.expires_at)
    }
}

impl fmt::Debug for TemporaryRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TemporaryRead")
            .field("url", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// No deletion, mutable overwrite, public promotion or caller-selected storage keys exist.
pub trait PrivateStorage: Send + Sync {
    fn put_if_absent(&self, audio: VerifiedAudio) -> StorageFuture<()>;

    fn verify(&self, key: ObjectKey, integrity: ObjectIntegrity) -> StorageFuture<()>;

    fn open_verified(
        &self,
        key: ObjectKey,
        integrity: ObjectIntegrity,
        range: Option<NormalizedByteRange>,
    ) -> StorageFuture<StorageRead>;

    /// An unsupported direct grant keeps delivery on the protected API stream.
    fn temporary_read(
        &self,
        _key: ObjectKey,
        _integrity: ObjectIntegrity,
        _ttl: u32,
    ) -> StorageFuture<Option<TemporaryRead>> {
        Box::pin(async { Ok(None) })
    }
}

#[cfg(test)]
mod tests {
    use super::{StorageError, TemporaryRead};

    #[test]
    fn temporary_read_is_https_only_and_debug_never_contains_the_bearer_url() {
        let secret = "https://storage.example/private?X-Amz-Signature=sentinel-secret";
        let grant = TemporaryRead::new(secret.into(), 100).unwrap();
        let debug = format!("{grant:?}");
        assert!(!debug.contains("storage.example"));
        assert!(!debug.contains("sentinel-secret"));
        assert!(debug.contains("[REDACTED]"));
        assert_eq!(grant.into_parts(), (secret.into(), 100));
        for url in [
            "http://storage.example/private",
            "https://user:secret@storage.example/private",
            "https://storage.example/private#fragment",
            "\nhttps://storage.example/private",
            "not-a-url",
        ] {
            assert!(matches!(
                TemporaryRead::new(url.into(), 100),
                Err(StorageError::Unavailable)
            ));
        }
        assert!(matches!(
            TemporaryRead::new(secret.into(), 0),
            Err(StorageError::Unavailable)
        ));
    }
}
