//! Turso adapter of the provider-free Identity sessions over the shared `server.db` owner
//! (#333, #335): the one `SessionStore` the composed Server serves from.
//!
//! Each mutation reads its facts and writes inside one `BEGIN IMMEDIATE` on the owner's
//! serialized writer. That replaces PostgreSQL's family/actor `FOR UPDATE`/`FOR SHARE` locks and
//! the registry's advisory lock by ordering; guarded statements still check their affected rows,
//! and a rejected statement rolls the whole unit back. Time is the trusted clock in whole Unix
//! seconds, the unit of `SessionPolicy`: a write uses its transaction's stamp, taken after the
//! writer is held, and a read takes one clock reading. Nothing here holds the writer while
//! Identity is asked, and nothing retries: a lost COMMIT acknowledgement is
//! [`SessionError::Unknown`].
//!
//! PostgreSQL's admission lease keeps the family and actor share-locked through `dispatch`. It is
//! deliberately **not** translated into a writer held across a callback that could ask for the
//! writer again. Its port is [`admit_in`]: a business write re-admits the session inside its own
//! transaction, so a logout or deactivation either committed before it (and the write is
//! refused) or waits for it. A read-only dispatch is admitted at its reader snapshot only.
use cantos_store_turso::{
    error::CommitFault,
    params_from_iter,
    value::{integer_to_u64, u64_to_integer, UnixMicros},
    Row, Rows, Value, WriteTransaction,
};

use super::{
    credential::{Channel, Secrets, Surface},
    decision::{Epoch, ObservedSecurity, RevokeReason, Subject},
    session::{
        channel_of, digest, token_expiry, Issued, Rotated, SessionError, SessionRecord,
        SessionStore, ISSUER,
    },
};
use crate::platform::{
    bearer::core::{self, Lifetime, Rotation},
    identity::{
        domain::{ActorId, ExternalIdentityKey, IdentityError},
        turso::{register_in, Effect},
        RegistryError,
    },
    store::{Store, StoreError, Tx},
    turso::{Acknowledgement, ServerDatabase},
};

const COLUMNS: &str = "s.id,s.actor_id,s.channel,s.subject,s.observed_instance_epoch,\
s.observed_security_version,s.authenticated_at,s.absolute_expires,s.idle_expires,t.expires_at";

/// The live session behind an access/cookie digest of exactly one channel at second `?3`.
fn live() -> String {
    format!(
        "SELECT {COLUMNS} FROM identity_session_tokens t JOIN identity_sessions s ON s.id=t.session_id \
         JOIN actors a ON a.id=s.actor_id WHERE t.access_hash=?1 AND s.channel=?2 AND s.revoked=0 \
         AND t.used=0 AND a.active=1 AND t.expires_at>?3 AND s.idle_expires>?3 AND s.absolute_expires>?3"
    )
}

/// The native family behind a refresh digest and its session id, whatever its state.
fn refresh_row() -> String {
    format!(
        "SELECT {COLUMNS},s.revoked,t.used FROM identity_session_tokens t \
         JOIN identity_sessions s ON s.id=t.session_id \
         WHERE t.refresh_hash=?1 AND s.id=?2 AND s.channel='theatre-native'"
    )
}

#[derive(Clone)]
pub struct TursoSessions {
    database: ServerDatabase,
    acknowledgement: Acknowledgement,
}

/// What a unit of work decided. An `Err` from the unit rolls it back; `Commit` makes its writes
/// durable before answering, and that answer may itself be a refusal (a contained replay).
enum Decided<T> {
    ReadOnly(T),
    Commit(Result<T, SessionError>),
}

impl<T> Decided<T> {
    fn of(value: T, effect: Effect) -> Self {
        match effect {
            Effect::ReadOnly => Self::ReadOnly(value),
            Effect::Written => Self::Commit(Ok(value)),
        }
    }
}

impl TursoSessions {
    pub fn new(database: ServerDatabase) -> Self {
        Self {
            database,
            acknowledgement: Acknowledgement::Engine,
        }
    }

    #[cfg(test)]
    fn with_acknowledgement(database: ServerDatabase, acknowledgement: Acknowledgement) -> Self {
        Self {
            database,
            acknowledgement,
        }
    }

