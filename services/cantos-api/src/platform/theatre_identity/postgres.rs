//! Durable replay/rotation boundary. No network I/O while SQL locks are held.
use crate::platform::oidc::{
    config::OidcConfig,
    core::{AuthError, VerifiedIdentity, FLOW_SECONDS},
};
use crate::platform::postgres::{token_hash, Store};

pub(super) struct Flow {
    pub nonce: String,
    pub verifier: String,
}

pub(super) struct SessionCredential {
    pub access_token: String,
    pub subject: String,
    pub actor: String,
}

fn unavailable(_: impl std::fmt::Debug) -> AuthError {
    AuthError::Unavailable
}

impl Store {
    pub(super) async fn theatre_begin(
        &self,
        config: &OidcConfig,
        state: &str,
        browser: &str,
        nonce: &str,
        verifier: &str,
        previous: Option<&str>,
    ) -> Result<(), AuthError> {
        let client = self.connection().await.map_err(unavailable)?;
        let previous = previous.map(token_hash);
        client.execute("INSERT INTO theatre_identity_flows(state_hash,browser_hash,nonce,verifier,issuer,client_id,redirect_uri,previous_hash,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,clock_timestamp()+($9::bigint*interval '1 second'))", &[&token_hash(state),&token_hash(browser),&nonce,&verifier,&config.issuer,&config.client_id,&config.redirect,&previous,&(FLOW_SECONDS as i64)]).await.map_err(unavailable)?;
        Ok(())
    }

    pub(super) async fn theatre_consume(
        &self,
        config: &OidcConfig,
        state: &str,
        browser: &str,
    ) -> Result<Flow, AuthError> {
        let client = self.connection().await.map_err(unavailable)?;
        // This statement commits before token exchange, including when cancellation/timeout
        // drops the future later. A lost response cannot redispatch the authorization code.
        let row = client.query_opt("UPDATE theatre_identity_flows SET consumed=true WHERE state_hash=$1 AND browser_hash=$2 AND issuer=$3 AND redirect_uri=$4 AND client_id=$5 AND NOT consumed AND NOT cancelled AND expires_at>clock_timestamp() RETURNING nonce,verifier", &[&token_hash(state),&token_hash(browser),&config.issuer,&config.redirect,&config.client_id]).await.map_err(unavailable)?.ok_or(AuthError::State)?;
        Ok(Flow {
            nonce: row.get(0),
            verifier: row.get(1),
        })
    }

    pub(super) async fn theatre_finish(
        &self,
        config: &OidcConfig,
        state: &str,
        proof: VerifiedIdentity,
        access: &str,
        token: &str,
        expires: u64,
    ) -> Result<String, AuthError> {
        let client = self.connection().await.map_err(unavailable)?;
        let candidate = uuid::Uuid::new_v4().to_string();
        let expires = i64::try_from(expires).map_err(|_| AuthError::Expired)?;
        let row = client
            .query_one(
                "SELECT theatre_identity_finish($1,$2,$3,$4,$5,$6,$7,$8,$9)",
                &[
                    &token_hash(state),
                    &proof.key().issuer(),
                    &proof.key().subject(),
                    &config.redirect,
                    &candidate,
                    &token_hash(token),
                    &proof.digest(),
                    &access,
                    &expires,
                ],
            )
            .await
            .map_err(unavailable)?;
        row.get::<_, Option<String>>(0).ok_or(AuthError::Inactive)
    }

    pub(super) async fn theatre_credential(
        &self,
        config: &OidcConfig,
        token: &str,
    ) -> Result<SessionCredential, AuthError> {
        let client = self.connection().await.map_err(unavailable)?;
        let row = client.query_opt("SELECT o.access_token,o.subject,o.actor_id FROM theatre_identity_sessions o JOIN actors a ON a.id=o.actor_id WHERE o.token_hash=$1 AND o.issuer=$2 AND o.client_id=$3 AND NOT o.revoked AND o.expires_at>clock_timestamp() AND a.active", &[&token_hash(token),&config.issuer,&config.client_id]).await.map_err(unavailable)?.ok_or(AuthError::Inactive)?;
        Ok(SessionCredential {
            access_token: row.get(0),
            subject: row.get(1),
            actor: row.get(2),
        })
    }

    pub(super) async fn theatre_cancel_and_revoke(
        &self,
        token: Option<&str>,
        browser: Option<&str>,
    ) -> Result<Option<String>, AuthError> {
        let mut client = self.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        if let Some(browser) = browser {
            tx.execute("UPDATE theatre_identity_flows SET cancelled=true WHERE browser_hash=$1 AND NOT completed", &[&token_hash(browser)]).await.map_err(unavailable)?;
        }
        let access = if let Some(token) = token {
            // Same row lock as issuance rotation and transactional authorization. Logout wins
            // before any subsequent authorization; in-flight work may linearize first.
            tx.execute(
                "UPDATE theatre_identity_sessions SET revoked=true WHERE token_hash=$1",
                &[&token_hash(token)],
            )
            .await
            .map_err(unavailable)?;
            tx.query_opt("UPDATE theatre_identity_sessions SET revocation_pending=true WHERE token_hash=$1 RETURNING access_token", &[&token_hash(token)]).await.map_err(unavailable)?.map(|row| row.get::<_,String>(0))
        } else {
            None
        };
        tx.commit().await.map_err(unavailable)?;
        Ok(access)
    }

    pub(super) async fn theatre_revocation_completed(&self, token: &str) -> Result<(), AuthError> {
        self.connection()
            .await
            .map_err(unavailable)?
            .execute(
                "UPDATE theatre_identity_sessions SET revocation_pending=false WHERE token_hash=$1",
                &[&token_hash(token)],
            )
            .await
            .map_err(unavailable)?;
        Ok(())
    }
}
