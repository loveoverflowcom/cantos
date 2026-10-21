//! Durable sessions for verified Identity facts. No network I/O ever happens inside a
//! transaction here: Identity is consulted before and after, never while a lock is held.
use super::{
    credential::{Channel, Secrets, Surface},
    decision::{Epoch, ObservedSecurity, RevokeReason, Subject},
};
use crate::platform::{
    bearer::core::{self, Lifetime, Rotation},
    postgres::{token_hash, Store, StoreError, Tx},
};
use tokio_postgres::{Row, Transaction};

/// Closed storage outcome. Mapped to the generic wire failures at the HTTP boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionError {
    /// Unknown, revoked, spent, wrong-channel or inactive-actor credential.
    Inactive,
    Expired,
    /// A spent refresh secret was presented: the whole family is now revoked.
    Replay,
    Unavailable,
}

impl From<StoreError> for SessionError {
    fn from(_: StoreError) -> Self {
        Self::Unavailable
    }
}

fn unavailable(_: impl std::fmt::Debug) -> SessionError {
    SessionError::Unavailable
}

async fn clock(tx: &Transaction<'_>) -> Result<u64, SessionError> {
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

fn seconds(row: &Row, name: &str) -> Result<u64, SessionError> {
    u64::try_from(row.get::<_, i64>(name)).map_err(unavailable)
}

fn channel_of(stored: &str) -> Result<Channel, SessionError> {
    match stored {
        "theatre-web" => Ok(Channel::Web(Surface::Theatre)),
        "studio-web" => Ok(Channel::Web(Surface::Studio)),
        "theatre-native" => Ok(Channel::Native),
        _ => Err(SessionError::Unavailable),
    }
}

/// A live session as stored. Secret material never appears here.
#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub session_id: String,
    pub actor_id: String,
    pub channel: Channel,
    pub observed: ObservedSecurity,
    pub authenticated_at: u64,
    pub absolute_expires: u64,
    pub idle_expires: u64,
    /// Expiry of the presented access/cookie secret.
    pub token_expires: u64,
}

fn record(row: &Row) -> Result<SessionRecord, SessionError> {
    let subject =
        Subject::from_hex(&row.get::<_, String>("subject")).ok_or(SessionError::Unavailable)?;
    let epoch = Epoch::from_slice(&row.get::<_, Vec<u8>>("observed_instance_epoch"))
        .ok_or(SessionError::Unavailable)?;
    Ok(SessionRecord {
        session_id: row.get("id"),
        actor_id: row.get("actor_id"),
        channel: channel_of(&row.get::<_, String>("channel"))?,
        observed: ObservedSecurity {
            subject,
            epoch,
            version: seconds(row, "observed_security_version")?,
        },
        authenticated_at: seconds(row, "authenticated_at")?,
        absolute_expires: seconds(row, "absolute_expires")?,
        idle_expires: seconds(row, "idle_expires")?,
        token_expires: seconds(row, "token_expires")?,
    })
}

const ACCESS: &str = "SELECT s.id::text AS id,s.actor_id,s.channel,s.subject,s.observed_instance_epoch,\
s.observed_security_version,s.authenticated_at,s.absolute_expires,s.idle_expires,t.expires_at AS token_expires \
FROM identity_session_tokens t JOIN identity_sessions s ON s.id=t.session_id JOIN actors a ON a.id=s.actor_id \
WHERE t.access_hash=$1 AND s.channel=$2 AND NOT s.revoked AND NOT t.used AND a.active \
AND t.expires_at>extract(epoch FROM clock_timestamp()) AND s.idle_expires>extract(epoch FROM clock_timestamp()) \
AND s.absolute_expires>extract(epoch FROM clock_timestamp())";

pub struct Issued {
    pub actor_id: String,
    pub session_id: String,
    pub lifetime: Lifetime,
}

