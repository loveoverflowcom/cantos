//! Playback is a real delivery consumer, separate from immutable catalog identity.
use super::listener_application::ListenerError;
use std::{future::Future, pin::Pin};
#[derive(Clone, Debug)]
pub struct PlaybackPermit {
    pub(super) publication_id: String,
    pub(super) rendition_id: String,
    pub(super) sha256: String,
    pub(super) object_id: String,
    pub(super) byte_length: u64,
    pub(super) valid_until: i64,
}
impl PlaybackPermit {
    pub fn publication_id(&self) -> &str {
        &self.publication_id
    }
    pub fn rendition_id(&self) -> &str {
        &self.rendition_id
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn publication_object_id(&self) -> &str {
        &self.object_id
    }
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }
    pub fn valid_until_ms(&self) -> i64 {
        self.valid_until
    }
}
/// Only trusted delivery adapters create grants after verifying the intended public object's
/// identity/bytes and range endpoint. URL Debug is deliberately redacted.
pub struct MediaGrant {
    pub(super) url: String,
    pub(super) expires: i64,
    pub(super) range: bool,
}
impl std::fmt::Debug for MediaGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaGrant")
            .field("url", &"[redacted]")
            .field("expires", &self.expires)
            .finish()
    }
}
impl MediaGrant {
    pub fn verified(
        url: String,
        expires_at_ms: i64,
        byte_range_supported: bool,
    ) -> Result<Self, ListenerError> {
        if !url.starts_with("https://")
            || url.len() > 4096
            || url.chars().any(|c| c.is_control() || c.is_whitespace())
            || expires_at_ms < 0
        {
            return Err(ListenerError::RenditionUnavailable);
        }
        Ok(Self {
            url,
            expires: expires_at_ms,
            range: byte_range_supported,
        })
    }
}
pub type PlaybackDeliveryFuture =
    Pin<Box<dyn Future<Output = Result<MediaGrant, ListenerError>> + Send + 'static>>;
/// The adapter must enforce current revocation/withdrawal on every media fetch, content-type
/// audio/ogg, cache isolation and signed Range semantics. A signer alone is insufficient.
pub trait PlaybackDelivery: Send + Sync {
    fn grant(&self, permit: PlaybackPermit) -> PlaybackDeliveryFuture;
}
pub(super) struct UnavailablePlayback;
impl PlaybackDelivery for UnavailablePlayback {
    fn grant(&self, _permit: PlaybackPermit) -> PlaybackDeliveryFuture {
        Box::pin(async { Err(ListenerError::RenditionUnavailable) })
    }
}
