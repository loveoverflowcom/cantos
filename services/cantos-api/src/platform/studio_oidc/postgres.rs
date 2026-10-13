//! Durable replay/rotation boundary. No network I/O while SQL locks are held.
use super::{
    config::OidcConfig,
    core::{AuthError, VerifiedIdentity, CLIENT_ID, FLOW_SECONDS},
};
use crate::platform::postgres::{token_hash, Store};

pub(super) struct Flow {
    pub nonce: String,
    pub verifier: String,
}

pub(super) struct SessionCredential {
    pub access_token: String,
    pub subject: String,
}

pub(super) enum RevocationCredential {
    None,
    PendingOnly,
    Current(String),
    ForeignIssuer,
}

fn unavailable(_: impl std::fmt::Debug) -> AuthError {
    AuthError::Unavailable
}

impl Store {
    pub(super) async fn oidc_known_inert(
        &self,
        config: &OidcConfig,
        token: &str,
    ) -> Result<bool, AuthError> {
        let client = self.connection().await.map_err(unavailable)?;
        client.query_one("SELECT EXISTS(SELECT 1 FROM sessions s LEFT JOIN studio_oidc_sessions o USING(token_hash) WHERE s.token_hash=$1 AND (s.auth_kind='development' OR (s.auth_kind='studio_oidc' AND o.issuer=$2)) AND (s.revoked OR s.expires_at<=clock_timestamp()))",&[&token_hash(token),&config.issuer]).await.map(|row|row.get(0)).map_err(unavailable)
    }
    pub(super) async fn oidc_begin(
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
        client.execute("INSERT INTO studio_oidc_flows(state_hash,browser_hash,nonce,verifier,issuer,client_id,redirect_uri,previous_hash,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,clock_timestamp()+($9::bigint*interval '1 second'))", &[&token_hash(state),&token_hash(browser),&nonce,&verifier,&config.issuer,&CLIENT_ID,&config.redirect,&previous,&(FLOW_SECONDS as i64)]).await.map_err(unavailable)?;
        Ok(())
    }

    pub(super) async fn oidc_consume(
        &self,
        config: &OidcConfig,
        state: &str,
        browser: &str,
    ) -> Result<Flow, AuthError> {
        let client = self.connection().await.map_err(unavailable)?;
        // This statement commits before token exchange, including when cancellation/timeout
        // drops the future later. A lost response cannot redispatch the authorization code.
        let row = client.query_opt("UPDATE studio_oidc_flows SET consumed=true WHERE state_hash=$1 AND browser_hash=$2 AND issuer=$3 AND redirect_uri=$4 AND client_id=$5 AND NOT consumed AND NOT cancelled AND expires_at>clock_timestamp() RETURNING nonce,verifier", &[&token_hash(state),&token_hash(browser),&config.issuer,&config.redirect,&CLIENT_ID]).await.map_err(unavailable)?.ok_or(AuthError::State)?;
        Ok(Flow {
            nonce: row.get(0),
            verifier: row.get(1),
        })
    }