    async fn begin(&self) -> Result<WriteTransaction<'_>, SessionError> {
        self.database.owner().begin_write().await.map_err(storage)
    }

    async fn finish<T>(
        &self,
        tx: WriteTransaction<'_>,
        decided: Result<Decided<T>, SessionError>,
    ) -> Result<T, SessionError> {
        match decided {
            Ok(Decided::Commit(answer)) => {
                self.acknowledgement
                    .commit(tx)
                    .await
                    .map_err(|fault| match fault {
                        CommitFault::Rejected(_) => SessionError::Unavailable,
                        CommitFault::Unknown(_) => SessionError::Unknown,
                    })?;
                answer
            }
            Ok(Decided::ReadOnly(value)) => {
                tx.rollback().await.map_err(storage)?;
                Ok(value)
            }
            Err(error) => {
                tx.rollback().await.map_err(storage)?;
                Err(error)
            }
        }
    }

    /// One trusted-clock reading for a decision taken on a reader.
    fn now(&self) -> Result<u64, SessionError> {
        seconds(self.database.now().map_err(|_| SessionError::Unavailable)?)
    }
}

/// Every engine fault, corruption included, fails closed; a corrupt row is never served.
fn storage(_: cantos_store_turso::error::StoreFault) -> SessionError {
    SessionError::Unavailable
}

fn corrupt<E>(_: E) -> SessionError {
    SessionError::Unavailable
}

/// Whole seconds of a trusted timestamp. `UnixMicros` is never negative, and the conversion is
/// still checked rather than cast.
fn seconds(at: UnixMicros) -> Result<u64, SessionError> {
    u64::try_from(at.get().div_euclid(1_000_000)).map_err(corrupt)
}

fn integer(value: u64) -> Result<Value, SessionError> {
    u64_to_integer(value).map(Value::Integer).map_err(corrupt)
}

fn text(row: &Row, index: usize) -> Result<String, SessionError> {
    match row.get_value(index) {
        Ok(Value::Text(value)) => Ok(value),
        _ => Err(SessionError::Unavailable),
    }
}

fn blob(row: &Row, index: usize) -> Result<Vec<u8>, SessionError> {
    match row.get_value(index) {
        Ok(Value::Blob(value)) => Ok(value),
        _ => Err(SessionError::Unavailable),
    }
}

fn unsigned(row: &Row, index: usize) -> Result<u64, SessionError> {
    match row.get_value(index) {
        Ok(Value::Integer(value)) => integer_to_u64(value).map_err(corrupt),
        _ => Err(SessionError::Unavailable),
    }
}

fn positive(row: &Row, index: usize) -> Result<u64, SessionError> {
    match unsigned(row, index)? {
        0 => Err(SessionError::Unavailable),
        value => Ok(value),
    }
}

fn flag(row: &Row, index: usize) -> Result<bool, SessionError> {
    match row.get_value(index) {
        Ok(Value::Integer(0)) => Ok(false),
        Ok(Value::Integer(1)) => Ok(true),
        _ => Err(SessionError::Unavailable),
    }
}

/// The canonical spelling PostgreSQL's `uuid` cast compared. Every spelling both the shell's
/// `Uuid::parse_str` and that cast admit maps to it, so a client's spelling never selects another
/// family; the `urn:uuid:` form, which the cast refuses, names none.
fn session_key(value: &str) -> Option<String> {
    if value.len() >= 4 && value[..4].eq_ignore_ascii_case("urn:") {
        return None;
    }
    uuid::Uuid::parse_str(value)
        .ok()
        .map(|id| id.hyphenated().to_string())
}

fn stored_session_id(row: &Row, index: usize) -> Result<String, SessionError> {
    let stored = text(row, index)?;
    match session_key(&stored) {
        Some(canonical) if canonical == stored => Ok(stored),
        _ => Err(SessionError::Unavailable),
    }
}

