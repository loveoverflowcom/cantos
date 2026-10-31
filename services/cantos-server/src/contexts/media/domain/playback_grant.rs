//! Origin-delivery playback grants (proposed ADR 0032, mode A). A grant is a short-lived HMAC
//! capability for one sealed publication permit, with optional session/access binding. Its token names only the publication and expiry;
//! the MAC authenticates every immutable identity field, including the private object ID,
//! without revealing it. Pure: no clock, randomness, key loading, SQL, storage or HTTP.
//! Possessing a valid token is never eligibility: the application re-resolves the current
//! authorized release before and after every object read and decides with `decide_media_fetch`.
use std::fmt;

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::platform::ids::canonical_uuid;

/// Upper bound of one grant. Existing playback authorization already used this window.
pub const GRANT_TTL_MS: i64 = 120_000;
/// `pg1_` + 32 hex publication UUID bits + 16 hex expiry + 64 hex HMAC-SHA256.
pub const GRANT_TOKEN_LEN: usize = 116;
const PREFIX: &str = "pg1_";
const DOMAIN: &[u8] = b"cantos/theatre/playback-grant/v1\n";
const KEY_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantError {
    InvalidKey,
    InvalidSubject,
    InvalidExpiry,
    Malformed,
}

/// Secret HMAC key held by trusted composition only. It has no Clone, serde or readable Debug.
pub struct GrantKey([u8; KEY_BYTES]);
impl GrantKey {
    pub fn new(bytes: &[u8]) -> Result<Self, GrantError> {
        let key: [u8; KEY_BYTES] = bytes.try_into().map_err(|_| GrantError::InvalidKey)?;
        if key.iter().all(|byte| *byte == 0) {
            return Err(GrantError::InvalidKey);
        }
        Ok(Self(key))
    }
    fn mac(&self) -> Result<Hmac<Sha256>, GrantError> {
        Hmac::<Sha256>::new_from_slice(&self.0).map_err(|_| GrantError::InvalidKey)
    }
}
impl fmt::Debug for GrantKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GrantKey([redacted])")
    }
}

/// Immutable identity of the exact sealed permit a grant authenticates. Deliberately not
/// Debug: `object_id` is a private delivery location that must never reach logs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GrantSubject<'a> {
    pub publication_id: &'a str,
    pub rendition_id: &'a str,
    pub manifest_digest: &'a str,
    pub sha256: &'a str,
    pub object_id: &'a str,
    pub byte_length: u64,
    /// Hash of the current actor/session, purpose, generation and access policy.
    /// None is exclusively the anonymous public lane.
    pub access_scope: Option<&'a str>,
}

/// Claims parsed from a presented token. Parsing proves nothing; only `authenticates`
/// against the freshly resolved current subject does.
#[derive(Clone, PartialEq, Eq)]
pub struct GrantClaims {
    publication_id: String,
    expires_at_ms: i64,
    mac: [u8; 32],
}
impl fmt::Debug for GrantClaims {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GrantClaims")
            .field("publication_id", &self.publication_id)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("mac", &"[redacted]")
            .finish()
    }
}
impl GrantClaims {
    /// Only the exact canonical lower-case spelling parses; there is no alternate encoding.
    pub fn parse(token: &str) -> Result<Self, GrantError> {
        let rest = token
            .strip_prefix(PREFIX)
            .filter(|_| token.len() == GRANT_TOKEN_LEN)
            .ok_or(GrantError::Malformed)?;
        if !rest
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(GrantError::Malformed);
        }
        let id = u128::from_str_radix(&rest[..32], 16).map_err(|_| GrantError::Malformed)?;
        let expires = u64::from_str_radix(&rest[32..48], 16).map_err(|_| GrantError::Malformed)?;
        let expires_at_ms = i64::try_from(expires).map_err(|_| GrantError::Malformed)?;
        let mut mac = [0u8; 32];
        for (index, byte) in mac.iter_mut().enumerate() {
            let pair = &rest[48 + 2 * index..50 + 2 * index];
            *byte = u8::from_str_radix(pair, 16).map_err(|_| GrantError::Malformed)?;
        }
        Ok(Self {
            publication_id: uuid::Uuid::from_u128(id).to_string(),
            expires_at_ms,
            mac,
        })
    }
    pub fn publication_id(&self) -> &str {
        &self.publication_id
    }
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
    /// Constant-time MAC comparison over the complete current identity and this expiry.
    pub fn authenticates(&self, key: &GrantKey, subject: &GrantSubject<'_>) -> bool {
        subject.publication_id == self.publication_id
            && authenticator(key, subject, self.expires_at_ms)
                .is_ok_and(|mac| mac.verify_slice(&self.mac).is_ok())
    }
}

