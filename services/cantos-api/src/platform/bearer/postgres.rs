//! Durable flow consume, atomic refresh rotation and family revocation. No network I/O.
use super::{
    client::Client,
    core::{self, Lifetime, Rotation},
    credential, refusal, Secrets,
};
use crate::platform::{
    oidc::{
        config::OidcConfig,
        core::{pkce_challenge, AuthError},
        provider::Exchange,
    },
    postgres::{token_hash, Store, StoreError, Tx},
};
use cantos_contracts::bearer::{
    ExchangeRequest, FlowRequest, RefreshRequest, Session, TokenPair, TokenType,
};
use tokio_postgres::{Row, Transaction};
fn unavailable(_: impl std::fmt::Debug) -> AuthError {
    AuthError::Unavailable
}
async fn clock(tx: &Transaction<'_>) -> Result<u64, AuthError> {
    let time: i64 = tx
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .await
        .map_err(unavailable)?
        .get(0);
    u64::try_from(time).map_err(unavailable)
}
fn seconds(row: &Row, name: &str) -> Result<u64, AuthError> {
    u64::try_from(row.get::<_, i64>(name)).map_err(unavailable)
}
fn view(row: &Row) -> Result<Session, AuthError> {
    Ok(Session {
        actor_id: row.get("actor_id"),
        session_id: row.get("id"),
        expires_at: seconds(row, "expires_at")?,
        absolute_expires_at: seconds(row, "absolute_expires")?,
        authenticated_at: row
            .get::<_, Option<i64>>("authenticated_at")
            .map(u64::try_from)
            .transpose()
            .map_err(unavailable)?,
    })
}
pub(super) struct CallbackFlow {
    pub id: String,
    pub state: String,
    pub nonce: String,
    pub verifier: String,
}
pub(super) struct Credential {
    pub subject: String,
    pub provider_access: String,
    pub session: Session,
}
/// A durable, locally fenced owner of a provider artifact awaiting revocation.
pub(super) enum Revocation {
    Flow { id: String, access: String },
    Family { id: String, access: String },
}
impl Revocation {
    pub fn access(&self) -> &str {
        match self {
            Self::Flow { access, .. } | Self::Family { access, .. } => access,
        }
    }
}
pub(super) enum Cancellation {
    Complete,
    Pending,
    Revoke(Revocation),
}
const CREDENTIAL: &str = include_str!("credential.sql");
pub(super) async fn begin(
    store: &Store,
    config: &OidcConfig,
    id: &str,
    state: &str,
    verifier: &str,
    redirect: &str,
    request: FlowRequest,
) -> Result<(), AuthError> {
    store.connection().await.map_err(unavailable)?.execute("INSERT INTO bearer_flows(id,client_id,issuer,client_state,challenge,provider_state_hash,nonce,verifier,expires_at,redirect_uri,return_origin) VALUES($1::text::uuid,$2,$3,$4,$5,$6,$7,$8,floor(extract(epoch FROM clock_timestamp()))::bigint+300,$9,$10)",&[&id,&config.client_id,&config.issuer,&request.state,&request.code_challenge,&token_hash(state),&request.nonce,&verifier,&redirect,&config.web_origin]).await.map_err(unavailable)?;
    Ok(())
}
pub(super) async fn consume_callback(
    store: &Store,
    config: &OidcConfig,
    state: &str,
) -> Result<CallbackFlow, AuthError> {
    let row=store.connection().await.map_err(unavailable)?.query_opt("UPDATE bearer_flows SET consumed=true WHERE provider_state_hash=$1 AND client_id=$2 AND issuer=$3 AND redirect_uri=$4 AND return_origin=$5 AND NOT consumed AND NOT cancelled AND NOT completed AND expires_at>extract(epoch FROM clock_timestamp()) RETURNING id::text,client_state,nonce,verifier",&[&token_hash(state),&config.client_id,&config.issuer,&config.redirect,&config.web_origin]).await.map_err(unavailable)?.ok_or(AuthError::State)?;
    Ok(CallbackFlow {
        id: row.get(0),
        state: row.get(1),
        nonce: row.get(2),
        verifier: row.get(3),
    })
}
/// Consumes a pending native flow and returns its nonce and the callback bound at begin.
/// Only callbacks still admitted by the operator can be consumed; the stored URI is never
/// replaced by the current default, so a flow cannot change profile between begin and exchange.
pub(super) async fn consume_native(
    store: &Store,
    config: &OidcConfig,
    id: &str,
    state: &str,
    challenge: &str,
) -> Result<(String, String), AuthError> {
    let admitted = config.admitted_native_redirects();
    store.connection().await.map_err(unavailable)?.query_opt("UPDATE bearer_flows SET consumed=true WHERE id=$1::text::uuid AND client_id=$2 AND issuer=$3 AND client_state=$4 AND challenge=$5 AND redirect_uri=ANY($6::text[]) AND return_origin=$7 AND NOT consumed AND NOT cancelled AND NOT completed AND expires_at>extract(epoch FROM clock_timestamp()) RETURNING nonce,redirect_uri",&[&id,&config.client_id,&config.issuer,&state,&challenge,&admitted,&config.web_origin]).await.map_err(unavailable)?.map(|r|(r.get(0),r.get(1))).ok_or(AuthError::State)
}
pub(super) async fn complete_callback(
    store: &Store,
    config: &OidcConfig,
    id: &str,
    exchange: &Exchange,
    absolute: u64,
    handoff: &str,
    redirect: &str,
) -> Result<(), AuthError> {
    if exchange.proof.key().issuer() != config.issuer {
        return Err(AuthError::Issuer);
    }
    let absolute = i64::try_from(absolute).map_err(unavailable)?;
    let authenticated = exchange
        .proof
        .authenticated_at()
        .map(i64::try_from)
        .transpose()
        .map_err(unavailable)?;
    let count=store.connection().await.map_err(unavailable)?.execute("UPDATE bearer_flows SET handoff_hash=$4,subject=$5,proof_hash=$6,provider_access=$7,authority_expires=$8,authenticated_at=$9,nonce='',verifier='' WHERE id=$1::text::uuid AND client_id=$2 AND issuer=$3 AND consumed AND NOT completed AND NOT cancelled AND handoff_hash IS NULL AND expires_at>extract(epoch FROM clock_timestamp()) AND $8::bigint>extract(epoch FROM clock_timestamp()) AND redirect_uri=$10 AND return_origin=$11",&[&id,&config.client_id,&config.issuer,&token_hash(handoff),&exchange.proof.key().subject(),&exchange.proof.digest(),&exchange.access_token,&absolute,&authenticated,&redirect,&config.web_origin]).await.map_err(unavailable)?;
    if count != 1 {
        return Err(AuthError::State);
    }
    Ok(())
}
pub(super) async fn issue(
    store: &Store,
    config: &OidcConfig,
    client: Client,
    id: &str,
    request: &ExchangeRequest,
    secrets: Secrets,
    redirect: &str,
) -> Result<TokenPair, AuthError> {
    let mut connection = store.connection().await.map_err(unavailable)?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let row=tx.query_opt("SELECT subject,proof_hash,provider_access,authority_expires,authenticated_at,expires_at FROM bearer_flows WHERE id=$1::text::uuid AND client_id=$2 AND issuer=$3 AND client_state=$4 AND challenge=$5 AND consumed AND NOT cancelled AND NOT completed AND handoff_hash=$6 AND redirect_uri=$7 AND return_origin=$8 AND expires_at>extract(epoch FROM clock_timestamp()) FOR UPDATE",&[&id,&config.client_id,&config.issuer,&request.state,&pkce_challenge(&request.code_verifier),&token_hash(if client==Client::TheatreNative{""}else{&request.code}),&redirect,&config.web_origin]).await.map_err(unavailable)?.ok_or(AuthError::State)?;
    let absolute = seconds(&row, "authority_expires")?;
    let lifetime =
        core::issue(client.policy(), clock(&tx).await?, absolute, absolute).map_err(refusal)?;
    let subject: String = row.get("subject");
    let candidate = uuid::Uuid::new_v4().to_string();
    let actor: String = tx
        .query_one(
            "SELECT bearer_register($1::text::uuid,$2,$3,$4)",
            &[&id, &config.issuer, &subject, &candidate],
        )
        .await
        .map_err(unavailable)?
        .get::<_, Option<String>>(0)
        .ok_or(AuthError::Inactive)?;
    tx.execute("INSERT INTO bearer_families(id,actor_id,client_id,issuer,subject,provider_access,proof_hash,absolute_expires,idle_expires,authenticated_at) VALUES($1::text::uuid,$2,$3,$4,$5,$6,$7,$8,$9,$10)",&[&id,&actor,&config.client_id,&config.issuer,&subject,&row.get::<_,String>("provider_access"),&row.get::<_,Vec<u8>>("proof_hash"),&(lifetime.absolute as i64),&(lifetime.idle as i64),&row.get::<_,Option<i64>>("authenticated_at")]).await.map_err(unavailable)?;
    insert_token(&tx, id, &secrets, lifetime.access).await?;
    tx.execute(
        "UPDATE bearer_flows SET completed=true,provider_access=NULL WHERE id=$1::text::uuid",
        &[&id],
    )
    .await
    .map_err(unavailable)?;
    // Recheck after every possible lock/trigger wait. Rollback prevents orphan registry issuance.
    if clock(&tx).await? >= lifetime.access.min(seconds(&row, "expires_at")?) {
        return Err(AuthError::Expired);
    }
    let result = pair(actor, id, lifetime, row.get("authenticated_at"), secrets)?;
    tx.commit().await.map_err(unavailable)?;
    Ok(result)
}
async fn insert_token(
    tx: &Transaction<'_>,
    id: &str,
    secrets: &Secrets,
    expires: u64,
) -> Result<(), AuthError> {
    tx.execute("INSERT INTO bearer_tokens(access_hash,refresh_hash,family_id,expires_at) VALUES($1,$2,$3::text::uuid,$4)",&[&token_hash(&secrets.access),&token_hash(&secrets.refresh),&id,&(expires as i64)]).await.map_err(unavailable)?;
    Ok(())
}
fn pair(
    actor: String,
    id: &str,
    lifetime: Lifetime,
    authenticated: Option<i64>,
    secrets: Secrets,
) -> Result<TokenPair, AuthError> {
    Ok(TokenPair {
        token_type: TokenType::Bearer,
        access_token: secrets.access,
        refresh_token: secrets.refresh,
        actor_id: actor,
        session_id: id.into(),
        expires_at: lifetime.access,
        refresh_expires_at: lifetime.idle,
        absolute_expires_at: lifetime.absolute,
        authenticated_at: authenticated
            .map(u64::try_from)
            .transpose()
            .map_err(unavailable)?,
    })
}
pub(super) async fn credential(
    store: &Store,
    config: &OidcConfig,
    token: &str,
) -> Result<Credential, AuthError> {
    let row = store
        .connection()
        .await
        .map_err(unavailable)?
        .query_opt(
            CREDENTIAL,
            &[&token_hash(token), &config.client_id, &config.issuer],
        )
        .await
        .map_err(unavailable)?
        .ok_or(AuthError::Inactive)?;
    Ok(Credential {
        subject: row.get("subject"),
        provider_access: row.get("provider_access"),
        session: view(&row)?,
    })
}
async fn lock_refresh(
    tx: &Transaction<'_>,
    config: &OidcConfig,
    request: &RefreshRequest,
) -> Result<Row, AuthError> {
    let parameters: &[&(dyn tokio_postgres::types::ToSql + Sync)] = &[
        &token_hash(&request.refresh_token),
        &request.session_id,
        &config.client_id,
        &config.issuer,
    ];
    tx.query_opt("SELECT f.id FROM bearer_tokens t JOIN bearer_families f ON f.id=t.family_id WHERE t.refresh_hash=$1 AND f.id=$2::text::uuid AND f.client_id=$3 AND f.issuer=$4 FOR UPDATE OF f",parameters).await.map_err(unavailable)?.ok_or(AuthError::Inactive)?;
    // A joined, nonlocked token may come from the statement's earlier snapshot. Read spent
    // state in a NEW statement after the family lock, rather than relying on join EPQ behavior.
    tx.query_opt("SELECT f.id::text,f.actor_id,f.subject,f.provider_access,f.absolute_expires,f.idle_expires,f.authenticated_at,f.revoked,t.used,t.expires_at FROM bearer_tokens t JOIN bearer_families f ON f.id=t.family_id WHERE t.refresh_hash=$1 AND f.id=$2::text::uuid AND f.client_id=$3 AND f.issuer=$4",parameters).await.map_err(unavailable)?.ok_or(AuthError::Inactive)
}
async fn actor_active(
    tx: &Transaction<'_>,
    actor: &str,
    client: Client,
) -> Result<bool, AuthError> {
    let sql = if client == Client::Studio {
        "SELECT production_lock_actor($1)"
    } else {
        "SELECT listener_lock_actor($1)"
    };
    Ok(tx
        .query_one(sql, &[&actor])
        .await
        .map_err(unavailable)?
        .get::<_, Option<bool>>(0)
        == Some(true))
}
async fn revoke_family(tx: &Transaction<'_>, id: &str) -> Result<(), AuthError> {
    tx.execute(
        "UPDATE bearer_families SET revoked=true,revocation_pending=true WHERE id=$1::text::uuid",
        &[&id],
    )
    .await
    .map_err(unavailable)?;
    Ok(())
}
pub(super) async fn refresh_credential(
    store: &Store,
    config: &OidcConfig,
    request: &RefreshRequest,
) -> Result<Credential, AuthError> {
    let mut connection = store.connection().await.map_err(unavailable)?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let row = lock_refresh(&tx, config, request).await?;
    if row.get::<_, bool>("revoked") {
        return Err(AuthError::Inactive);
    }
    // Replay revokes even if the provider is down; do not wait for network to contain compromise.
    if row.get::<_, bool>("used") {
        revoke_family(&tx, &request.session_id).await?;
        tx.commit().await.map_err(unavailable)?;
        return Err(AuthError::Replay);
    }
    let result = Credential {
        subject: row.get("subject"),
        provider_access: row.get("provider_access"),
        session: view(&row)?,
    };
    tx.commit().await.map_err(unavailable)?;
    Ok(result)
}
pub(super) async fn rotate(
    store: &Store,
    config: &OidcConfig,
    client: Client,
    request: RefreshRequest,
    secrets: Secrets,
) -> Result<TokenPair, AuthError> {
    let mut connection = store.connection().await.map_err(unavailable)?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let row = lock_refresh(&tx, config, &request).await?;
    let actor: String = row.get("actor_id");
    let active = !row.get::<_, bool>("revoked") && actor_active(&tx, &actor, client).await?;
    let lifetime = match core::rotate(
        client.policy(),
        clock(&tx).await?,
        seconds(&row, "absolute_expires")?,
        seconds(&row, "idle_expires")?,
        active,
        row.get("used"),
    ) {
        Rotation::Refuse(error) => return Err(refusal(error)),
        Rotation::RevokeFamily => {
            revoke_family(&tx, &request.session_id).await?;
            tx.commit().await.map_err(unavailable)?;
            return Err(AuthError::Replay);
        }
        Rotation::Rotate(lifetime) => lifetime,
    };
    tx.execute(
        "UPDATE bearer_tokens SET used=true WHERE refresh_hash=$1",
        &[&token_hash(&request.refresh_token)],
    )
    .await
    .map_err(unavailable)?;
    insert_token(&tx, &request.session_id, &secrets, lifetime.access).await?;
    tx.execute(
        "UPDATE bearer_families SET idle_expires=$2 WHERE id=$1::text::uuid",
        &[&request.session_id, &(lifetime.idle as i64)],
    )
    .await
    .map_err(unavailable)?;
    if clock(&tx).await? >= lifetime.access.min(seconds(&row, "idle_expires")?) {
        return Err(AuthError::Expired);
    }
    let result = pair(
        actor,
        &request.session_id,
        lifetime,
        row.get("authenticated_at"),
        secrets,
    )?;
    tx.commit().await.map_err(unavailable)?;
    Ok(result)
}
pub(super) async fn revoke(
    store: &Store,
    config: &OidcConfig,
    token: &str,
    session: Option<&str>,
    refresh: bool,
) -> Result<Option<Revocation>, AuthError> {
    let mut connection = store.connection().await.map_err(unavailable)?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let sql = if refresh {
        "SELECT f.id::text,f.provider_access FROM bearer_tokens t JOIN bearer_families f ON f.id=t.family_id WHERE t.refresh_hash=$1 AND f.client_id=$2 AND f.issuer=$3 AND f.id=$4::text::uuid FOR UPDATE OF f"
    } else {
        "SELECT f.id::text,f.provider_access FROM bearer_tokens t JOIN bearer_families f ON f.id=t.family_id WHERE t.access_hash=$1 AND f.client_id=$2 AND f.issuer=$3 AND ($4::text IS NULL OR f.id=$4::text::uuid) FOR UPDATE OF f"
    };
    let row = tx
        .query_opt(
            sql,
            &[
                &token_hash(token),
                &config.client_id,
                &config.issuer,
                &session,
            ],
        )
        .await
        .map_err(unavailable)?;
    let result = if let Some(row) = row {
        let id: String = row.get(0);
        revoke_family(&tx, &id).await?;
        Some(Revocation::Family {
            id,
            access: row.get(1),
        })
    } else {
        None
    };
    tx.commit().await.map_err(unavailable)?;
    Ok(result)
}
pub(super) async fn cancel(
    store: &Store,
    config: &OidcConfig,
    flow: &str,
    state: &str,
    challenge: &str,
) -> Result<Cancellation, AuthError> {
    let mut connection = store.connection().await.map_err(unavailable)?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let row=tx.query_opt("UPDATE bearer_flows SET cancelled=true,nonce='',verifier='' WHERE id=$1::text::uuid AND client_id=$2 AND issuer=$3 AND client_state=$4 AND challenge=$5 RETURNING provider_access,consumed AND NOT completed AS uncertain",&[&flow,&config.client_id,&config.issuer,&state,&challenge]).await.map_err(unavailable)?.ok_or(AuthError::State)?;
    // Flow before family matches issuance. A cancelled callback/exchange cannot issue later.
    let receipt = cancel_receipt(&tx, flow, row.get(0)).await?;
    tx.commit().await.map_err(unavailable)?;
    Ok(match receipt {
        Some(receipt) => Cancellation::Revoke(receipt),
        // A consumed exchange can still be in provider I/O. A missing artifact is not a
        // provider acknowledgement; successful token-cleared retries are conservative too.
        None if row.get("uncertain") => Cancellation::Pending,
        None => Cancellation::Complete,
    })
}
/// An exchange may return after cancel/expiry or fail a local authorization check. Keep the
/// verified upstream artifact in the already consumed flow before any best-effort network I/O.
pub(super) async fn abandon_exchange(
    store: &Store,
    config: &OidcConfig,
    flow: &str,
    access: &str,
    redirect: &str,
) -> Result<Option<Revocation>, AuthError> {
    let mut connection = store.connection().await.map_err(unavailable)?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let row=tx.query_opt("UPDATE bearer_flows SET cancelled=true,nonce='',verifier='',provider_access=CASE WHEN completed THEN provider_access ELSE $4 END WHERE id=$1::text::uuid AND client_id=$2 AND issuer=$3 AND redirect_uri=$5 AND return_origin=$6 AND consumed AND (completed OR provider_access IS NULL OR provider_access=$4) RETURNING provider_access",&[&flow,&config.client_id,&config.issuer,&access,&redirect,&config.web_origin]).await.map_err(unavailable)?.ok_or(AuthError::State)?;
    let receipt = cancel_receipt(&tx, flow, row.get(0)).await?;
    tx.commit().await.map_err(unavailable)?;
    Ok(receipt)
}
async fn cancel_receipt(
    tx: &Transaction<'_>,
    id: &str,
    flow_access: Option<String>,
) -> Result<Option<Revocation>, AuthError> {
    // Do not turn an acknowledged provider revoke back into pending on a repeated cancel.
    let family = tx.query_opt(
        "UPDATE bearer_families SET revoked=true,revocation_pending=true WHERE id=$1::text::uuid AND (NOT revoked OR revocation_pending) RETURNING provider_access",
        &[&id],
    ).await.map_err(unavailable)?;
    Ok(if let Some(row) = family {
        Some(Revocation::Family {
            id: id.into(),
            access: row.get(0),
        })
    } else {
        flow_access.map(|access| Revocation::Flow {
            id: id.into(),
            access,
        })
    })
}
pub(super) async fn revocation_completed(
    store: &Store,
    receipt: &Revocation,
) -> Result<(), AuthError> {
    let (id, access, sql) = match receipt {
        Revocation::Flow { id, access } => (id, access,
            "UPDATE bearer_flows SET provider_access=NULL WHERE id=$1::text::uuid AND cancelled AND provider_access=$2"),
        Revocation::Family { id, access } => (id, access,
            "UPDATE bearer_families SET revocation_pending=false WHERE id=$1::text::uuid AND revoked AND provider_access=$2"),
    };
    store
        .connection()
        .await
        .map_err(unavailable)?
        .execute(sql, &[&id, &access])
        .await
        .map_err(unavailable)?;
    Ok(())
}
pub(super) async fn admission<F, Fut, T>(
    store: &Store,
    config: &OidcConfig,
    token: &str,
    dispatch: F,
) -> Result<T, AuthError>
where
    F: FnOnce(Session) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let mut connection = store.connection().await.map_err(unavailable)?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let row = tx
        .query_opt(
            &format!("{CREDENTIAL} FOR SHARE OF f"),
            &[&token_hash(token), &config.client_id, &config.issuer],
        )
        .await
        .map_err(unavailable)?
        .ok_or(AuthError::Inactive)?;
    let actor: String = row.get("actor_id");
    let client = credential::access_client(token).ok_or(AuthError::InvalidToken)?;
    if !actor_active(&tx, &actor, client).await? {
        return Err(AuthError::Inactive);
    }
    // Re-read volatile expiry and used state after lock waits, then admit until dispatch finishes.
    let row = tx
        .query_opt(
            CREDENTIAL,
            &[&token_hash(token), &config.client_id, &config.issuer],
        )
        .await
        .map_err(unavailable)?
        .ok_or(AuthError::Inactive)?;
    let output = dispatch(view(&row)?).await;
    tx.commit().await.map_err(unavailable)?;
    Ok(output)
}
pub(crate) async fn admit_studio(tx: &Tx<'_>, token: &str) -> Result<String, StoreError> {
    let row=tx.query_opt("SELECT f.actor_id FROM bearer_tokens t JOIN bearer_families f ON f.id=t.family_id WHERE t.access_hash=$1 AND f.client_id='cantos-studio-web-bff' AND NOT f.revoked AND NOT t.used AND t.expires_at>extract(epoch FROM clock_timestamp()) AND f.absolute_expires>extract(epoch FROM clock_timestamp()) FOR SHARE OF f",&[&token_hash(token)]).await?.ok_or(StoreError::Unauthenticated)?;
    let actor: String = row.get(0);
    let active: Option<bool> = tx
        .query_one("SELECT production_lock_actor($1)", &[&actor])
        .await?
        .get(0);
    if active != Some(true) {
        return Err(StoreError::Unauthenticated);
    }
    let valid:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM bearer_tokens t JOIN bearer_families f ON f.id=t.family_id WHERE t.access_hash=$1 AND NOT t.used AND NOT f.revoked AND t.expires_at>extract(epoch FROM clock_timestamp()) AND f.absolute_expires>extract(epoch FROM clock_timestamp()))",&[&token_hash(token)]).await?.get(0);
    if !valid {
        return Err(StoreError::Unauthenticated);
    }
    Ok(actor)
}

pub(super) async fn reject_callback(
    store: &Store,
    config: &OidcConfig,
    state: &str,
) -> Result<(String, String), AuthError> {
    let row=store.connection().await.map_err(unavailable)?.query_opt("UPDATE bearer_flows SET consumed=true,cancelled=true WHERE provider_state_hash=$1 AND client_id=$2 AND issuer=$3 AND redirect_uri=$4 AND return_origin=$5 AND NOT consumed AND NOT cancelled AND NOT completed AND expires_at>extract(epoch FROM clock_timestamp()) RETURNING id::text,client_state",&[&token_hash(state),&config.client_id,&config.issuer,&config.redirect,&config.web_origin]).await.map_err(unavailable)?.ok_or(AuthError::State)?;
    Ok((row.get(0), row.get(1)))
}
