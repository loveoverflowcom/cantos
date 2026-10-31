//! Playback is a real delivery consumer, separate from immutable catalog identity.
use super::{
    domain::{
        object_storage::NormalizedByteRange,
        playback_grant::{seal, GrantKey, GrantSubject},
    },
    listener_application::ListenerError,
    storage::port::{StorageFuture, StorageRead},
};
use crate::contract::routes::THEATRE_BASE;
use std::{fmt, future::Future, pin::Pin, sync::Arc};
#[derive(Clone, PartialEq, Eq)]
pub struct PlaybackPermit {
    pub(super) publication_id: String,
    pub(super) rendition_id: String,
    pub(super) manifest_digest: String,
    pub(super) sha256: String,
    pub(super) object_id: String,
    pub(super) byte_length: u64,
    pub(super) valid_until: i64,
    pub(super) access_scope: Option<String>,
}
impl fmt::Debug for PlaybackPermit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaybackPermit")
            .field("publication_id", &self.publication_id)
            .field("valid_until", &self.valid_until)
            .field("object_id", &"[redacted]")
            .field("access_scope", &"[redacted]")
            .finish()
    }
}
impl PlaybackPermit {
    pub fn publication_id(&self) -> &str {
        &self.publication_id
    }
    pub fn rendition_id(&self) -> &str {
        &self.rendition_id
    }
    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
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
    pub(super) fn subject(&self) -> GrantSubject<'_> {
        GrantSubject {
            publication_id: &self.publication_id,
            rendition_id: &self.rendition_id,
            manifest_digest: &self.manifest_digest,
            sha256: &self.sha256,
            object_id: &self.object_id,
            byte_length: self.byte_length,
            access_scope: self.access_scope.as_deref(),
        }
    }
}
/// Only trusted delivery adapters create grants after verifying the intended publication object's
/// identity/bytes and range endpoint. URL Debug is deliberately redacted.
pub struct MediaGrant {
    pub(super) permit: PlaybackPermit,
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
    /// Bind the result to the exact sealed publication permit, including its manifest version.
    /// This validates adapter output, not the storage bytes or current serving eligibility.
    pub fn verified_for(
        permit: &PlaybackPermit,
        url: String,
        expires_at_ms: i64,
        byte_range_supported: bool,
    ) -> Result<Self, ListenerError> {
        let parsed = url::Url::parse(&url).map_err(|_| ListenerError::RenditionUnavailable)?;
        if !url.starts_with("https://")
            || url.contains('\\')
            || parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
            || url.len() > 4096
            || url.chars().any(|c| c.is_control() || c.is_whitespace())
            || expires_at_ms < 0
            || expires_at_ms > permit.valid_until
            || !byte_range_supported
        {
            return Err(ListenerError::RenditionUnavailable);
        }
        Ok(Self {
            permit: permit.clone(),
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
/// `OriginPlayback` is the only non-test implementation; it satisfies these obligations only
/// together with the Theatre media route, which rechecks eligibility before and after reads.
/// The host opts in only with separate publication storage, an explicit HTTPS origin and
/// an operator-owned key. Local acceptance is separate from deployed TLS/proxy/capacity gates
/// in ADR 0032. Never adapt PrivateStorage or a Studio preview grant into listener authority.
pub trait PlaybackDelivery: Send + Sync {
    fn grant(&self, permit: PlaybackPermit) -> PlaybackDeliveryFuture;
}
pub(super) struct UnavailablePlayback;
impl PlaybackDelivery for UnavailablePlayback {
    fn grant(&self, _permit: PlaybackPermit) -> PlaybackDeliveryFuture {
        Box::pin(async { Err(ListenerError::RenditionUnavailable) })
    }
}

/// HTTPS origin of this API host as listeners reach it; grant URLs are built on it only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicMediaOrigin(String);
impl PublicMediaOrigin {
    /// Exactly `https://host[:port]`: no path, query, fragment, credentials or trailing slash.
    pub fn parse(raw: &str) -> Result<Self, ListenerError> {
        let parsed = url::Url::parse(raw).map_err(|_| ListenerError::Unavailable)?;
        let origin = parsed.origin().ascii_serialization();
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || origin != raw
        {
            return Err(ListenerError::Unavailable);
        }
        Ok(Self(origin))
    }
    fn media_url(&self, token: &str, restricted: bool) -> String {
        let lane = if restricted { "shared/media" } else { "media" };
        format!("{}{THEATRE_BASE}/{lane}/{token}", self.0)
    }
    /// The accepted Discovery v1.2 cover URL; clients use it exactly as given (ADR 0033).
    pub(super) fn cover_url(&self, work_id: &str, sha256: &str) -> String {
        format!("{}{THEATRE_BASE}/works/{work_id}/cover/{sha256}", self.0)
    }
}

/// The exact promoted publication object named by a sealed permit. Only the listener
/// application constructs it, from the permit it has just re-resolved and authenticated.
#[derive(Clone, PartialEq, Eq)]
pub struct PublicationObjectRef {
    object_id: String,
    sha256: String,
    byte_length: u64,
}
impl PublicationObjectRef {
    pub(super) fn for_permit(permit: &PlaybackPermit) -> Self {
        Self {
            object_id: permit.object_id.clone(),
            sha256: permit.sha256.clone(),
            byte_length: permit.byte_length,
        }
    }
    pub fn object_id(&self) -> &str {
        &self.object_id
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }
}
impl fmt::Debug for PublicationObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublicationObjectRef")
            .field("object_id", &"[redacted]")
            .field("byte_length", &self.byte_length)
            .finish()
    }
}

/// Publication-class byte reads for origin delivery: a real external storage boundary, distinct
/// from PrivateStorage. An adapter reads only an object already promoted and fully verified
/// for this exact SHA/length (PublicationDelivery's observation), pinned by object version, and
/// returns exactly the requested range or a typed failure. It performs no authorization; the
/// listener application rechecks current eligibility after every open.
/// The filesystem publication class implements this port; S3 publication promotion remains
/// separate work. Named test doubles establish transport behavior, never deployed acceptance.
pub trait PublicationObjects: Send + Sync {
    fn open(
        &self,
        object: PublicationObjectRef,
        range: Option<NormalizedByteRange>,
    ) -> StorageFuture<StorageRead>;
}

/// ADR 0032 mode A source composition. One trusted value enables grant issuance and the media route
/// together, so no grant can name the route without its key and publication reader.
#[derive(Clone)]
pub struct OriginPlayback {
    origin: PublicMediaOrigin,
    key: Arc<GrantKey>,
    objects: Arc<dyn PublicationObjects>,
}
impl OriginPlayback {
    pub fn new(
        origin: PublicMediaOrigin,
        key: GrantKey,
        objects: Arc<dyn PublicationObjects>,
    ) -> Self {
        Self {
            origin,
            key: Arc::new(key),
            objects,
        }
    }
    /// The explicit HTTPS origin this host's listener URLs are built on.
    pub fn origin(&self) -> &PublicMediaOrigin {
        &self.origin
    }
    pub(super) fn key(&self) -> &GrantKey {
        &self.key
    }
    pub(super) fn objects(&self) -> &dyn PublicationObjects {
        self.objects.as_ref()
    }
}
impl PlaybackDelivery for OriginPlayback {
    /// Pure sealing: no I/O happens between the application's two eligibility reads.
    fn grant(&self, permit: PlaybackPermit) -> PlaybackDeliveryFuture {
        let expires = permit.valid_until;
        let grant = seal(&self.key, &permit.subject(), expires)
            .map_err(|_| ListenerError::RenditionUnavailable)
            .and_then(|token| {
                MediaGrant::verified_for(
                    &permit,
                    self.origin.media_url(&token, permit.access_scope.is_some()),
                    expires,
                    true,
                )
            });
        Box::pin(async move { grant })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permit() -> PlaybackPermit {
        PlaybackPermit {
            publication_id: "80000000-0000-4000-8000-000000000008".into(),
            rendition_id: "60000000-0000-4000-8000-000000000006".into(),
            manifest_digest: format!("manifest-v1:sha256:{}", "b".repeat(64)),
            sha256: "a".repeat(64),
            object_id: "publication-fixture".into(),
            byte_length: 100,
            valid_until: 10000,
            access_scope: None,
        }
    }

    #[test]
    fn grants_bind_exact_immutable_identity_and_redact_access_credentials() {
        let permit = permit();
        let sentinel = "https://synthetic.invalid/approved?signature=never-log-sentinel";
        let grant = MediaGrant::verified_for(&permit, sentinel.into(), 9000, true).unwrap();
        assert_eq!(grant.permit, permit);
        assert_eq!(permit.manifest_digest(), grant.permit.manifest_digest());
        let debug = format!("{grant:?}");
        assert!(!debug.contains("synthetic.invalid"));
        assert!(!debug.contains("never-log-sentinel"));
        assert!(debug.contains("[redacted]"));
    }

    struct NoObjects;
    impl PublicationObjects for NoObjects {
        fn open(
            &self,
            _object: PublicationObjectRef,
            _range: Option<NormalizedByteRange>,
        ) -> StorageFuture<StorageRead> {
            panic!("sealing a grant performs no object I/O")
        }
    }

    #[test]
    fn public_media_origin_admits_only_a_bare_https_origin() {
        for raw in [
            "https://listen.example.test",
            "https://listen.example.test:8443",
        ] {
            assert_eq!(PublicMediaOrigin::parse(raw).unwrap().0, raw);
        }
        for raw in [
            "http://listen.example.test",
            "https://listen.example.test/",
            "https://listen.example.test/api",
            "https://listen.example.test?q=1",
            "https://listen.example.test#f",
            "https://user:secret@listen.example.test",
            "https://LISTEN.example.test",
            "https://listen.example.test:443",
            "listen.example.test",
        ] {
            assert_eq!(
                PublicMediaOrigin::parse(raw),
                Err(ListenerError::Unavailable),
                "{raw}"
            );
        }
    }

    #[tokio::test]
    async fn origin_grant_names_the_media_route_and_authenticates_the_sealed_permit() {
        use super::super::domain::playback_grant::GrantClaims;
        let origin = OriginPlayback::new(
            PublicMediaOrigin::parse("https://listen.example.test").unwrap(),
            GrantKey::new(&[3; 32]).unwrap(),
            Arc::new(NoObjects),
        );
        let permit = permit();
        let grant = origin.grant(permit.clone()).await.unwrap();
        assert_eq!(grant.permit, permit);
        assert_eq!(grant.expires, permit.valid_until_ms());
        assert!(grant.range);
        let token = grant
            .url
            .strip_prefix("https://listen.example.test/api/v1/theatre/media/")
            .unwrap();
        let claims = GrantClaims::parse(token).unwrap();
        assert_eq!(claims.publication_id(), permit.publication_id());
        assert_eq!(claims.expires_at_ms(), permit.valid_until_ms());
        assert!(claims.authenticates(origin.key(), &permit.subject()));
        assert!(!grant.url.contains(permit.publication_object_id()));
        let object = format!("{:?}", PublicationObjectRef::for_permit(&permit));
        assert!(!object.contains(permit.publication_object_id()));
        // A permit that cannot be sealed yields no grant, never an unauthenticated URL.
        let mut unsealable = permit;
        unsealable.publication_id = "not-a-publication".into();
        assert_eq!(
            origin.grant(unsealable).await.unwrap_err(),
            ListenerError::RenditionUnavailable
        );
    }

    #[test]
    fn malformed_or_credential_bearing_locations_and_invalid_windows_fail_closed() {
        let permit = permit();
        for location in [
            "http://synthetic.invalid/approved",
            "https://",
            "https://user:secret@synthetic.invalid/approved",
            "https://synthetic.invalid/approved#fragment",
            "https://synthetic.invalid/approved\n",
            "https://synthetic.invalid/approved path",
            "not-a-url",
        ] {
            assert!(matches!(
                MediaGrant::verified_for(&permit, location.into(), 9000, true),
                Err(ListenerError::RenditionUnavailable)
            ));
        }
        for (expires, ranges) in [(-1, true), (10001, true), (9000, false)] {
            assert!(matches!(
                MediaGrant::verified_for(
                    &permit,
                    "https://synthetic.invalid/approved".into(),
                    expires,
                    ranges
                ),
                Err(ListenerError::RenditionUnavailable)
            ));
        }
    }
}