/// Seal one grant. The caller has already bounded `expires_at_ms` with `grant_expiry`.
pub fn seal(
    key: &GrantKey,
    subject: &GrantSubject<'_>,
    expires_at_ms: i64,
) -> Result<String, GrantError> {
    let id = canonical_uuid(subject.publication_id)
        .and_then(|id| uuid::Uuid::parse_str(&id).ok())
        .ok_or(GrantError::InvalidSubject)?;
    let expires = u64::try_from(expires_at_ms).map_err(|_| GrantError::InvalidExpiry)?;
    let mac = authenticator(key, subject, expires_at_ms)?
        .finalize()
        .into_bytes();
    let mut token = String::with_capacity(GRANT_TOKEN_LEN);
    token.push_str(PREFIX);
    token.push_str(&format!("{:032x}{expires:016x}", id.as_u128()));
    for byte in mac {
        token.push_str(&format!("{byte:02x}"));
    }
    Ok(token)
}

/// Domain-separated, length-prefixed preimage: no two field sequences share bytes.
fn authenticator(
    key: &GrantKey,
    subject: &GrantSubject<'_>,
    expires_at_ms: i64,
) -> Result<Hmac<Sha256>, GrantError> {
    let mut mac = key.mac()?;
    mac.update(DOMAIN);
    for field in [
        subject.publication_id,
        subject.rendition_id,
        subject.manifest_digest,
        subject.sha256,
        subject.object_id,
    ] {
        mac.update(&(field.len() as u64).to_be_bytes());
        mac.update(field.as_bytes());
    }
    if let Some(scope) = subject.access_scope {
        mac.update(b"cantos/theatre/restricted/v1\n");
        mac.update(&(scope.len() as u64).to_be_bytes());
        mac.update(scope.as_bytes());
    }
    mac.update(&subject.byte_length.to_be_bytes());
    mac.update(&expires_at_ms.to_be_bytes());
    Ok(mac)
}

/// Grant expiry for a request observed at `now`: never beyond the TTL or the rights window,
/// and always strictly in the future. Saturating addition keeps the bound total near i64::MAX.
pub fn grant_expiry(now_ms: i64, rights_valid_until_ms: i64) -> Option<i64> {
    let expires = now_ms
        .saturating_add(GRANT_TTL_MS)
        .min(rights_valid_until_ms);
    (now_ms >= 0 && expires > now_ms).then_some(expires)
}

/// Current public facts of the release named by a grant, freshly resolved for this fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurrentRelease {
    pub published_at_ms: i64,
    pub rights_valid_until_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaFetchFacts {
    pub now_ms: i64,
    pub expires_at_ms: i64,
    /// `None` when the publication is not currently eligible (withdrawn, revoked, superseded,
    /// owner disabled, private, archived, expired rights or absent).
    pub current: Option<CurrentRelease>,
    /// Result of `GrantClaims::authenticates` against that current release's identity.
    pub authenticated: bool,
}

/// Every denial is presented to anonymous callers as the same `not_found`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaFetchDenial {
    Expired,
    Unbounded,
    NotCurrent,
    Forged,
    OutsideRights,
}

