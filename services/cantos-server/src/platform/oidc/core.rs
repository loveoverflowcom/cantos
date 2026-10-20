//! Deterministic credential boundary: raw provider DTOs can never construct VerifiedIdentity.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::signature::{UnparsedPublicKey, ECDSA_P256_SHA256_FIXED};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::platform::identity::domain::ExternalIdentityKey;

pub const CLIENT_ID: &str = "cantos-studio-web-bff";
pub const FLOW_SECONDS: u64 = 300;
pub const SESSION_SECONDS: u64 = 900;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthError {
    Configuration,
    ProviderUnavailable,
    InvalidMetadata,
    InvalidToken,
    Algorithm,
    UnknownKey,
    InvalidKey,
    Signature,
    Issuer,
    Audience,
    Expired,
    NotYetValid,
    Nonce,
    Subject,
    State,
    Replay,
    Inactive,
    Unavailable,
}

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: String,
    typ: Option<String>,
    #[serde(default)]
    crit: Vec<String>,
}

#[derive(Deserialize)]
struct Claims {
    iss: String,
    sub: String,
    aud: String,
    azp: Option<String>,
    exp: u64,
    iat: u64,
    nbf: Option<u64>,
    nonce: String,
    auth_time: Option<u64>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct JwkSet {
    pub keys: Vec<Jwk>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct Jwk {
    pub kid: String,
    kty: String,
    crv: String,
    x: String,
    y: String,
    alg: Option<String>,
    #[serde(rename = "use")]
    usage: Option<String>,
    key_ops: Option<Vec<String>>,
}

/// Not Deserialize/Serialize/Clone; fields and constructor stay private to the verifier.
/// Only the sibling application may consume the facts via getters.
pub(crate) struct VerifiedIdentity {
    key: ExternalIdentityKey,
    expires: u64,
    digest: Vec<u8>,
    authenticated_at: Option<u64>,
}

/// Resource-token witness: an ID token or a registry row cannot construct it.
pub(crate) struct VerifiedAccess {
    key: ExternalIdentityKey,
    expires: u64,
}
impl VerifiedAccess {
    pub(crate) fn key(&self) -> &ExternalIdentityKey {
        &self.key
    }
    pub(crate) fn expires(&self) -> u64 {
        self.expires
    }
}

#[derive(Deserialize)]
struct AccessClaims {
    iss: String,
    sub: String,
    aud: String,
    client_id: String,
    exp: u64,
    iat: u64,
    nbf: u64,
    jti: String,
    scope: String,
}

pub(crate) fn verify_native_access(
    token: &str,
    keys: &JwkSet,
    issuer: &str,
    client_id: &str,
    now: u64,
) -> Result<VerifiedAccess, AuthError> {
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(AuthError::InvalidToken);
    }
    let header: Header =
        serde_json::from_slice(&segment(parts[0], 2048)?).map_err(|_| AuthError::InvalidToken)?;
    if header.typ.as_deref() != Some("at+jwt") {
        return Err(AuthError::InvalidToken);
    }
    let body = verify_signature(token, keys)?;
    let claims: AccessClaims =
        serde_json::from_slice(&body).map_err(|_| AuthError::InvalidToken)?;
    if claims.iss != issuer {
        return Err(AuthError::Issuer);
    }
    if claims.aud != client_id || claims.client_id != client_id {
        return Err(AuthError::Audience);
    }
    if now >= claims.exp || claims.exp <= claims.iat {
        return Err(AuthError::Expired);
    }
    if claims.iat > now || claims.nbf > now {
        return Err(AuthError::NotYetValid);
    }
    if claims.jti.is_empty()
        || claims.jti.len() > 128
        || claims.jti.chars().any(char::is_control)
        || !claims.scope.split_ascii_whitespace().any(|v| v == "openid")
    {
        return Err(AuthError::InvalidToken);
    }
    let key = ExternalIdentityKey::new(claims.iss, claims.sub).map_err(|_| AuthError::Subject)?;
    Ok(VerifiedAccess {
        key,
        expires: claims.exp,
    })
}

impl VerifiedIdentity {
    pub(crate) fn key(&self) -> &ExternalIdentityKey {
        &self.key
    }
    pub(crate) fn expires(&self) -> u64 {
        self.expires
    }
    pub(crate) fn authenticated_at(&self) -> Option<u64> {
        self.authenticated_at
    }
    pub(crate) fn digest(&self) -> &[u8] {
        &self.digest
    }
}

fn segment(value: &str, limit: usize) -> Result<Vec<u8>, AuthError> {
    if value.is_empty() || value.len() > limit {
        return Err(AuthError::InvalidToken);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| AuthError::InvalidToken)?;
    if URL_SAFE_NO_PAD.encode(&bytes) != value {
        return Err(AuthError::InvalidToken);
    }
    Ok(bytes)
}

pub(crate) fn token_kid(token: &str) -> Result<String, AuthError> {
    if token.len() > 16_384 {
        return Err(AuthError::InvalidToken);
    }
    let encoded = token.split('.').next().ok_or(AuthError::InvalidToken)?;
    let header: Header =
        serde_json::from_slice(&segment(encoded, 2048)?).map_err(|_| AuthError::InvalidToken)?;
    if header.alg != "ES256" || !header.crit.is_empty() {
        return Err(AuthError::Algorithm);
    }
    if header.kid.is_empty() || header.kid.len() > 128 {
        return Err(AuthError::InvalidKey);
    }
    Ok(header.kid)
}

pub(crate) fn verify_for_client(
    token: &str,
    keys: &JwkSet,
    issuer: &str,
    client_id: &str,
    nonce: &str,
    now: u64,
) -> Result<VerifiedIdentity, AuthError> {
    let encoded = token.split('.').next().ok_or(AuthError::InvalidToken)?;
    let header: Header =
        serde_json::from_slice(&segment(encoded, 2048)?).map_err(|_| AuthError::InvalidToken)?;
    if header.typ.as_deref().is_some_and(|v| v != "JWT") {
        return Err(AuthError::InvalidToken);
    }
    let body = verify_signature(token, keys)?;
    let claims: Claims = serde_json::from_slice(&body).map_err(|_| AuthError::InvalidToken)?;
    if claims.iss != issuer {
        return Err(AuthError::Issuer);
    }
    if claims.aud != client_id || claims.azp.as_deref().is_some_and(|azp| azp != client_id) {
        return Err(AuthError::Audience);
    }
    // No ambient clock/leeway. A callback proof must have been minted in the flow's lifetime.
    if now >= claims.exp || now >= claims.iat.saturating_add(FLOW_SECONDS) {
        return Err(AuthError::Expired);
    }
    if claims.iat > now || claims.nbf.is_some_and(|nbf| nbf > now) {
        return Err(AuthError::NotYetValid);
    }
    if nonce.is_empty() || !constant_time_equal(claims.nonce.as_bytes(), nonce.as_bytes()) {
        return Err(AuthError::Nonce);
    }
    if claims
        .auth_time
        .is_some_and(|time| time > claims.iat || time > now)
    {
        return Err(AuthError::NotYetValid);
    }
    let key = ExternalIdentityKey::new(claims.iss, claims.sub).map_err(|_| AuthError::Subject)?;
    Ok(VerifiedIdentity {
        key,
        expires: claims.exp,
        digest: Sha256::digest(token).to_vec(),
        authenticated_at: claims.auth_time,
    })
}

/// Shared crypto only: callers still validate their own credential purpose and claims.
pub(crate) fn verify_signature(token: &str, keys: &JwkSet) -> Result<Vec<u8>, AuthError> {
    let kid = token_kid(token)?;
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(AuthError::InvalidToken);
    }
    let mut matching = keys.keys.iter().filter(|key| key.kid == kid);
    let key = matching.next().ok_or(AuthError::UnknownKey)?;
    if matching.next().is_some()
        || key.kty != "EC"
        || key.crv != "P-256"
        || key.alg.as_deref().is_some_and(|alg| alg != "ES256")
        || key.usage.as_deref().is_some_and(|usage| usage != "sig")
        || key
            .key_ops
            .as_ref()
            .is_some_and(|ops| ops.as_slice() != ["verify"])
    {
        return Err(AuthError::InvalidKey);
    }
    let x = segment(&key.x, 64).map_err(|_| AuthError::InvalidKey)?;
    let y = segment(&key.y, 64).map_err(|_| AuthError::InvalidKey)?;
    if x.len() != 32 || y.len() != 32 {
        return Err(AuthError::InvalidKey);
    }
    let public = [vec![4], x, y].concat();
    let signature = segment(parts[2], 128)?;
    let message = format!("{}.{}", parts[0], parts[1]);
    UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, public)
        .verify(message.as_bytes(), &signature)
        .map_err(|_| AuthError::Signature)?;
    segment(parts[1], 12_000)
}

fn constant_time_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

pub(crate) fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub(crate) fn session_expiry(now: u64, proof: u64, access: u64) -> Result<u64, AuthError> {
    let expires = proof.min(access).min(now.saturating_add(SESSION_SECONDS));
    if now >= expires {
        return Err(AuthError::Expired);
    }
    Ok(expires)
}

#[cfg(test)]
pub(crate) fn verify(
    token: &str,
    keys: &JwkSet,
    issuer: &str,
    nonce: &str,
    now: u64,
) -> Result<VerifiedIdentity, AuthError> {
    verify_for_client(token, keys, issuer, CLIENT_ID, nonce, now)
}