    pub(super) async fn oidc_finish(
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
                "SELECT studio_oidc_finish($1,$2,$3,$4,$5,$6,$7,$8,$9)",
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

    pub(super) async fn oidc_credential(
        &self,
        config: &OidcConfig,
        token: &str,
    ) -> Result<SessionCredential, AuthError> {
        let client = self.connection().await.map_err(unavailable)?;
        let row = client.query_opt("SELECT o.access_token,o.subject FROM studio_oidc_sessions o JOIN sessions s ON s.token_hash=o.token_hash JOIN actors a ON a.id=s.actor_id WHERE o.token_hash=$1 AND o.issuer=$2 AND s.auth_kind='studio_oidc' AND NOT s.revoked AND s.expires_at>clock_timestamp() AND a.active", &[&token_hash(token),&config.issuer]).await.map_err(unavailable)?.ok_or(AuthError::Inactive)?;
        Ok(SessionCredential {
            access_token: row.get(0),
            subject: row.get(1),
        })
    }

    pub(super) async fn oidc_cancel_and_revoke(
        &self,
        config: &OidcConfig,
        token: Option<&str>,
        browser: Option<&str>,
        inventory: Option<&super::BrowserInventory>,
    ) -> Result<RevocationCredential, AuthError> {
        let mut client = self.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        let current_hash = token.map(token_hash);
        let mut lineage = None::<Vec<u8>>;
        let mut states = Vec::<Vec<u8>>::new();
        if let Some(current) = current_hash.as_ref() {
            if let Some(row)=tx.query_opt("SELECT source_state_hash,coalesce(lineage_root_hash,token_hash) FROM studio_oidc_sessions WHERE token_hash=$1 AND issuer=$2",&[current,&config.issuer]).await.map_err(unavailable)? {
                if let Some(source)=row.get::<_,Option<Vec<u8>>>(0) {states.push(source);}
                lineage=Some(row.get(1));
            }
        }
        if let Some(root) = lineage.as_ref() {
            tx.query_one("SELECT pg_advisory_xact_lock(('x'||encode(substring(sha256(convert_to('cantos-studio-session-lineage-v1','UTF8') || $1::bytea),1,8),'hex'))::bit(64)::bigint)",&[root]).await.map_err(unavailable)?;
        }
        let own_states = states.clone();
        let mut retry_states = Vec::new();
        if let Some(current) = current_hash.as_ref() {
            retry_states=tx.query("SELECT o.source_state_hash FROM studio_oidc_sessions o JOIN sessions s USING(token_hash) WHERE o.revoked_by_token_hash=$1 AND o.issuer=$2 AND o.revocation_pending AND s.revoked AND o.source_state_hash IS NOT NULL",&[current,&config.issuer]).await.map_err(unavailable)?.into_iter().map(|row|row.get::<_,Vec<u8>>(0)).collect();
            states.extend(retry_states.iter().cloned());
        }
        if let Some(inventory) = inventory {
            states.extend(inventory.flows.iter().map(|flow| flow.state_hash.clone()));
        }
        if let Some(browser) = browser {
            states.extend(
                tx.query(
                    "SELECT state_hash FROM studio_oidc_flows WHERE browser_hash=$1 AND issuer=$2",
                    &[&token_hash(browser), &config.issuer],
                )
                .await
                .map_err(unavailable)?
                .into_iter()
                .map(|row| row.get::<_, Vec<u8>>(0)),
            );
        }
        states.sort();
        states.dedup();
        // Callback holds its flow before any session. Lock the complete bounded inventory in
        // this same order, including completed flows whose cookie response may still be pending.
        let flows=tx.query("SELECT state_hash,browser_hash,issuer FROM studio_oidc_flows WHERE state_hash=ANY($1::bytea[]) ORDER BY state_hash FOR UPDATE",&[&states]).await.map_err(unavailable)?;
        if let Some(inventory) = inventory {
            for supplied in &inventory.flows {
                if !flows.iter().any(|row| {
                    row.get::<_, Vec<u8>>(0) == supplied.state_hash
                        && row.get::<_, Vec<u8>>(1) == supplied.browser_hash
                        && row.get::<_, String>(2) == config.issuer
                }) {
                    return Err(AuthError::State);
                }
            }
        }
        let mut affected=tx.query("SELECT token_hash FROM studio_oidc_sessions WHERE source_state_hash=ANY($1::bytea[]) AND issuer=$2",&[&states,&config.issuer]).await.map_err(unavailable)?.into_iter().map(|row|row.get::<_,Vec<u8>>(0)).collect::<Vec<_>>();
        if let Some(current) = &current_hash {
            affected.push(current.clone());
        }
        affected.sort();
        affected.dedup();
        tx.query("SELECT token_hash FROM sessions WHERE token_hash=ANY($1::bytea[]) ORDER BY token_hash FOR UPDATE",&[&affected]).await.map_err(unavailable)?;
        tx.query("SELECT token_hash FROM studio_oidc_sessions WHERE token_hash=ANY($1::bytea[]) ORDER BY token_hash FOR UPDATE",&[&affected]).await.map_err(unavailable)?;
        if let Some(inventory) = inventory {
            let supplied = inventory
                .flows
                .iter()
                .map(|flow| flow.state_hash.clone())
                .collect::<Vec<_>>();
            let expired:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM studio_oidc_flows WHERE state_hash=ANY($1::bytea[]) AND expires_at<=clock_timestamp())",&[&supplied]).await.map_err(unavailable)?.get(0);
            if expired {
                return Err(AuthError::State);
            }
        }
        // Fresh current facts after every wait: an inert A retry cannot cancel a newly intended B
        // inventory. Unknown/foreign-issuer tokens have no browser cancellation authority.
        let active=match current_hash.as_ref() {
            Some(current)=>tx.query_one("SELECT EXISTS(SELECT 1 FROM sessions s JOIN studio_oidc_sessions o USING(token_hash) WHERE s.token_hash=$1 AND s.auth_kind='studio_oidc' AND NOT s.revoked AND s.expires_at>clock_timestamp() AND o.issuer=$2)",&[current,&config.issuer]).await.map_err(unavailable)?.get::<_,bool>(0),
            None=>false,
        };
        let cancel_inventory = inventory.is_some() && active;
        let legacy_binder = browser.is_some();
        let cancel_states = if cancel_inventory || legacy_binder {
            states.clone()
        } else {
            let mut permitted = own_states;
            permitted.extend(retry_states);
            if let (Some(root), Some(inventory)) = (lineage.as_ref(), inventory) {
                let supplied = inventory
                    .flows
                    .iter()
                    .map(|flow| flow.state_hash.clone())
                    .collect::<Vec<_>>();
                permitted.extend(tx.query("SELECT f.state_hash FROM studio_oidc_flows f LEFT JOIN studio_oidc_sessions issued ON issued.source_state_hash=f.state_hash LEFT JOIN studio_oidc_sessions previous ON previous.token_hash=f.previous_hash WHERE f.state_hash=ANY($1::bytea[]) AND f.expires_at>clock_timestamp() AND (issued.lineage_root_hash=$2 OR previous.lineage_root_hash=$2)",&[&supplied,root]).await.map_err(unavailable)?.into_iter().map(|row|row.get::<_,Vec<u8>>(0)));
            }
            permitted.sort();
            permitted.dedup();
            permitted
        };
        let mut cancelled_receipts = false;
        if !cancel_states.is_empty() {
            tx.execute("UPDATE studio_oidc_flows SET cancelled=true WHERE state_hash=ANY($1::bytea[]) AND issuer=$2",&[&cancel_states,&config.issuer]).await.map_err(unavailable)?;
            let cancel_tokens=tx.query("SELECT o.token_hash FROM studio_oidc_sessions o JOIN sessions s USING(token_hash) WHERE o.source_state_hash=ANY($1::bytea[]) AND o.issuer=$2 AND (NOT s.revoked OR o.revocation_pending)",&[&cancel_states,&config.issuer]).await.map_err(unavailable)?.into_iter().map(|row|row.get::<_,Vec<u8>>(0)).collect::<Vec<_>>();
            cancelled_receipts = !cancel_tokens.is_empty();
            tx.execute(
                "UPDATE sessions SET revoked=true WHERE token_hash=ANY($1::bytea[])",
                &[&cancel_tokens],
            )
            .await
            .map_err(unavailable)?;
            tx.execute("UPDATE studio_oidc_sessions SET revocation_pending=true,revoked_by_token_hash=coalesce(revoked_by_token_hash,$2) WHERE token_hash=ANY($1::bytea[]) AND issuer=$3",&[&cancel_tokens,&current_hash,&config.issuer]).await.map_err(unavailable)?;
        }
        let access = if let Some(token) = token {
            // Same row lock as issuance rotation and transactional authorization. Logout wins
            // before any subsequent authorization; in-flight work may linearize first.
            tx.execute(
                "UPDATE sessions SET revoked=true WHERE token_hash=$1",
                &[&token_hash(token)],
            )
            .await
            .map_err(unavailable)?;
            match tx.query_opt("UPDATE studio_oidc_sessions SET revocation_pending=true WHERE token_hash=$1 RETURNING issuer,access_token", &[&token_hash(token)]).await.map_err(unavailable)? {
                Some(row) if row.get::<_,String>(0)==config.issuer => RevocationCredential::Current(row.get(1)),
                Some(_) => RevocationCredential::ForeignIssuer,
                None => RevocationCredential::None,
            }
        } else {
            RevocationCredential::None
        };
        // Effects acquire table/constraint locks as well as the row locks above. Every write
        // remains speculative until a final clock observation; an expired cancellation proof
        // must roll back flow/session/receipt attribution before a definitive refusal is sent.
        let mut expired = false;
        if let Some(inventory) = inventory {
            let supplied = inventory
                .flows
                .iter()
                .map(|flow| flow.state_hash.clone())
                .collect::<Vec<_>>();
            expired=tx.query_one("SELECT EXISTS(SELECT 1 FROM studio_oidc_flows WHERE state_hash=ANY($1::bytea[]) AND expires_at<=clock_timestamp())",&[&supplied]).await.map_err(unavailable)?.get(0);
        }
        if cancel_inventory {
            if let Some(current) = current_hash.as_ref() {
                expired |= tx
                    .query_one(
                        "SELECT expires_at<=clock_timestamp() FROM sessions WHERE token_hash=$1",
                        &[current],
                    )
                    .await
                    .map_err(unavailable)?
                    .get::<_, bool>(0);
            }
        }
        if expired {
            tx.rollback().await.map_err(unavailable)?;
            return Err(AuthError::State);
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(
            if cancelled_receipts && matches!(access, RevocationCredential::None) {
                RevocationCredential::PendingOnly
            } else {
                access
            },
        )
    }

    pub(super) async fn oidc_pending_siblings(&self, token: &str) -> Result<bool, AuthError> {
        self.connection().await.map_err(unavailable)?.query_one("SELECT EXISTS(SELECT 1 FROM studio_oidc_sessions WHERE revoked_by_token_hash=$1 AND token_hash<>$1 AND revocation_pending)",&[&token_hash(token)]).await.map(|row|row.get(0)).map_err(unavailable)
    }

    pub(super) async fn oidc_revocation_completed(&self, token: &str) -> Result<(), AuthError> {
        self.connection()
            .await
            .map_err(unavailable)?
            .execute(
                "UPDATE studio_oidc_sessions SET revocation_pending=false WHERE token_hash=$1",
                &[&token_hash(token)],
            )
            .await
            .map_err(unavailable)?;
        Ok(())
    }
}