/// Checked hydration of `COLUMNS`: every value is re-admitted through its production type, so a
/// row written outside this adapter cannot become a session.
fn record(row: &Row) -> Result<SessionRecord, SessionError> {
    let record = SessionRecord {
        session_id: stored_session_id(row, 0)?,
        actor_id: ActorId::new(text(row, 1)?).map_err(corrupt)?.into(),
        channel: channel_of(&text(row, 2)?).ok_or(SessionError::Unavailable)?,
        observed: ObservedSecurity {
            subject: Subject::from_hex(&text(row, 3)?).ok_or(SessionError::Unavailable)?,
            epoch: Epoch::from_slice(&blob(row, 4)?).ok_or(SessionError::Unavailable)?,
            version: positive(row, 5)?,
        },
        authenticated_at: positive(row, 6)?,
        absolute_expires: unsigned(row, 7)?,
        idle_expires: unsigned(row, 8)?,
        token_expires: unsigned(row, 9)?,
    };
    if record.idle_expires > record.absolute_expires {
        return Err(SessionError::Unavailable);
    }
    Ok(record)
}

/// `COLUMNS` plus `(revoked, used)` of `refresh_row`.
fn refresh_record(row: &Row) -> Result<(SessionRecord, bool, bool), SessionError> {
    let current = record(row)?;
    if current.channel != Channel::Native {
        return Err(SessionError::Unavailable);
    }
    Ok((current, flag(row, 10)?, flag(row, 11)?))
}

/// At most one row: every predicate names a primary or unique key, so a second row is corrupt.
async fn at_most_one<T>(
    mut rows: Rows,
    decode: impl Fn(&Row) -> Result<T, SessionError>,
) -> Result<Option<T>, SessionError> {
    let Some(row) = rows.next().await.map_err(storage)? else {
        return Ok(None);
    };
    let value = decode(&row)?;
    if rows.next().await.map_err(storage)?.is_some() {
        return Err(SessionError::Unavailable);
    }
    Ok(Some(value))
}

fn access_params(token: &str, channel: Channel, now: u64) -> Result<Vec<Value>, SessionError> {
    Ok(vec![
        Value::Blob(digest(token).to_vec()),
        Value::Text(channel.stored().to_owned()),
        integer(now)?,
    ])
}

async fn live_in(
    tx: &WriteTransaction<'_>,
    token: &str,
    channel: Channel,
    now: u64,
) -> Result<Option<SessionRecord>, SessionError> {
    let rows = tx
        .query(
            &live(),
            params_from_iter(access_params(token, channel, now)?),
        )
        .await
        .map_err(storage)?;
    at_most_one(rows, record).await
}

async fn refresh_in(
    tx: &WriteTransaction<'_>,
    refresh: &str,
    session_id: &str,
) -> Result<Option<(SessionRecord, bool, bool)>, SessionError> {
    let rows = tx
        .query(
            &refresh_row(),
            params_from_iter([
                Value::Blob(digest(refresh).to_vec()),
                Value::Text(session_id.to_owned()),
            ]),
        )
        .await
        .map_err(storage)?;
    at_most_one(rows, refresh_record).await
}

/// Exact `cantos-identity`/subject → ActorId through the Actor Registry's own transaction-aware
/// primitive, inside the caller's unit of work. An inactive actor issues nothing.
async fn bind_in(
    tx: &WriteTransaction<'_>,
    subject: Subject,
) -> Result<(ActorId, Effect), SessionError> {
    let key = ExternalIdentityKey::new(ISSUER, subject.hex()).map_err(corrupt)?;
    let candidate = ActorId::new(uuid::Uuid::new_v4().to_string()).map_err(corrupt)?;
    match register_in(tx, &key, &candidate).await {
        Ok((registration, effect)) => Ok((registration.actor_id().clone(), effect)),
        Err(RegistryError::Rejected(IdentityError::ActorInactive)) => Err(SessionError::Inactive),
        Err(_) => Err(SessionError::Unavailable),
    }
}

/// Exactly one row must change; anything else is a concurrent or corrupt state, never success.
fn one(changed: u64) -> Result<(), SessionError> {
    match changed {
        1 => Ok(()),
        _ => Err(SessionError::Unavailable),
    }
}

