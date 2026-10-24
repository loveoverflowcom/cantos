//! Studio admission by credential kind, and the development session tokens.
//!
//! Two Studio credentials exist: the Identity v1 Web cookie (`ci1_`), admitted through the shared
//! Identity session adapter, and a development token (a 64-hex secret in `development_sessions`)
//! that only a non-production host accepts. Provider sessions no longer exist. Identity issuance
//! belongs to `cantos-identity` and the operator tools.
use sha2::{Digest, Sha256};

use super::{Store, StoreError, Tx};
use crate::platform::identity::domain::ActorId;

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn is_development_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) async fn session_token_registered(tx: &Tx<'_>, token: &str) -> Result<bool, StoreError> {
    if token.starts_with("ci1_") {
        return Ok(true);
    }
    Ok(tx
        .query_opt(
            "SELECT 1 FROM development_sessions WHERE token_hash=?1",
            &[&token_hash(token)],
        )
        .await?
        .is_some())
}

/// Admits the credential inside the caller's unit: a logout, revocation or deactivation either
/// committed before it (and the credential is refused) or waits behind it on the serialized
/// writer.
pub(crate) async fn authenticate(tx: &Tx<'_>, token: &str) -> Result<String, StoreError> {
    if token.starts_with("ci1_") {
        return crate::platform::identity_auth::turso::admit_studio(tx, token).await;
    }
    if !is_development_token(token) {
        return Err(StoreError::Unauthenticated);
    }
    let row = tx
        .query_opt(
            "SELECT a.id FROM development_sessions s JOIN actors a ON a.id=s.actor_id \
             WHERE s.token_hash=?1 AND s.revoked=0 AND s.expires_at>?2 AND a.active=1",
            &[&token_hash(token), &tx.now_micros()],
        )
        .await?;
    let actor: String = row.ok_or(StoreError::Unauthenticated)?.try_get(0)?;
    ActorId::new(actor)
        .map(String::from)
        .map_err(|_| StoreError::CorruptRevision)
}

impl Store {
    /// Production composition: refuse every development Studio session token.
    pub fn without_development_sessions(mut self) -> Self {
        self.development_sessions = false;
        self
    }
    pub fn development_sessions_enabled(&self) -> bool {
        self.development_sessions
    }
    /// Public correlation only; knowing this UUID never authenticates a caller.
    pub async fn studio_session_id(&self, token: &str) -> Result<Option<String>, StoreError> {
        if token.starts_with("ci1_") {
            return crate::platform::identity_auth::turso::studio_session_id(self, token).await;
        }
        let client = self.connection().await?;
        let row = client
            .query_opt(
                "SELECT session_id FROM development_sessions WHERE token_hash=?1",
                &[&token_hash(token)],
            )
            .await?;
        row.map(|row| row.try_get(0).map_err(StoreError::from))
            .transpose()
    }

    pub async fn authenticate(&self, token: &str) -> Result<String, StoreError> {
        if token.starts_with("ci1_") {
            use crate::platform::identity_auth::{
                credential::{Channel, Surface},
                AuthFailure,
            };
            let service = self.identity_auth().ok_or(StoreError::Unauthenticated)?;
            return service
                .authenticate(self, token, Channel::Web(Surface::Studio))
                .await
                .map(|record| record.actor_id)
                .map_err(|failure| match failure {
                    AuthFailure::Unavailable | AuthFailure::Throttled { .. } => {
                        StoreError::Unavailable
                    }
                    _ => StoreError::Unauthenticated,
                });
        }
        if !self.development_sessions {
            return Err(StoreError::Unauthenticated);
        }
        let client = self.connection().await?;
        let tx = client.read().await?;
        let actor = authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(actor)
    }

    pub async fn revoke(&self, token: &str) -> Result<(), StoreError> {
        if token.starts_with("ci1_") {
            use crate::platform::identity_auth::credential::{Channel, Surface};
            let service = self.identity_auth().ok_or(StoreError::Unauthenticated)?;
            return service
                .logout(self, token, Channel::Web(Surface::Studio))
                .await
                .map_err(|_| StoreError::Unavailable);
        }
        let mut client = self.connection().await?;
        client
            .execute(
                "UPDATE development_sessions SET revoked=1 WHERE token_hash=?1",
                &[&token_hash(token)],
            )
            .await?;
        Ok(())
    }
}

/// Read current actor state inside the caller's writer unit or pinned read snapshot.
/// Contexts never query the platform-owned actor table themselves.
pub(crate) async fn actor_active(tx: &Tx<'_>, actor: &str) -> Result<bool, StoreError> {
    tx.query_opt("SELECT active FROM actors WHERE id=?1", &[&actor])
        .await?
        .map(|row| row.try_get::<_, bool>(0).map_err(StoreError::from))
        .transpose()
        .map(|active| active.unwrap_or(false))
}