/// Binds the exact Identity subject to its ActorId (idempotent, never by email) and writes the
/// session in one transaction. `observed` is what a fresh `GetSecurityState` confirmed.
pub(super) async fn issue(
    store: &Store,
    channel: Channel,
    observed: &ObservedSecurity,
    authenticated_at: u64,
    secrets: &Secrets,
) -> Result<Issued, SessionError> {
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let subject = observed.subject.hex();
    let candidate = uuid::Uuid::new_v4().to_string();
    let actor: String = tx
        .query_one(
            "SELECT identity_session_register($1,$2)",
            &[&subject, &candidate],
        )
        .await
        .map_err(unavailable)?
        .get::<_, Option<String>>(0)
        .ok_or(SessionError::Inactive)?;
    let now = clock(&tx).await?;
    // Identity carries no upstream authority expiry, so only this server's policy bounds it.
    let lifetime = core::issue(channel.policy_client().policy(), now, u64::MAX, u64::MAX)
        .map_err(|_| SessionError::Expired)?;
    let id = uuid::Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO identity_sessions(id,actor_id,channel,issuer,subject,assurance,authenticated_at,\
         observed_security_version,observed_instance_epoch,absolute_expires,idle_expires) \
         VALUES($1::text::uuid,$2,$3,'cantos-identity',$4,'password',$5,$6,$7,$8,$9)",
        &[
            &id,
            &actor,
            &channel.stored(),
            &subject,
            &(authenticated_at.max(1) as i64),
            &(observed.version as i64),
            &observed.epoch.bytes().to_vec(),
            &(lifetime.absolute as i64),
            &(lifetime.idle as i64),
        ],
    )
    .await
    .map_err(unavailable)?;
    insert_token(&tx, &id, channel, secrets, lifetime).await?;
    // Recheck after every lock/trigger wait: a late commit never issues a dead credential.
    if clock(&tx).await? >= token_expiry(channel, lifetime) {
        return Err(SessionError::Expired);
    }
    tx.commit().await.map_err(unavailable)?;
    Ok(Issued {
        actor_id: actor,
        session_id: id,
        lifetime,
    })
}

/// Web: the cookie lives until the idle deadline (renewal extends it). Native: the access
/// secret lives for the access window and the refresh secret carries the idle deadline.
fn token_expiry(channel: Channel, lifetime: Lifetime) -> u64 {
    match channel {
        Channel::Web(_) => lifetime.idle,
        Channel::Native => lifetime.access,
    }
}

async fn insert_token(
    tx: &Transaction<'_>,
    session: &str,
    channel: Channel,
    secrets: &Secrets,
    lifetime: Lifetime,
) -> Result<(), SessionError> {
    let refresh = secrets.refresh.as_deref().map(token_hash);
    tx.execute(
        "INSERT INTO identity_session_tokens(access_hash,refresh_hash,session_id,expires_at) \
         VALUES($1,$2,$3::text::uuid,$4)",
        &[
            &token_hash(&secrets.access),
            &refresh,
            &session,
            &(token_expiry(channel, lifetime) as i64),
        ],
    )
    .await
    .map_err(unavailable)?;
    Ok(())
}

/// Idempotent exact-subject → ActorId binding (never by email); no session is created.
pub(super) async fn bind(store: &Store, subject: Subject) -> Result<String, SessionError> {
    store
        .connection()
        .await?
        .query_one(
            "SELECT identity_session_register($1,$2)",
            &[&subject.hex(), &uuid::Uuid::new_v4().to_string()],
        )
        .await
        .map_err(unavailable)?
        .get::<_, Option<String>>(0)
        .ok_or(SessionError::Inactive)
}

/// Read-only lookup of the live session behind an access/cookie secret of exactly `channel`.
pub(super) async fn access(
    store: &Store,
    token: &str,
    channel: Channel,
) -> Result<SessionRecord, SessionError> {
    let connection = store.connection().await?;
    let row = connection
        .query_opt(ACCESS, &[&token_hash(token), &channel.stored()])
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Inactive)?;
    record(&row)
}

