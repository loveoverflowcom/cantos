//! Private verified-media storage capability used by intake and authorized Studio reads.
//! Provider details and transport failures are translated by concrete adapters.

use std::{fmt, future::Future, pin::Pin};

use bytes::Bytes;
use futures_util::Stream;

use crate::contexts::media::{
    audio_intake::VerifiedAudio,
    domain::object_storage::{NormalizedByteRange, ObjectIntegrity, ObjectKey, StorageError},
    image_intake::VerifiedImage,
};

pub type StorageFuture<T> = Pin<Box<dyn Future<Output = Result<T, StorageError>> + Send + 'static>>;
pub type StorageStream = Pin<Box<dyn Stream<Item = Result<Bytes, StorageError>> + Send + 'static>>;

/// Only actual-byte inspection witnesses may cross the private write boundary.
/// No raw bytes, caller-selected MIME or mutable metadata can construct a payload.
/// Image witnesses retain their admission lane until storage/readback finishes or is dropped.
///
/// ```compile_fail
/// use cantos_server::contexts::media::storage::port::VerifiedPayload;
/// let forged = VerifiedPayload::Image(vec![1, 2, 3]);
/// ```
pub enum VerifiedPayload {
    Audio(VerifiedAudio),
    Image(VerifiedImage),
}

impl VerifiedPayload {
    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Audio(audio) => audio.bytes(),
            Self::Image(image) => image.bytes(),
        }
    }

    pub fn sha256(&self) -> &str {
        match self {
            Self::Audio(audio) => &audio.metadata().sha256,
            Self::Image(image) => image.metadata().sha256(),
        }
    }

    pub fn byte_length(&self) -> u64 {
        match self {
            Self::Audio(audio) => audio.metadata().byte_length,
            Self::Image(image) => image.metadata().byte_length(),
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            // Audio admission accepts only the versioned Ogg/Opus delivery profile.
            Self::Audio(_) => "audio/ogg",
            Self::Image(image) => image.metadata().content_type().as_str(),
        }
    }
}

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
    fn put_if_absent(&self, payload: VerifiedPayload) -> StorageFuture<()>;

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
    use super::{StorageError, TemporaryRead, VerifiedPayload};
    use crate::contexts::media::{domain::image::ImageMime, image_intake::ImageAdmission};

    #[tokio::test]
    async fn image_payload_facts_come_from_complete_actual_byte_inspection() {
        for (bytes, mime, hash) in [
            (
                include_bytes!("../image_intake/fixtures/python-rgb-2x3.png").as_slice(),
                ImageMime::Png,
                "7b222c7332a2d0a325b482be5ff001918ff8470f52405b99d4c8351856bed276",
            ),
            (
                include_bytes!("../image_intake/fixtures/pillow-rgb-2x3.jpg").as_slice(),
                ImageMime::Jpeg,
                "a259cc25075434554da937e24a8056b5f9db0608a448c06effac4ba9c950eb46",
            ),
        ] {
            let image = ImageAdmission::from_test_lanes(std::sync::Arc::new(
                tokio::sync::Semaphore::new(1),
            ))
            .unwrap()
            .inspect(bytes.to_vec(), mime)
            .await
            .unwrap();
            let payload = VerifiedPayload::Image(image);
            assert_eq!(payload.bytes(), bytes);
            // Digests are independently recorded by the existing synthetic fixture oracle.
            assert_eq!(payload.sha256(), hash);
            assert_eq!(payload.byte_length(), bytes.len() as u64);
            assert_eq!(payload.content_type(), mime.as_str());
        }
    }

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
