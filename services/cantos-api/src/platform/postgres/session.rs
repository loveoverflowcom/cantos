//! Development session tokens: hashing, authentication inside a caller's transaction and
//! revocation. Identity issuance belongs to the operator tools.
use sha2::{Digest, Sha256};

use super::{Store, StoreError, Tx};
use crate::platform::{identity::domain::ActorId, studio_oidc::AuthError};

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub(crate) async fn session_token_registered(tx: &Tx<'_>, token: &str) -> Result<bool, StoreError> {
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
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(StoreError::Unauthenticated);
    }
    let row = tx.query_opt("SELECT a.id,coalesce(to_jsonb(s)->>'auth_kind','development') FROM sessions s JOIN actors a ON a.id=s.actor_id WHERE s.token_hash=$1 AND NOT s.revoked AND s.expires_at>clock_timestamp() AND a.active FOR SHARE OF s", &[&token_hash(token)]).await?;
    let row = row.ok_or(StoreError::Unauthenticated)?;
    let actor: String = row.get(0);
    if row.get::<_, String>(1) == "studio_oidc" {
        let active: Option<bool> = tx
            .query_one("SELECT production_lock_actor($1)", &[&actor])
            .await?
            .get(0);
        if active != Some(true) || tx.query_opt("SELECT token_hash FROM sessions WHERE token_hash=$1 AND NOT revoked AND expires_at>clock_timestamp()",&[&token_hash(token)]).await?.is_none() {
            return Err(StoreError::Unauthenticated);
        }
    }
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
        if let Some(oidc) = self.studio_oidc() {
            oidc.authenticate(self, token)
                .await
                .map_err(|error| match error {
                    AuthError::ProviderUnavailable | AuthError::Unavailable => {
                        StoreError::Unavailable
                    }
                    AuthError::Configuration
                    | AuthError::InvalidMetadata
                    | AuthError::InvalidToken
                    | AuthError::Algorithm
                    | AuthError::UnknownKey
                    | AuthError::InvalidKey
                    | AuthError::Signature
                    | AuthError::Issuer
                    | AuthError::Audience
                    | AuthError::Expired
                    | AuthError::NotYetValid
                    | AuthError::Nonce
                    | AuthError::Subject
                    | AuthError::State
                    | AuthError::Replay
                    | AuthError::Inactive => StoreError::Unauthenticated,
                })?;
        }
        if self.studio_oidc().is_none() && !self.development_sessions {
            return Err(StoreError::Unauthenticated);
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        if self.studio_oidc().is_none() && tx.query_opt("SELECT token_hash FROM sessions s WHERE token_hash=$1 AND coalesce(to_jsonb(s)->>'auth_kind','development')='development'",&[&token_hash(token)]).await?.is_none() {
            return Err(StoreError::Unauthenticated);
        }
        let actor = authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(actor)
    }

    pub async fn revoke(&self, token: &str) -> Result<(), StoreError> {
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