/// Per-fetch admission, evaluated before planning a response and again after object I/O.
/// A grant never outlives its TTL, the rights window or the release's current eligibility.
pub fn decide_media_fetch(f: MediaFetchFacts) -> Result<(), MediaFetchDenial> {
    if f.now_ms < 0 || f.now_ms >= f.expires_at_ms {
        return Err(MediaFetchDenial::Expired);
    }
    if f.expires_at_ms > f.now_ms.saturating_add(GRANT_TTL_MS) {
        return Err(MediaFetchDenial::Unbounded);
    }
    let Some(current) = f.current else {
        return Err(MediaFetchDenial::NotCurrent);
    };
    if !f.authenticated {
        return Err(MediaFetchDenial::Forged);
    }
    if current.published_at_ms < 0
        || current.published_at_ms > f.now_ms
        || f.expires_at_ms > current.rights_valid_until_ms
    {
        return Err(MediaFetchDenial::OutsideRights);
    }
    Ok(())
}

#[cfg(test)]
#[path = "playback_grant_tests.rs"]
mod tests;

#[cfg(kani)]
mod kani_proofs {
    use super::{
        decide_media_fetch, grant_expiry, CurrentRelease, MediaFetchDenial, MediaFetchFacts,
        GRANT_TTL_MS,
    };
    /// Full i64 clocks and windows, no assumptions/stubs. Proves the exact expiry bound and
    /// the complete per-fetch admission/precedence; not HMAC, SQL, storage or HTTP.
    #[kani::proof]
    #[kani::unwind(2)]
    fn playback_grant_expiry_and_fetch_fail_closed() {
        let now: i64 = kani::any();
        let until: i64 = kani::any();
        let wide_now = i128::from(now);
        let bound = (wide_now + i128::from(GRANT_TTL_MS)).min(i128::from(until));
        let expected_expiry = (now >= 0 && bound > wide_now).then_some(bound);
        assert_eq!(grant_expiry(now, until).map(i128::from), expected_expiry);

        let expires: i64 = kani::any();
        let published: i64 = kani::any();
        let present: bool = kani::any();
        let authenticated: bool = kani::any();
        let current = present.then_some(CurrentRelease {
            published_at_ms: published,
            rights_valid_until_ms: until,
        });
        let decision = decide_media_fetch(MediaFetchFacts {
            now_ms: now,
            expires_at_ms: expires,
            current,
            authenticated,
        });
        let wide_expires = i128::from(expires);
        let expected = if now < 0 || now >= expires {
            Err(MediaFetchDenial::Expired)
        } else if wide_expires > wide_now + i128::from(GRANT_TTL_MS) {
            Err(MediaFetchDenial::Unbounded)
        } else if !present {
            Err(MediaFetchDenial::NotCurrent)
        } else if !authenticated {
            Err(MediaFetchDenial::Forged)
        } else if published < 0 || published > now || expires > until {
            Err(MediaFetchDenial::OutsideRights)
        } else {
            Ok(())
        };
        assert_eq!(decision, expected);
        kani::cover!(decision.is_ok(), "current authenticated fetch admitted");
        // Cover expressions use widened arithmetic so they add no overflow obligations.
        kani::cover!(
            wide_now == wide_expires - 1 && decision.is_ok(),
            "last admitted millisecond"
        );
        kani::cover!(now == expires && now >= 0, "exclusive grant expiry");
        kani::cover!(
            decision == Err(MediaFetchDenial::Unbounded),
            "grant beyond the TTL rejected"
        );
        kani::cover!(
            decision == Err(MediaFetchDenial::OutsideRights)
                && wide_expires == i128::from(until) + 1,
            "grant outliving rights rejected"
        );
        kani::cover!(
            now > i64::MAX - GRANT_TTL_MS && expected_expiry.is_some(),
            "saturating expiry near the clock limit"
        );
    }
}