/// Admission lease: the session family and actor stay share-locked until `dispatch` finishes, so
/// logout/disable linearize after admitted work. The caller already consulted Identity; no
/// network runs under this lock.
pub(super) async fn admit<F, Fut, T>(
    store: &Store,
    token: &str,
    channel: Channel,
    dispatch: F,
) -> Result<T, SessionError>
where
    F: FnOnce(SessionRecord) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let row = tx
        .query_opt(
            &format!("{ACCESS} FOR SHARE OF s"),
            &[&token_hash(token), &channel.stored()],
        )
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Inactive)?;
    lock_actor(&tx, &row.get::<_, String>("actor_id"), channel.surface()).await?;
    // Re-read volatile expiry/used state after any lock wait, then hold the lease.
    let row = tx
        .query_opt(ACCESS, &[&token_hash(token), &channel.stored()])
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Inactive)?;
    let output = dispatch(record(&row)?).await;
    tx.commit().await.map_err(unavailable)?;
    Ok(output)
}

async fn lock_actor(tx: &Tx<'_>, actor: &str, surface: Surface) -> Result<(), SessionError> {
    let sql = match surface {
        Surface::Studio => "SELECT production_lock_actor($1)",
        Surface::Theatre => "SELECT listener_lock_actor($1)",
    };
    match tx
        .query_one(sql, &[&actor])
        .await
        .map_err(unavailable)?
        .get::<_, Option<bool>>(0)
    {
        Some(true) => Ok(()),
        _ => Err(SessionError::Inactive),
    }
}

/// Studio data operations authenticate inside their own transaction with this call, which locks
/// the family and actor for it exactly like the other Studio credential kinds.
pub(crate) async fn admit_studio(tx: &Tx<'_>, token: &str) -> Result<String, StoreError> {
    let channel = Channel::Web(Surface::Studio);
    let row = tx
        .query_opt(
            &format!("{ACCESS} FOR SHARE OF s"),
            &[&token_hash(token), &channel.stored()],
        )
        .await?
        .ok_or(StoreError::Unauthenticated)?;
    let actor: String = row.get("actor_id");
    lock_actor(tx, &actor, Surface::Studio)
        .await
        .map_err(|_| StoreError::Unauthenticated)?;
    tx.query_opt(ACCESS, &[&token_hash(token), &channel.stored()])
        .await?
        .ok_or(StoreError::Unauthenticated)?;
    Ok(actor)
}

/// Public correlation only: knowing this UUID authenticates nobody.
pub(crate) async fn studio_session_id(
    store: &Store,
    token: &str,
) -> Result<Option<String>, StoreError> {
    let connection = store.connection().await?;
    Ok(connection
        .query_opt(
            "SELECT s.id::text FROM identity_session_tokens t JOIN identity_sessions s ON s.id=t.session_id \
             WHERE t.access_hash=$1 AND s.channel='studio-web'",
            &[&token_hash(token)],
        )
        .await?
        .map(|row| row.get(0)))
}

/// The nonsecret session id behind an access/cookie secret of exactly `channel`, even once the
/// secret expired or the session was revoked. A fence input only; knowing it authenticates nobody.
pub(super) async fn session_id_of(
    store: &Store,
    token: &str,
    channel: Channel,
) -> Result<Option<String>, SessionError> {
    Ok(store
        .connection()
        .await?
        .query_opt(
            "SELECT s.id::text FROM identity_session_tokens t JOIN identity_sessions s ON s.id=t.session_id \
             WHERE t.access_hash=$1 AND s.channel=$2",
            &[&token_hash(token), &channel.stored()],
        )
        .await
        .map_err(unavailable)?
        .map(|row| row.get(0)))
}

