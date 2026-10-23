//! Actor registry adapter over the shared, composed `server.db` owner (#333.4).
//!
//! Every mutation reads its facts and writes inside one `BEGIN IMMEDIATE` transaction on the
//! owner's serialized writer, which replaces PostgreSQL's per-key advisory lock and actor share
//! locks by ordering; the guarded inserts still check their affected rows. The immutable exact
//! binding is this registry's receipt: a lost COMMIT acknowledgement returns `Unavailable`, and
//! the caller reconciles by replaying the same exact key, never by assuming a rollback.
use cantos_store_turso::{
    error::{CommitFault, GuardCode, StoreFault},
    params_from_iter, Row, Rows, Value, WriteTransaction,
};

use crate::platform::turso::{Acknowledgement, ServerDatabase};

use super::{
    domain::{
        decide_link, decide_registration, ActorId, ActorRecord, ExternalIdentityKey, IdentityError,
        LinkOutcome, Registration,
    },
    IdentityRegistry, RegistryError,
};

/// A binding whose actor row is missing is corrupt rather than unbound, so it is read with a
/// LEFT JOIN even though foreign keys make it unreachable through this owner.
const BINDING: &str = "SELECT e.actor_id,a.id,a.active FROM actor_external_identities e LEFT JOIN actors a ON a.id=e.actor_id WHERE e.issuer=?1 AND e.subject=?2";
const ACTOR: &str = "SELECT id,active FROM actors WHERE id=?1";

#[derive(Clone)]
pub struct TursoIdentityRegistry {
    database: ServerDatabase,
    acknowledgement: Acknowledgement,
}

impl TursoIdentityRegistry {
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

    async fn begin(&self) -> Result<WriteTransaction<'_>, RegistryError> {
        self.database.owner().begin_write().await.map_err(storage)
    }

    /// Commit only an outcome that wrote; a decision made from reads alone needs no durable
    /// acknowledgement, so it is rolled back like a rejection.
    async fn finish<T>(
        &self,
        tx: WriteTransaction<'_>,
        result: Result<(T, Effect), RegistryError>,
    ) -> Result<T, RegistryError> {
        match result {
            Ok((outcome, Effect::Written)) => {
                self.acknowledgement
                    .commit(tx)
                    .await
                    .map_err(|fault| match fault {
                        // Rejected rolled back; Unknown may be durable. Both leave the caller to
                        // replay the exact key, which returns the durable winner either way.
                        CommitFault::Rejected(_) | CommitFault::Unknown(_) => {
                            RegistryError::Unavailable
                        }
                    })?;
                Ok(outcome)
            }
            Ok((outcome, Effect::ReadOnly)) => {
                tx.rollback().await.map_err(storage)?;
                Ok(outcome)
            }
            Err(error) => {
                tx.rollback().await.map_err(storage)?;
                Err(error)
            }
        }
    }
}

/// Whether a decision wrote rows and therefore needs a COMMIT acknowledgement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    ReadOnly,
    Written,
}

/// The schema refuses an insert over an existing key before conflict resolution (so `REPLACE`
/// cannot transfer a row); the adapter reports that refusal as the same domain rejection as
/// its own affected-row check.
fn occupied(guard: &str, rejection: IdentityError) -> impl Fn(StoreFault) -> RegistryError {
    let code = GuardCode::new(guard);
    move |fault| match fault {
        StoreFault::Guard(found) if Some(&found) == code.as_ref() => rejection.clone().into(),
        other => storage(other),
    }
}

/// Engine corruption is never served as a record; every other fault is an unavailable store.
fn storage(fault: StoreFault) -> RegistryError {
    match fault {
        StoreFault::Corrupt | StoreFault::NotDatabase => RegistryError::CorruptRecord,
        _ => RegistryError::Unavailable,
    }
}

fn text(row: &Row, index: usize) -> Result<String, RegistryError> {
    match row.get_value(index) {
        Ok(Value::Text(value)) => Ok(value),
        _ => Err(RegistryError::CorruptRecord),
    }
}

fn actor_id(row: &Row, index: usize) -> Result<ActorId, RegistryError> {
    ActorId::new(text(row, index)?).map_err(|_| RegistryError::CorruptRecord)
}

fn active(row: &Row, index: usize) -> Result<bool, RegistryError> {
    match row.get_value(index) {
        Ok(Value::Integer(0)) => Ok(false),
        Ok(Value::Integer(1)) => Ok(true),
        _ => Err(RegistryError::CorruptRecord),
    }
}

/// `(id, active)` of `ACTOR`.
fn actor_row(row: Row) -> Result<ActorRecord, RegistryError> {
    Ok(ActorRecord {
        id: actor_id(&row, 0)?,
        active: active(&row, 1)?,
    })
}

/// `(binding actor_id, actor id, actor active)` of `BINDING`.
fn binding_row(row: Row) -> Result<ActorRecord, RegistryError> {
    let bound = actor_id(&row, 0)?;
    let record = ActorRecord {
        id: actor_id(&row, 1)?,
        active: active(&row, 2)?,
    };
    if record.id != bound {
        return Err(RegistryError::CorruptRecord);
    }
    Ok(record)
}

