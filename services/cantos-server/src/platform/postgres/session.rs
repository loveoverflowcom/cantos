//! Development session tokens: hashing, authentication inside a caller's transaction and
//! revocation. Identity issuance belongs to the operator tools.
use sha2::{Digest, Sha256};

use super::{Store, StoreError, Tx};
use crate::platform::identity::domain::ActorId;

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub(crate) async fn session_token_registered(tx: &Tx<'_>, token: &str) -> Result<bool, StoreError> {
    // `ca1_` belonged to the retired provider-bound family; the namespace stays reserved so a
    // producer/capability token can never be minted in a retired credential's shape.
    if token.starts_with("ca1_") || token.starts_with("ci1_") {
        return Ok(true);
    }
    Ok(tx
        .query_opt(
            "SELECT token_hash FROM sessions WHERE token_hash=$1",
            &[&token_hash(token)],
        )
        .await?
        .is_some())
}

/// Locks the session row for the transaction, so a concurrent revocation waits for it.
pub(crate) async fn authenticate(tx: &Tx<'_>, token: &str) -> Result<String, StoreError> {
    if token.starts_with("ci1_") {
        return crate::platform::identity_auth::postgres::admit_studio(tx, token).await;
    }
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(StoreError::Unauthenticated);
    }
    // Only development tokens live in `sessions`; a row of any other `auth_kind` (the retired
    // Studio OIDC sessions) is never admitted.
    let row = tx.query_opt("SELECT a.id FROM sessions s JOIN actors a ON a.id=s.actor_id WHERE s.token_hash=$1 AND coalesce(to_jsonb(s)->>'auth_kind','development')='development' AND NOT s.revoked AND s.expires_at>clock_timestamp() AND a.active FOR SHARE OF s", &[&token_hash(token)]).await?;
    let row = row.ok_or(StoreError::Unauthenticated)?;
    let actor: String = row.get(0);
    ActorId::new(actor)
        .map(String::from)
        .map_err(|_| StoreError::CorruptRevision)
}

impl Store {
    /// Production composition: refuse every development Studio session token and token login.
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
            return crate::platform::identity_auth::postgres::studio_session_id(self, token).await;
        }
        let client = self.connection().await?;
        let row = client
            .query_opt(
                "SELECT to_jsonb(s)->>'session_id' FROM sessions s WHERE token_hash=$1",
                &[&token_hash(token)],
            )
            .await?;
        Ok(row.and_then(|row| row.get(0)))
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
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
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
        let client = self.connection().await?;
        client
            .execute(
                "UPDATE sessions SET revoked=true WHERE token_hash=$1",
                &[&token_hash(token)],
            )
            .await?;
        Ok(())
    }
}