/// Durable, irreversible, idempotent revocation after a non-admit security decision.
pub(super) async fn revoke(
    store: &Store,
    session_id: &str,
    reason: RevokeReason,
) -> Result<(), SessionError> {
    store
        .connection()
        .await?
        .execute(
            "UPDATE identity_sessions SET revoked=true,revoked_reason=$2 WHERE id=$1::text::uuid AND NOT revoked",
            &[&session_id, &reason.stored()],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}

/// Logout by the secret itself, even if that secret has expired or been spent: a client must
/// always be able to end its own family. Wrong-channel secrets select nothing.
pub(super) async fn logout(
    store: &Store,
    token: &str,
    channel: Channel,
) -> Result<(), SessionError> {
    store
        .connection()
        .await?
        .execute(
            "UPDATE identity_sessions s SET revoked=true,revoked_reason='logout' FROM identity_session_tokens t \
             WHERE t.session_id=s.id AND t.access_hash=$1 AND s.channel=$2 AND NOT s.revoked",
            &[&token_hash(token), &channel.stored()],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}

/// Refresh cleanup by the (possibly spent) refresh secret and its session id.
pub(super) async fn revoke_refresh(
    store: &Store,
    refresh: &str,
    session_id: &str,
) -> Result<(), SessionError> {
    store
        .connection()
        .await?
        .execute(
            "UPDATE identity_sessions s SET revoked=true,revoked_reason='logout' FROM identity_session_tokens t \
             WHERE t.session_id=s.id AND t.refresh_hash=$1 AND s.id=$2::text::uuid AND s.channel='theatre-native' AND NOT s.revoked",
            &[&token_hash(refresh), &session_id],
        )
        .await
        .map_err(unavailable)?;
    Ok(())
}

const REFRESH_LOCK: &str = "SELECT s.id::text FROM identity_session_tokens t JOIN identity_sessions s ON s.id=t.session_id \
WHERE t.refresh_hash=$1 AND s.id=$2::text::uuid AND s.channel='theatre-native' FOR UPDATE OF s";
const REFRESH_ROW: &str = "SELECT s.id::text AS id,s.actor_id,s.channel,s.subject,s.observed_instance_epoch,\
s.observed_security_version,s.authenticated_at,s.absolute_expires,s.idle_expires,t.expires_at AS token_expires,\
s.revoked,t.used FROM identity_session_tokens t JOIN identity_sessions s ON s.id=t.session_id \
WHERE t.refresh_hash=$1 AND s.id=$2::text::uuid AND s.channel='theatre-native'";

async fn lock_refresh(
    tx: &Transaction<'_>,
    refresh: &str,
    session_id: &str,
) -> Result<Row, SessionError> {
    let parameters: &[&(dyn tokio_postgres::types::ToSql + Sync)] =
        &[&token_hash(refresh), &session_id];
    tx.query_opt(REFRESH_LOCK, parameters)
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Inactive)?;
    // The used flag is read by a NEW statement after the family lock, never from the lock query.
    tx.query_opt(REFRESH_ROW, parameters)
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Inactive)
}

/// First leg of a renewal: contain a replay at once (no Identity needed) and return what the
/// session observed, so the caller can compare it with a fresh read **outside** any transaction.
pub(super) async fn peek_refresh(
    store: &Store,
    refresh: &str,
    session_id: &str,
) -> Result<SessionRecord, SessionError> {
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let row = lock_refresh(&tx, refresh, session_id).await?;
    if row.get::<_, bool>("revoked") {
        return Err(SessionError::Inactive);
    }
    if row.get::<_, bool>("used") {
        tx.execute(
            "UPDATE identity_sessions SET revoked=true,revoked_reason='replay' WHERE id=$1::text::uuid",
            &[&session_id],
        )
        .await
        .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        return Err(SessionError::Replay);
    }
    record(&row)
}

pub struct Rotated {
    pub actor_id: String,
    pub lifetime: Lifetime,
    pub authenticated_at: u64,
}