/// At most one row: every predicate is a primary key, so a second row is corruption.
async fn at_most_one<T>(
    mut rows: Rows,
    decode: impl Fn(Row) -> Result<T, RegistryError>,
) -> Result<Option<T>, RegistryError> {
    let Some(row) = rows.next().await.map_err(storage)? else {
        return Ok(None);
    };
    let value = decode(row)?;
    if rows.next().await.map_err(storage)?.is_some() {
        return Err(RegistryError::CorruptRecord);
    }
    Ok(Some(value))
}

fn key_params(key: &ExternalIdentityKey) -> [&str; 2] {
    [key.issuer(), key.subject()]
}

async fn binding(
    tx: &WriteTransaction<'_>,
    key: &ExternalIdentityKey,
) -> Result<Option<ActorRecord>, RegistryError> {
    let rows = tx.query(BINDING, key_params(key)).await.map_err(storage)?;
    at_most_one(rows, binding_row).await
}

async fn actor(
    tx: &WriteTransaction<'_>,
    id: &ActorId,
) -> Result<Option<ActorRecord>, RegistryError> {
    let rows = tx.query(ACTOR, [id.as_str()]).await.map_err(storage)?;
    at_most_one(rows, actor_row).await
}

async fn insert_binding(
    tx: &WriteTransaction<'_>,
    key: &ExternalIdentityKey,
    actor: &ActorId,
) -> Result<(), RegistryError> {
    let changed = tx
        .execute(
            "INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES(?1,?2,?3,?4) ON CONFLICT(issuer,subject) DO NOTHING",
            params_from_iter([
                Value::Text(key.issuer().to_owned()),
                Value::Text(key.subject().to_owned()),
                Value::Text(actor.as_str().to_owned()),
                Value::Integer(tx.now().get()),
            ]),
        )
        .await
        .map_err(occupied(
            "immutable.actor_external_identities",
            IdentityError::IdentityAlreadyBound,
        ))?;
    if changed != 1 {
        return Err(IdentityError::IdentityAlreadyBound.into());
    }
    Ok(())
}

/// Registration inside a transaction the caller already holds, so another owner can bind and
/// write in the same unit of work (the Identity sessions issue a session with the binding). It
/// never begins, commits or rolls back.
pub(crate) async fn register_in(
    tx: &WriteTransaction<'_>,
    key: &ExternalIdentityKey,
    candidate: &ActorId,
) -> Result<(Registration, Effect), RegistryError> {
    let bound = binding(tx, key).await?;
    let candidate_exists = actor(tx, candidate).await?.is_some();
    let outcome = decide_registration(bound.as_ref(), candidate, candidate_exists)?;
    let Registration::Created(created) = &outcome else {
        return Ok((outcome, Effect::ReadOnly));
    };
    let changed = tx
        .execute(
            "INSERT INTO actors(id,active) VALUES(?1,1) ON CONFLICT(id) DO NOTHING",
            [created.as_str()],
        )
        .await
        .map_err(occupied(
            "immutable.actor_id",
            IdentityError::ActorIdCollision,
        ))?;
    if changed != 1 {
        return Err(IdentityError::ActorIdCollision.into());
    }
    insert_binding(tx, key, created).await?;
    Ok((outcome, Effect::Written))
}

async fn link_in(
    tx: &WriteTransaction<'_>,
    key: &ExternalIdentityKey,
    target: &ActorId,
) -> Result<(LinkOutcome, Effect), RegistryError> {
    let bound = binding(tx, key).await?;
    let target = actor(tx, target).await?;
    let outcome = decide_link(target.as_ref(), bound.as_ref().map(|actor| &actor.id))?;
    if outcome == LinkOutcome::Existing {
        return Ok((outcome, Effect::ReadOnly));
    }
    let target = target.ok_or(RegistryError::CorruptRecord)?;
    insert_binding(tx, key, &target.id).await?;
    Ok((outcome, Effect::Written))
}

impl IdentityRegistry for TursoIdentityRegistry {
    async fn lookup(
        &self,
        key: &ExternalIdentityKey,
    ) -> Result<Option<ActorRecord>, RegistryError> {
        let reader = self.database.owner().reader().await.map_err(storage)?;
        let rows = reader
            .query(BINDING, key_params(key))
            .await
            .map_err(storage)?;
        at_most_one(rows, binding_row).await
    }

    async fn register(
        &self,
        key: &ExternalIdentityKey,
        candidate: &ActorId,
    ) -> Result<Registration, RegistryError> {
        let tx = self.begin().await?;
        let result = register_in(&tx, key, candidate).await;
        self.finish(tx, result).await
    }

    async fn link(
        &self,
        key: &ExternalIdentityKey,
        target: &ActorId,
    ) -> Result<LinkOutcome, RegistryError> {
        let tx = self.begin().await?;
        let result = link_in(&tx, key, target).await;
        self.finish(tx, result).await
    }
}

#[cfg(test)]
mod tests;
