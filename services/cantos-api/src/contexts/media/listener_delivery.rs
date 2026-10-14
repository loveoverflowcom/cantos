//! Playback is a real delivery consumer, separate from immutable catalog identity.
use super::listener_application::ListenerError;
use std::{future::Future, pin::Pin};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackPermit {
    pub(super) publication_id: String,
    pub(super) rendition_id: String,
    pub(super) manifest_digest: String,
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
}
/// Only trusted delivery adapters create grants after verifying the intended public object's
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
    /// This validates adapter output, not the storage bytes or current public eligibility.
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
/// TODO(issue120, #10/#11): compose a publication-class adapter only after the delivery-mode
/// ADR and live byte-integrity/Range/expiry/withdrawal/cache-isolation gates are accepted.
/// Never adapt PrivateStorage or a Studio preview grant into listener authority.
pub trait PlaybackDelivery: Send + Sync {
    fn grant(&self, permit: PlaybackPermit) -> PlaybackDeliveryFuture;
}
pub(super) struct UnavailablePlayback;
impl PlaybackDelivery for UnavailablePlayback {
    fn grant(&self, _permit: PlaybackPermit) -> PlaybackDeliveryFuture {
        Box::pin(async { Err(ListenerError::RenditionUnavailable) })
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