/// Second leg: spend the refresh secret exactly once and issue the next pair in one
/// transaction. The family's absolute deadline, ActorId and authentication time never move.
pub(super) async fn rotate(
    store: &Store,
    refresh: &str,
    session_id: &str,
    secrets: &Secrets,
) -> Result<Rotated, SessionError> {
    let channel = Channel::Native;
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let row = lock_refresh(&tx, refresh, session_id).await?;
    let actor: String = row.get("actor_id");
    let live =
        !row.get::<_, bool>("revoked") && lock_actor(&tx, &actor, Surface::Theatre).await.is_ok();
    let lifetime = match core::rotate(
        channel.policy_client().policy(),
        clock(&tx).await?,
        seconds(&row, "absolute_expires")?,
        seconds(&row, "idle_expires")?,
        live,
        row.get("used"),
    ) {
        Rotation::Refuse(core::Refusal::Expired) => return Err(SessionError::Expired),
        Rotation::Refuse(core::Refusal::Inactive) => return Err(SessionError::Inactive),
        Rotation::RevokeFamily => {
            tx.execute(
                "UPDATE identity_sessions SET revoked=true,revoked_reason='replay' WHERE id=$1::text::uuid",
                &[&session_id],
            )
            .await
            .map_err(unavailable)?;
            tx.commit().await.map_err(unavailable)?;
            return Err(SessionError::Replay);
        }
        Rotation::Rotate(lifetime) => lifetime,
    };
    tx.execute(
        "UPDATE identity_session_tokens SET used=true WHERE refresh_hash=$1",
        &[&token_hash(refresh)],
    )
    .await
    .map_err(unavailable)?;
    insert_token(&tx, session_id, channel, secrets, lifetime).await?;
    tx.execute(
        "UPDATE identity_sessions SET idle_expires=$2 WHERE id=$1::text::uuid",
        &[&session_id, &(lifetime.idle as i64)],
    )
    .await
    .map_err(unavailable)?;
    if clock(&tx).await? >= lifetime.access {
        return Err(SessionError::Expired);
    }
    let authenticated_at = seconds(&row, "authenticated_at")?;
    tx.commit().await.map_err(unavailable)?;
    Ok(Rotated {
        actor_id: actor,
        lifetime,
        authenticated_at,
    })
}

/// Web renewal extends the idle deadline of the *same* cookie secret, never past the absolute
/// deadline. There is no rotation to race, so concurrent tabs cannot revoke each other.
pub(super) async fn renew_web(
    store: &Store,
    token: &str,
    channel: Channel,
) -> Result<(SessionRecord, Lifetime), SessionError> {
    let mut connection = store.connection().await?;
    let tx = connection.transaction().await.map_err(unavailable)?;
    let row = tx
        .query_opt(
            &format!("{ACCESS} FOR UPDATE OF s"),
            &[&token_hash(token), &channel.stored()],
        )
        .await
        .map_err(unavailable)?
        .ok_or(SessionError::Inactive)?;
    let current = record(&row)?;
    lock_actor(&tx, &current.actor_id, channel.surface()).await?;
    let lifetime = match core::rotate(
        channel.policy_client().policy(),
        clock(&tx).await?,
        current.absolute_expires,
        current.idle_expires,
        true,
        false,
    ) {
        Rotation::Rotate(lifetime) => lifetime,
        Rotation::Refuse(core::Refusal::Expired) => return Err(SessionError::Expired),
        Rotation::Refuse(core::Refusal::Inactive) | Rotation::RevokeFamily => {
            return Err(SessionError::Inactive)
        }
    };
    tx.execute(
        "UPDATE identity_sessions SET idle_expires=$2 WHERE id=$1::text::uuid",
        &[&current.session_id, &(lifetime.idle as i64)],
    )
    .await
    .map_err(unavailable)?;
    tx.execute(
        "UPDATE identity_session_tokens SET expires_at=$2 WHERE access_hash=$1",
        &[&token_hash(token), &(lifetime.idle as i64)],
    )
    .await
    .map_err(unavailable)?;
    tx.commit().await.map_err(unavailable)?;
    Ok((
        SessionRecord {
            idle_expires: lifetime.idle,
            token_expires: lifetime.idle,
            ..current
        },
        lifetime,
    ))
}
