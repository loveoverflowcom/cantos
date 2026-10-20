//! Native-only durable single-attempt and session admission. No provider I/O under locks.
use crate::platform::{
    oidc::{
        config::OidcConfig,
        core::{AuthError, VerifiedIdentity},
    },
    postgres::{token_hash, Store},
};
use cantos_contracts::theatre_identity::{NativeFlowRequest, NativeSession};
pub(super) struct Credential {
    pub access_token: String,
    pub subject: String,
    pub view: NativeSession,
}
fn unavailable(_: impl std::fmt::Debug) -> AuthError {
    AuthError::Unavailable
}
fn view(row: &tokio_postgres::Row) -> Result<NativeSession, AuthError> {
    let expires: i64 = row.get("expires");
    Ok(NativeSession {
        actor_id: row.get("actor_id"),
        session_id: row.get("session_id"),
        expires_at: u64::try_from(expires).map_err(|_| AuthError::Expired)?,
    })
}
impl Store {
    pub(super) async fn native_begin(
        &self,
        config: &OidcConfig,
        flow: &str,
        request: &NativeFlowRequest,
    ) -> Result<(), AuthError> {
        self.connection().await.map_err(unavailable)?.execute(
            "INSERT INTO theatre_native_flows(flow_id,state_hash,nonce,challenge,issuer,expires_at) VALUES($1::text::uuid,$2,$3,$4,$5,clock_timestamp()+interval '5 minutes')",
            &[&flow,&token_hash(&request.state),&request.nonce,&request.code_challenge,&config.issuer]).await.map_err(unavailable)?;
        Ok(())
    }
    pub(super) async fn native_consume(
        &self,
        config: &OidcConfig,
        flow: &str,
        state: &str,
        challenge: &str,
    ) -> Result<String, AuthError> {
        self.connection().await.map_err(unavailable)?.query_opt(
            "UPDATE theatre_native_flows SET consumed=true WHERE flow_id=$1::text::uuid AND state_hash=$2 AND challenge=$3 AND issuer=$4 AND NOT consumed AND NOT completed AND expires_at>clock_timestamp() RETURNING nonce",
            &[&flow,&token_hash(state),&challenge,&config.issuer]).await.map_err(unavailable)?.map(|r| r.get(0)).ok_or(AuthError::State)
    }
    pub(super) async fn native_finish(
        &self,
        config: &OidcConfig,
        flow: &str,
        proof: VerifiedIdentity,
        access: &str,
        token: &str,
        expires: u64,
    ) -> Result<NativeSession, AuthError> {
        if proof.key().issuer() != config.issuer {
            return Err(AuthError::Issuer);
        }
        let candidate = uuid::Uuid::new_v4().to_string();
        let expires = i64::try_from(expires).map_err(|_| AuthError::Expired)?;
        let session = uuid::Uuid::new_v4().to_string();
        let row = self.connection().await.map_err(unavailable)?.query_one(
            "SELECT theatre_native_finish($1::text::uuid,$2,$3,$4,$5,$6,$7,$8,$9::text::uuid) AS actor_id",
            &[&flow,&proof.key().issuer(),&proof.key().subject(),&candidate,&token_hash(token),&proof.digest(),&access,&expires,&session]).await
            .map_err(|e| if e.code().is_some_and(|v| v.code()=="PT401") { AuthError::Expired } else { AuthError::Unavailable })?;
        let actor_id = row.get::<_, Option<String>>(0).ok_or(AuthError::Inactive)?;
        Ok(NativeSession {
            actor_id,
            session_id: session,
            expires_at: expires as u64,
        })
    }
    pub(super) async fn native_credential(
        &self,
        config: &OidcConfig,
        token: &str,
    ) -> Result<Credential, AuthError> {
        let row = self.connection().await.map_err(unavailable)?.query_opt(
            "SELECT n.access_token,n.subject,n.actor_id,n.session_id::text,extract(epoch FROM n.expires_at)::bigint AS expires FROM theatre_native_sessions n JOIN actors a ON a.id=n.actor_id WHERE token_hash=$1 AND n.issuer=$2 AND NOT n.revoked AND n.expires_at>clock_timestamp() AND a.active",
            &[&token_hash(token),&config.issuer]).await.map_err(unavailable)?.ok_or(AuthError::Inactive)?;
        Ok(Credential {
            access_token: row.get("access_token"),
            subject: row.get("subject"),
            view: view(&row)?,
        })
    }
    pub(super) async fn native_admission<F, Fut, T>(
        &self,
        config: &OidcConfig,
        token: &str,
        actor: &str,
        dispatch: F,
    ) -> Result<T, AuthError>
    where
        F: FnOnce(NativeSession) -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let mut client = self.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        tx.query_opt("SELECT session_id FROM theatre_native_sessions WHERE token_hash=$1 AND issuer=$2 AND actor_id=$3 AND NOT revoked FOR SHARE", &[&token_hash(token),&config.issuer,&actor]).await.map_err(unavailable)?.ok_or(AuthError::Inactive)?;
        let active = tx
            .query_one("SELECT listener_lock_actor($1)", &[&actor])
            .await
            .map_err(unavailable)?;
        if active.get::<_, Option<bool>>(0) != Some(true) {
            return Err(AuthError::Inactive);
        }
        let row = tx.query_opt("SELECT actor_id,session_id::text,extract(epoch FROM expires_at)::bigint AS expires FROM theatre_native_sessions WHERE token_hash=$1 AND NOT revoked AND expires_at>clock_timestamp()", &[&token_hash(token)]).await.map_err(unavailable)?.ok_or(AuthError::Inactive)?;
        let output = dispatch(view(&row)?).await;
        tx.commit().await.map_err(unavailable)?;
        Ok(output)
    }
    pub(super) async fn native_revoke(
        &self,
        config: &OidcConfig,
        token: &str,
    ) -> Result<Option<String>, AuthError> {
        // The UPDATE commits local revocation before any remote request; replays are safe.
        Ok(self.connection().await.map_err(unavailable)?.query_opt("UPDATE theatre_native_sessions SET revoked=true,revocation_pending=true WHERE token_hash=$1 AND issuer=$2 RETURNING access_token", &[&token_hash(token),&config.issuer]).await.map_err(unavailable)?.map(|r|r.get(0)))
    }
    pub(super) async fn native_revocation_completed(&self, token: &str) -> Result<(), AuthError> {
        self.connection()
            .await
            .map_err(unavailable)?
            .execute(
                "UPDATE theatre_native_sessions SET revocation_pending=false WHERE token_hash=$1",
                &[&token_hash(token)],
            )
            .await
            .map_err(unavailable)?;
        Ok(())
    }
}
