//! Durable replay/rotation boundary. No network I/O while SQL locks are held.
use crate::platform::oidc::{
    config::OidcConfig,
    core::{AuthError, VerifiedIdentity, FLOW_SECONDS},
};
use crate::platform::store::{token_hash, Store};

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
        let mut client = self.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        let root = if let Some(previous) = previous {
            let hash = token_hash(previous);
            let root: Vec<u8> = tx.query_opt("SELECT lineage_root_hash FROM theatre_identity_sessions WHERE token_hash=$1 AND issuer=$2 AND client_id=$3", &[&hash,&config.issuer,&config.client_id]).await.map_err(unavailable)?.ok_or(AuthError::Inactive)?.get(0);
            tx.query_one("SELECT pg_advisory_xact_lock(('x'||encode(substring(sha256(convert_to('cantos-theatre-lineage-v1','UTF8')||$1::bytea),1,8),'hex'))::bit(64)::bigint)", &[&root]).await.map_err(unavailable)?;
            if !tx.query_one("SELECT EXISTS(SELECT 1 FROM theatre_identity_sessions WHERE token_hash=$1 AND NOT revoked AND expires_at>clock_timestamp())", &[&hash]).await.map_err(unavailable)?.get::<_,bool>(0) { return Err(AuthError::Inactive); }
            Some(root)
        } else {
            None
        };
        let previous = previous.map(token_hash);
        tx.execute("INSERT INTO theatre_identity_flows(state_hash,browser_hash,nonce,verifier,issuer,client_id,redirect_uri,previous_hash,expires_at,lineage_root_hash) VALUES($1,$2,$3,$4,$5,$6,$7,$8,clock_timestamp()+($9::bigint*interval '1 second'),$10)", &[&token_hash(state),&token_hash(browser),&nonce,&verifier,&config.issuer,&config.client_id,&config.redirect,&previous,&(FLOW_SECONDS as i64),&root]).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
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

    pub(super) async fn theatre_cancel_flow(
        &self,
        config: &OidcConfig,
        state: &str,
        browser: &str,
    ) -> Result<Vec<(Vec<u8>, String)>, AuthError> {
        let mut client = self.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        let hash = token_hash(state);
        // Issuance also locks the flow before its session. Cancel either wins that lock and
        // fences final issuance, or revokes the exact already-issued session after it commits.
        let row = tx.query_opt(
            "UPDATE theatre_identity_flows SET consumed=true,cancelled=true WHERE state_hash=$1 AND browser_hash=$2 AND issuer=$3 AND client_id=$4 AND redirect_uri=$5 AND expires_at>clock_timestamp() RETURNING state_hash",
            &[&hash, &token_hash(browser), &config.issuer, &config.client_id, &config.redirect],
        ).await.map_err(unavailable)?;
        if row.is_none() {
            return Err(AuthError::State);
        }
        let rows = tx.query(
            "UPDATE theatre_identity_sessions SET revoked=true,revocation_pending=true WHERE source_state_hash=$1 AND issuer=$2 AND client_id=$3 AND (NOT revoked OR revocation_pending) RETURNING token_hash,access_token",
            &[&hash, &config.issuer, &config.client_id],
        ).await.map_err(unavailable)?;
        // Lock/trigger waits must not extend the private binder's cancellation authority.
        let current: bool = tx.query_one(
            "SELECT expires_at>clock_timestamp() FROM theatre_identity_flows WHERE state_hash=$1",
            &[&hash],
        ).await.map_err(unavailable)?.get(0);
        if !current {
            return Err(AuthError::Expired);
        }
        let batch = rows
            .into_iter()
            .map(|row| (row.get(0), row.get(1)))
            .collect();
        tx.commit().await.map_err(unavailable)?;
        Ok(batch)
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
            .map_err(|e| {
                if e.code().is_some_and(|v| v.code() == "PT401") {
                    AuthError::Expired
                } else {
                    AuthError::Unavailable
                }
            })?;
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

    pub(super) async fn theatre_admission<F, Fut, T>(
        &self,
        config: &OidcConfig,
        token: &str,
        actor: &str,
        dispatch: F,
    ) -> Result<T, AuthError>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let mut client = self.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        let row = tx.query_opt("SELECT session_id::text FROM theatre_identity_sessions WHERE token_hash=$1 AND issuer=$2 AND actor_id=$3 AND NOT revoked AND expires_at>clock_timestamp() FOR SHARE", &[&token_hash(token),&config.issuer,&actor]).await.map_err(unavailable)?.ok_or(AuthError::Inactive)?;
        let session_id: String = row.get(0);
        let active = tx
            .query_one("SELECT listener_lock_actor($1)", &[&actor])
            .await
            .map_err(unavailable)?;
        if active.get::<_, Option<bool>>(0) != Some(true) {
            return Err(AuthError::Inactive);
        }
        // A separate statement guarantees this clock read follows the actor lock wait.
        let current = tx.query_one("SELECT EXISTS(SELECT 1 FROM theatre_identity_sessions WHERE token_hash=$1 AND expires_at>clock_timestamp() AND NOT revoked)", &[&token_hash(token)]).await.map_err(unavailable)?;
        if !current.get::<_, bool>(0) {
            return Err(AuthError::Inactive);
        }
        let output = dispatch(session_id).await;
        tx.commit().await.map_err(unavailable)?;
        Ok(output)
    }

    pub(super) async fn theatre_cancel_and_revoke(
        &self,
        config: &OidcConfig,
        token: Option<&str>,
        browser: Option<&str>,
    ) -> Result<Option<String>, AuthError> {
        let mut client = self.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        if let Some(browser) = browser {
            tx.execute("UPDATE theatre_identity_flows SET cancelled=true WHERE browser_hash=$1 AND issuer=$2 AND NOT completed", &[&token_hash(browser),&config.issuer]).await.map_err(unavailable)?;
        }
        let access = if let Some(token) = token {
            // Same row lock as issuance rotation and transactional authorization. Logout wins
            // before any subsequent authorization; in-flight work may linearize first.
            tx.execute(
                "UPDATE theatre_identity_sessions SET revoked=true WHERE token_hash=$1 AND issuer=$2",
                &[&token_hash(token),&config.issuer],
            )
            .await
            .map_err(unavailable)?;
            tx.query_opt("UPDATE theatre_identity_sessions SET revocation_pending=true WHERE token_hash=$1 AND issuer=$2 RETURNING access_token", &[&token_hash(token),&config.issuer]).await.map_err(unavailable)?.map(|row| row.get::<_,String>(0))
        } else {
            None
        };
        tx.commit().await.map_err(unavailable)?;
        Ok(access)
    }

    pub(super) async fn theatre_session_id(
        &self,
        config: &OidcConfig,
        token: &str,
    ) -> Result<Option<String>, AuthError> {
        Ok(self.connection().await.map_err(unavailable)?.query_opt("SELECT session_id::text FROM theatre_identity_sessions WHERE token_hash=$1 AND issuer=$2",&[&token_hash(token),&config.issuer]).await.map_err(unavailable)?.map(|r|r.get(0)))
    }

    pub(super) async fn theatre_previous_is_inert(
        &self,
        config: &OidcConfig,
        token: &str,
    ) -> Result<Option<bool>, AuthError> {
        Ok(self.connection().await.map_err(unavailable)?.query_opt(
            "SELECT revoked OR expires_at<=clock_timestamp() FROM theatre_identity_sessions WHERE token_hash=$1 AND issuer=$2 AND client_id=$3",
            &[&token_hash(token), &config.issuer, &config.client_id],
        ).await.map_err(unavailable)?.map(|row| row.get(0)))
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

    pub(super) async fn theatre_guarded_revoke(
        &self,
        config: &OidcConfig,
        token: &str,
        expected: Option<&str>,
        stale_only: bool,
        inventory: &[(String, String)],
    ) -> Result<Vec<(Vec<u8>, String)>, AuthError> {
        let mut client = self.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        let hash = token_hash(token);
        let own = tx.query_opt("SELECT source_state_hash,lineage_root_hash FROM theatre_identity_sessions WHERE token_hash=$1 AND issuer=$2 AND client_id=$3", &[&hash,&config.issuer,&config.client_id]).await.map_err(unavailable)?;
        let Some(own) = own else {
            tx.commit().await.map_err(unavailable)?;
            return Ok(Vec::new());
        };
        let root: Vec<u8> = own.get(1);
        // This root only serializes related rotations; it never grants cancellation authority.
        tx.query_one("SELECT pg_advisory_xact_lock(('x'||encode(substring(sha256(convert_to('cantos-theatre-lineage-v1','UTF8')||$1::bytea),1,8),'hex'))::bit(64)::bigint)", &[&root]).await.map_err(unavailable)?;
        let mut states = Vec::<Vec<u8>>::new();
        if let Some(state) = own.get::<_, Option<Vec<u8>>>(0) {
            states.push(state);
        }
        let mut proven = Vec::<Vec<u8>>::new();
        let mut proven_lineage = Vec::<Vec<u8>>::new();
        for row in tx.query("SELECT source_state_hash FROM theatre_identity_sessions WHERE revoked_by_token_hash=$1 AND revoked AND revocation_pending", &[&hash]).await.map_err(unavailable)? {
            if let Some(state) = row.get::<_,Option<Vec<u8>>>(0) { states.push(state); }
        }
        if expected.is_some() && !stale_only {
            for (state, browser) in inventory {
                if let Some(row) = tx.query_opt("SELECT state_hash,lineage_root_hash FROM theatre_identity_flows WHERE state_hash=$1 AND browser_hash=$2 AND issuer=$3 AND client_id=$4 AND expires_at>clock_timestamp()", &[&token_hash(state),&token_hash(browser),&config.issuer,&config.client_id]).await.map_err(unavailable)? {
                    let state: Vec<u8> = row.get(0);
                    states.push(state.clone());
                    proven.push(state.clone());
                    if row.get::<_,Option<Vec<u8>>>(1).as_ref() == Some(&root) { proven_lineage.push(state); }
                }
            }
        }
        states.sort();
        states.dedup();
        // Same order as issuance: flow locks first, then session locks. A callback either
        // commits before these locks (its sibling session is revoked), or observes cancelled.
        for state in &states {
            tx.query_opt(
                "SELECT state_hash FROM theatre_identity_flows WHERE state_hash=$1 FOR UPDATE",
                &[state],
            )
            .await
            .map_err(unavailable)?;
        }
        let rows = tx.query("SELECT token_hash,access_token,session_id::text,revoked,revocation_pending,revoked_by_token_hash,source_state_hash FROM theatre_identity_sessions WHERE issuer=$1 AND client_id=$2 AND (token_hash=$3 OR source_state_hash=ANY($4::bytea[])) ORDER BY token_hash FOR UPDATE", &[&config.issuer,&config.client_id,&hash,&states]).await.map_err(unavailable)?;
        // Refresh the commit clock after every flow/session lock wait.
        let active: bool = tx.query_one("SELECT EXISTS(SELECT 1 FROM theatre_identity_sessions WHERE token_hash=$1 AND NOT revoked AND expires_at>clock_timestamp())", &[&hash]).await.map_err(unavailable)?.get(0);
        let current = rows.iter().find(|row| row.get::<_, Vec<u8>>(0) == hash);
        let matched = current
            .is_some_and(|row| !stale_only && expected == Some(row.get::<_, String>(2).as_str()));
        let eligible = if active { &proven } else { &proven_lineage };
        let mut cancel_states = Vec::<Vec<u8>>::new();
        if matched {
            for row in tx.query("SELECT state_hash FROM theatre_identity_flows WHERE state_hash=ANY($1::bytea[]) AND expires_at>clock_timestamp()", &[eligible]).await.map_err(unavailable)? { cancel_states.push(row.get(0)); }
        }
        let mut batch = Vec::new();
        if !cancel_states.is_empty() {
            tx.execute("UPDATE theatre_identity_flows SET cancelled=true WHERE state_hash=ANY($1::bytea[])", &[&cancel_states]).await.map_err(unavailable)?;
        }
        for row in rows {
            let row_hash: Vec<u8> = row.get(0);
            let pending_sibling = row.get::<_, bool>(3)
                && row.get::<_, bool>(4)
                && row.get::<_, Option<Vec<u8>>>(5).as_ref() == Some(&hash);
            let captured = row
                .get::<_, Option<Vec<u8>>>(6)
                .as_ref()
                .is_some_and(|state| cancel_states.contains(state));
            if captured || row_hash == hash || pending_sibling {
                tx.execute("UPDATE theatre_identity_sessions SET revoked=true,revocation_pending=true,revoked_by_token_hash=CASE WHEN $2 THEN COALESCE(revoked_by_token_hash,$3) ELSE revoked_by_token_hash END WHERE token_hash=$1", &[&row_hash,&captured,&hash]).await.map_err(unavailable)?;
                batch.push((row_hash, row.get(1)));
            }
        }
        // A write trigger/constraint wait cannot extend the captured-binder cancellation window.
        if tx.query_one("SELECT EXISTS(SELECT 1 FROM theatre_identity_flows WHERE state_hash=ANY($1::bytea[]) AND expires_at<=clock_timestamp())", &[&cancel_states]).await.map_err(unavailable)?.get::<_,bool>(0) { return Err(AuthError::Expired); }
        tx.commit().await.map_err(unavailable)?;
        Ok(batch)
    }

    pub(super) async fn theatre_revocation_hash_completed(
        &self,
        hash: &[u8],
    ) -> Result<(), AuthError> {
        self.connection()
            .await
            .map_err(unavailable)?
            .execute(
                "UPDATE theatre_identity_sessions SET revocation_pending=false WHERE token_hash=$1",
                &[&hash],
            )
            .await
            .map_err(unavailable)?;
        Ok(())
    }
}