async fn insert_token(
    tx: &WriteTransaction<'_>,
    session: &str,
    channel: Channel,
    secrets: &Secrets,
    lifetime: Lifetime,
) -> Result<(), SessionError> {
    let refresh = match &secrets.refresh {
        Some(refresh) => Value::Blob(digest(refresh).to_vec()),
        None => Value::Null,
    };
    let changed = tx
        .execute(
            "INSERT INTO identity_session_tokens(access_hash,refresh_hash,session_id,expires_at) \
             VALUES(?1,?2,?3,?4)",
            params_from_iter([
                Value::Blob(digest(&secrets.access).to_vec()),
                refresh,
                Value::Text(session.to_owned()),
                integer(token_expiry(channel, lifetime))?,
            ]),
        )
        .await
        .map_err(storage)?;
    one(changed)
}

async fn issue_in(
    tx: &WriteTransaction<'_>,
    channel: Channel,
    observed: &ObservedSecurity,
    authenticated_at: u64,
    secrets: &Secrets,
) -> Result<Issued, SessionError> {
    let (actor, _) = bind_in(tx, observed.subject).await?;
    let now = seconds(tx.now())?;
    // Identity carries no upstream authority expiry, so only this server's policy bounds it. The
    // stamp is taken after the writer is held and nothing waits inside the unit, so PostgreSQL's
    // post-lock clock recheck has no Turso counterpart.
    let lifetime = core::issue(channel.policy_client().policy(), now, u64::MAX, u64::MAX)
        .map_err(|_| SessionError::Expired)?;
    let id = uuid::Uuid::new_v4().hyphenated().to_string();
    let changed = tx
        .execute(
            "INSERT INTO identity_sessions(id,actor_id,channel,issuer,subject,assurance,\
             authenticated_at,observed_security_version,observed_instance_epoch,absolute_expires,\
             idle_expires) VALUES(?1,?2,?3,'cantos-identity',?4,'password',?5,?6,?7,?8,?9)",
            params_from_iter([
                Value::Text(id.clone()),
                Value::Text(actor.as_str().to_owned()),
                Value::Text(channel.stored().to_owned()),
                Value::Text(observed.subject.hex()),
                integer(authenticated_at.max(1))?,
                integer(observed.version)?,
                Value::Blob(observed.epoch.bytes().to_vec()),
                integer(lifetime.absolute)?,
                integer(lifetime.idle)?,
            ]),
        )
        .await
        .map_err(storage)?;
    one(changed)?;
    insert_token(tx, &id, channel, secrets, lifetime).await?;
    Ok(Issued {
        actor_id: actor.into(),
        session_id: id,
        lifetime,
    })
}

async fn revoke_in(
    tx: &WriteTransaction<'_>,
    session_id: &str,
    reason: &str,
) -> Result<Effect, SessionError> {
    let changed = tx
        .execute(
            "UPDATE identity_sessions SET revoked=1,revoked_reason=?2 WHERE id=?1 AND revoked=0",
            params_from_iter([
                Value::Text(session_id.to_owned()),
                Value::Text(reason.to_owned()),
            ]),
        )
        .await
        .map_err(storage)?;
    match changed {
        0 => Ok(Effect::ReadOnly),
        1 => Ok(Effect::Written),
        _ => Err(SessionError::Unavailable),
    }
}

/// Logout of the family a (possibly expired or spent) secret selects; nothing selected is a
/// no-op, exactly like PostgreSQL's `UPDATE … FROM` that matched no row.
async fn logout_in(
    tx: &WriteTransaction<'_>,
    sql: &str,
    params: Vec<Value>,
) -> Result<Decided<()>, SessionError> {
    let rows = tx
        .query(sql, params_from_iter(params))
        .await
        .map_err(storage)?;
    let Some(session) = at_most_one(rows, |row| stored_session_id(row, 0)).await? else {
        return Ok(Decided::ReadOnly(()));
    };
    Ok(Decided::of((), revoke_in(tx, &session, "logout").await?))
}

async fn peek_in(
    tx: &WriteTransaction<'_>,
    refresh: &str,
    session_id: &str,
) -> Result<Decided<SessionRecord>, SessionError> {
    let (current, revoked, used) = refresh_in(tx, refresh, session_id)
        .await?
        .ok_or(SessionError::Inactive)?;
    if revoked {
        return Err(SessionError::Inactive);
    }
    if used {
        revoke_in(tx, session_id, "replay").await?;
        return Ok(Decided::Commit(Err(SessionError::Replay)));
    }
    Ok(Decided::ReadOnly(current))
}

