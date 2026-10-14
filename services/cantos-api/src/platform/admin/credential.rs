//! Actual ES256 resource credential boundary. Raw claims cannot create Admin authority.
use super::{
    domain::{map_groups, AdminError, Role, RoleEvidence},
    CLIENT_ID,
};
use crate::platform::{
    identity::domain::ExternalIdentityKey,
    oidc::core::{verify_signature, JwkSet},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;

#[derive(Deserialize)]
struct Header {
    typ: String,
}
#[derive(Deserialize)]
struct Claims {
    iss: String,
    sub: String,
    aud: String,
    client_id: String,
    exp: u64,
    iat: u64,
    nbf: Option<u64>,
    #[serde(default)]
    cantos_admin_groups: Vec<String>,
}
/// Verified signature and purpose, before current-provider observation and local policy.
pub(super) struct SignedAdmin {
    key: ExternalIdentityKey,
    role: Role,
    expires: u64,
}
impl SignedAdmin {
    pub(super) fn subject(&self) -> &str {
        self.key.subject()
    }
    pub(super) fn observe(
        self,
        groups: &[String],
        access_expiry: u64,
        now: u64,
    ) -> Result<RoleEvidence, AdminError> {
        // Removal/demotion invalidates a previously signed privileged claim immediately at this observation.
        let current = map_groups(groups)?;
        if current != self.role {
            return Err(AdminError::StaleRole);
        }
        let expires = self.expires.min(access_expiry).min(now.saturating_add(5));
        if now >= expires {
            return Err(AdminError::StaleRole);
        }
        Ok(RoleEvidence::verified(self.key, current, now, expires))
    }
}

pub(super) fn verify(
    token: &str,
    keys: &JwkSet,
    issuer: &str,
    now: u64,
) -> Result<SignedAdmin, AdminError> {
    let payload = verify_signature(token, keys).map_err(|_| AdminError::Credential)?;
    // Check purpose only after signature. A Studio ID-token or opaque session is never accepted.
    let encoded = token.split('.').next().ok_or(AdminError::Credential)?;
    let header: Header = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| AdminError::Credential)?,
    )
    .map_err(|_| AdminError::Credential)?;
    if header.typ != "at+jwt" {
        return Err(AdminError::Credential);
    }
    let claims: Claims = serde_json::from_slice(&payload).map_err(|_| AdminError::Credential)?;
    if claims.iss != issuer || claims.aud != CLIENT_ID || claims.client_id != CLIENT_ID {
        return Err(AdminError::Credential);
    }
    let expires = claims.exp.min(claims.iat.saturating_add(300));
    if now >= expires || claims.iat > now || claims.nbf.is_some_and(|n| n > now) {
        return Err(AdminError::Credential);
    }
    let key =
        ExternalIdentityKey::new(claims.iss, claims.sub).map_err(|_| AdminError::Credential)?;
    Ok(SignedAdmin {
        key,
        role: map_groups(&claims.cantos_admin_groups)?,
        expires,
    })
}
