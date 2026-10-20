//! Origin-mode media fetch use cases (proposed ADR 0032). Every GET/HEAD re-resolves current
//! public eligibility and authenticates its grant against that exact identity; a byte response
//! repeats both after object-open I/O. HTTP planning and headers stay in the transport adapter.
use super::{
    domain::{
        object_storage::{NormalizedByteRange, StorageError},
        playback_grant::{decide_media_fetch, CurrentRelease, GrantClaims, MediaFetchFacts},
    },
    listener_application::{permit_for, ListenerError, ListenerService},
    listener_delivery::{PlaybackPermit, PublicationObjectRef},
    storage::port::StorageRead,
};
use tokio::sync::OwnedSemaphorePermit;

/// A grant admitted for one fetch at one instant. It is not reusable authority: opening bytes
/// re-admits the same token and requires the identical permit.
pub struct AuthorizedMedia {
    token: String,
    permit: PlaybackPermit,
}
impl AuthorizedMedia {
    pub fn sha256(&self) -> &str {
        self.permit.sha256()
    }
    pub fn byte_length(&self) -> u64 {
        self.permit.byte_length()
    }
}

impl ListenerService {
    /// Bounded concurrent media responses; a held lane travels with the response body.
    pub fn media_lane(&self) -> Result<OwnedSemaphorePermit, ListenerError> {
        self.media_lanes
            .clone()
            .try_acquire_owned()
            .map_err(|_| ListenerError::Unavailable)
    }

    /// Every denial, including an unconfigured host, is the same anonymous `not_found`.
    pub async fn authorize_media(&self, token: &str) -> Result<AuthorizedMedia, ListenerError> {
        let origin = self.media.as_ref().ok_or(ListenerError::NotFound)?;
        let claims = GrantClaims::parse(token).map_err(|_| ListenerError::NotFound)?;
        let (item, now) = self.current(claims.publication_id()).await?;
        let permit = item
            .as_ref()
            .map(|item| permit_for(&item.publication, claims.expires_at_ms()));
        let current = item.as_ref().map(|item| {
            let document = item.publication.release().document();
            CurrentRelease {
                published_at_ms: document.published_at,
                rights_valid_until_ms: document.manifest.distribution_clearance.request.valid_until,
            }
        });
        let authenticated = permit
            .as_ref()
            .is_some_and(|permit| claims.authenticates(origin.key(), &permit.subject()));
        decide_media_fetch(MediaFetchFacts {
            now_ms: now,
            expires_at_ms: claims.expires_at_ms(),
            current,
            authenticated,
        })
        .map_err(|_| ListenerError::NotFound)?;
        Ok(AuthorizedMedia {
            token: token.to_owned(),
            permit: permit.ok_or(ListenerError::NotFound)?,
        })
    }

    /// Open the exact publication object, then re-admit the grant from a fresh read. A stream
    /// opened before withdrawal, expiry or supersession is dropped unread.
    pub async fn open_media(
        &self,
        authorized: &AuthorizedMedia,
        range: Option<NormalizedByteRange>,
    ) -> Result<StorageRead, ListenerError> {
        let origin = self.media.as_ref().ok_or(ListenerError::NotFound)?;
        let read = origin
            .objects()
            .open(PublicationObjectRef::for_permit(&authorized.permit), range)
            .await
            .map_err(storage_error)?;
        let again = self.authorize_media(&authorized.token).await?;
        if again.permit != authorized.permit {
            return Err(ListenerError::NotFound);
        }
        Ok(read)
    }
}

/// An eligible release whose promoted object cannot be read is a retryable delivery failure;
/// bytes that contradict the sealed identity are corruption, never a substitute rendition.
fn storage_error(error: StorageError) -> ListenerError {
    match error {
        StorageError::NotFound | StorageError::UnsupportedCapability(_) => {
            ListenerError::RenditionUnavailable
        }
        StorageError::DigestMismatch
        | StorageError::LengthMismatch
        | StorageError::InvalidIntegrity => ListenerError::Corrupt,
        StorageError::InvalidTarget
        | StorageError::InvalidKey
        | StorageError::Conflict
        | StorageError::Forbidden
        | StorageError::Timeout
        | StorageError::Cancelled
        | StorageError::Unavailable => ListenerError::Unavailable,
    }
}

#[cfg(test)]
#[path = "listener_media_tests.rs"]
mod tests;