async fn actor_active(tx: &WriteTransaction<'_>, actor: &str) -> Result<bool, SessionError> {
    let rows = tx
        .query("SELECT active FROM actors WHERE id=?1", [actor])
        .await
        .map_err(storage)?;
    Ok(at_most_one(rows, |row| flag(row, 0))
        .await?
        .unwrap_or(false))
}

async fn rotate_in(
    tx: &WriteTransaction<'_>,
    refresh: &str,
    session_id: &str,
    secrets: &Secrets,
) -> Result<Decided<Rotated>, SessionError> {
    let channel = Channel::Native;
    let (current, revoked, used) = refresh_in(tx, refresh, session_id)
        .await?
        .ok_or(SessionError::Inactive)?;
    let live = !revoked && actor_active(tx, &current.actor_id).await?;
    let lifetime = match core::rotate(
        channel.policy_client().policy(),
        seconds(tx.now())?,
        current.absolute_expires,
        current.idle_expires,
        live,
        used,
    ) {
        Rotation::Refuse(core::Refusal::Expired) => return Err(SessionError::Expired),
        Rotation::Refuse(core::Refusal::Inactive) => return Err(SessionError::Inactive),
        Rotation::RevokeFamily => {
            revoke_in(tx, session_id, "replay").await?;
            return Ok(Decided::Commit(Err(SessionError::Replay)));
        }
        Rotation::Rotate(lifetime) => lifetime,
    };
    let spent = tx
        .execute(
            "UPDATE identity_session_tokens SET used=1 WHERE refresh_hash=?1 AND used=0",
            params_from_iter([Value::Blob(digest(refresh).to_vec())]),
        )
        .await
        .map_err(storage)?;
    one(spent)?;
    insert_token(tx, session_id, channel, secrets, lifetime).await?;
    let renewed = tx
        .execute(
            "UPDATE identity_sessions SET idle_expires=?2 WHERE id=?1 AND revoked=0",
            params_from_iter([Value::Text(session_id.to_owned()), integer(lifetime.idle)?]),
        )
        .await
        .map_err(storage)?;
    one(renewed)?;
    Ok(Decided::Commit(Ok(Rotated {
        actor_id: current.actor_id,
        lifetime,
        authenticated_at: current.authenticated_at,
    })))
}

async fn renew_in(
    tx: &WriteTransaction<'_>,
    token: &str,
    channel: Channel,
) -> Result<Decided<(SessionRecord, Lifetime)>, SessionError> {
    let now = seconds(tx.now())?;
    let current = live_in(tx, token, channel, now)
        .await?
        .ok_or(SessionError::Inactive)?;
    let lifetime = match core::rotate(
        channel.policy_client().policy(),
        now,
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
    let renewed = tx
        .execute(
            "UPDATE identity_sessions SET idle_expires=?2 WHERE id=?1 AND revoked=0",
            params_from_iter([
                Value::Text(current.session_id.clone()),
                integer(lifetime.idle)?,
            ]),
        )
        .await
        .map_err(storage)?;
    one(renewed)?;
    let extended = tx
        .execute(
            "UPDATE identity_session_tokens SET expires_at=?2 WHERE access_hash=?1",
            params_from_iter([Value::Blob(digest(token).to_vec()), integer(lifetime.idle)?]),
        )
        .await
        .map_err(storage)?;
    one(extended)?;
    Ok(Decided::Commit(Ok((
        SessionRecord {
            idle_expires: lifetime.idle,
            token_expires: lifetime.idle,
            ..current
        },
        lifetime,
    ))))
}

/// Admission inside a business write's own transaction: the live session and active actor at the
/// transaction's stamp. The serialized writer orders it against every logout, revocation and
/// deactivation, which replaces PostgreSQL's lease. It never begins, commits or rolls back.
/// On a read snapshot the same check holds at that snapshot only.
pub(crate) async fn admit_in(
    tx: &Tx<'_>,
    token: &str,
    channel: Channel,
) -> Result<SessionRecord, SessionError> {
    let now = u64::try_from(tx.now_seconds()).map_err(corrupt)?;
    let rows = tx
        .raw_query(
            &live(),
            params_from_iter(access_params(token, channel, now)?),
        )
        .await
        .map_err(|_| SessionError::Unavailable)?;
    at_most_one(rows, record)
        .await?
        .ok_or(SessionError::Inactive)
}

/// Studio data operations authenticate inside their own business unit with this call. The family
/// and actor are re-admitted at the unit's own stamp; a logout or deactivation either committed
/// before the unit (and it is refused) or waits behind it on the serialized writer.
pub(crate) async fn admit_studio(tx: &Tx<'_>, token: &str) -> Result<String, StoreError> {
    admit_in(tx, token, Channel::Web(Surface::Studio))
        .await
        .map(|record| record.actor_id)
        .map_err(|error| match error {
            SessionError::Unavailable | SessionError::Unknown => StoreError::Unavailable,
            _ => StoreError::Unauthenticated,
        })
}

/// Public correlation only: knowing this UUID authenticates nobody.
pub(crate) async fn studio_session_id(
    store: &Store,
    token: &str,
) -> Result<Option<String>, StoreError> {
    let reader = store.database().owner().reader().await.map_err(|_| StoreError::Unavailable)?;
    let rows = reader
        .query(
            "SELECT s.id FROM identity_session_tokens t JOIN identity_sessions s ON s.id=t.session_id \
             WHERE t.access_hash=?1 AND s.channel='studio-web'",
            params_from_iter([Value::Blob(digest(token).to_vec())]),
        )
        .await
        .map_err(|_| StoreError::Unavailable)?;
    at_most_one(rows, |row| stored_session_id(row, 0))
        .await
        .map_err(|_| StoreError::Unavailable)
}

/// The composed store is the session port: each call opens the shared `server.db` owner.
impl SessionStore for Store {
    async fn bind(&self, subject: Subject) -> Result<String, SessionError> {
        self.sessions().bind(subject).await
    }

    async fn issue(
        &self,
        channel: Channel,
        observed: &ObservedSecurity,
        authenticated_at: u64,
        secrets: &Secrets,
    ) -> Result<Issued, SessionError> {
        self.sessions()
            .issue(channel, observed, authenticated_at, secrets)
            .await
    }

    async fn access(&self, token: &str, channel: Channel) -> Result<SessionRecord, SessionError> {
        self.sessions().access(token, channel).await
    }

    async fn session_id_of(
        &self,
        token: &str,
        channel: Channel,
    ) -> Result<Option<String>, SessionError> {
        self.sessions().session_id_of(token, channel).await
    }

    async fn revoke(&self, session_id: &str, reason: RevokeReason) -> Result<(), SessionError> {
        self.sessions().revoke(session_id, reason).await
    }

    async fn logout(&self, token: &str, channel: Channel) -> Result<(), SessionError> {
        self.sessions().logout(token, channel).await
    }

    async fn revoke_refresh(&self, refresh: &str, session_id: &str) -> Result<(), SessionError> {
        self.sessions().revoke_refresh(refresh, session_id).await
    }

    async fn peek_refresh(
        &self,
        refresh: &str,
        session_id: &str,
    ) -> Result<SessionRecord, SessionError> {
        self.sessions().peek_refresh(refresh, session_id).await
    }

    async fn rotate(
        &self,
        refresh: &str,
        session_id: &str,
        secrets: &Secrets,
    ) -> Result<Rotated, SessionError> {
        self.sessions().rotate(refresh, session_id, secrets).await
    }

    async fn renew_web(
        &self,
        token: &str,
        channel: Channel,
    ) -> Result<(SessionRecord, Lifetime), SessionError> {
        self.sessions().renew_web(token, channel).await
    }
}

impl SessionStore for TursoSessions {
    async fn bind(&self, subject: Subject) -> Result<String, SessionError> {
        let tx = self.begin().await?;
        let decided = bind_in(&tx, subject)
            .await
            .map(|(actor, effect)| Decided::of(String::from(actor), effect));
        self.finish(tx, decided).await
    }

    async fn issue(
        &self,
        channel: Channel,
        observed: &ObservedSecurity,
        authenticated_at: u64,
        secrets: &Secrets,
    ) -> Result<Issued, SessionError> {
        let tx = self.begin().await?;
        let decided = issue_in(&tx, channel, observed, authenticated_at, secrets)
            .await
            .map(|issued| Decided::Commit(Ok(issued)));
        self.finish(tx, decided).await
    }

    async fn access(&self, token: &str, channel: Channel) -> Result<SessionRecord, SessionError> {
        let now = self.now()?;
        let reader = self.database.owner().reader().await.map_err(storage)?;
        let rows = reader
            .query(
                &live(),
                params_from_iter(access_params(token, channel, now)?),
            )
            .await
            .map_err(storage)?;
        at_most_one(rows, record)
            .await?
            .ok_or(SessionError::Inactive)
    }

    async fn session_id_of(
        &self,
        token: &str,
        channel: Channel,
    ) -> Result<Option<String>, SessionError> {
        let reader = self.database.owner().reader().await.map_err(storage)?;
        let rows = reader
            .query(
                "SELECT s.id FROM identity_session_tokens t JOIN identity_sessions s \
                 ON s.id=t.session_id WHERE t.access_hash=?1 AND s.channel=?2",
                params_from_iter([
                    Value::Blob(digest(token).to_vec()),
                    Value::Text(channel.stored().to_owned()),
                ]),
            )
            .await
            .map_err(storage)?;
        at_most_one(rows, |row| stored_session_id(row, 0)).await
    }

    async fn revoke(&self, session_id: &str, reason: RevokeReason) -> Result<(), SessionError> {
        let Some(session) = session_key(session_id) else {
            return Ok(());
        };
        let tx = self.begin().await?;
        let decided = revoke_in(&tx, &session, reason.stored())
            .await
            .map(|effect| Decided::of((), effect));
        self.finish(tx, decided).await
    }

    async fn logout(&self, token: &str, channel: Channel) -> Result<(), SessionError> {
        let tx = self.begin().await?;
        let decided = logout_in(
            &tx,
            "SELECT s.id FROM identity_session_tokens t JOIN identity_sessions s \
             ON s.id=t.session_id WHERE t.access_hash=?1 AND s.channel=?2",
            vec![
                Value::Blob(digest(token).to_vec()),
                Value::Text(channel.stored().to_owned()),
            ],
        )
        .await;
        self.finish(tx, decided).await
    }

    async fn revoke_refresh(&self, refresh: &str, session_id: &str) -> Result<(), SessionError> {
        let Some(session) = session_key(session_id) else {
            return Ok(());
        };
        let tx = self.begin().await?;
        let decided = logout_in(
            &tx,
            "SELECT s.id FROM identity_session_tokens t JOIN identity_sessions s \
             ON s.id=t.session_id WHERE t.refresh_hash=?1 AND s.id=?2 AND s.channel='theatre-native'",
            vec![Value::Blob(digest(refresh).to_vec()), Value::Text(session)],
        )
        .await;
        self.finish(tx, decided).await
    }

    async fn peek_refresh(
        &self,
        refresh: &str,
        session_id: &str,
    ) -> Result<SessionRecord, SessionError> {
        let session = session_key(session_id).ok_or(SessionError::Inactive)?;
        let tx = self.begin().await?;
        let decided = peek_in(&tx, refresh, &session).await;
        self.finish(tx, decided).await
    }

    async fn rotate(
        &self,
        refresh: &str,
        session_id: &str,
        secrets: &Secrets,
    ) -> Result<Rotated, SessionError> {
        let session = session_key(session_id).ok_or(SessionError::Inactive)?;
        let tx = self.begin().await?;
        let decided = rotate_in(&tx, refresh, &session, secrets).await;
        self.finish(tx, decided).await
    }

    async fn renew_web(
        &self,
        token: &str,
        channel: Channel,
    ) -> Result<(SessionRecord, Lifetime), SessionError> {
        let tx = self.begin().await?;
        let decided = renew_in(&tx, token, channel).await;
        self.finish(tx, decided).await
    }
}

#[cfg(test)]
mod tests;
