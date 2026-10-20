use sha2::{Digest, Sha256};
use tokio_postgres::Row;

use crate::platform::postgres::{Store, Tx};

use super::{
    domain::{
        decide_link, decide_registration, ActorId, ActorRecord, ExternalIdentityKey, IdentityError,
        LinkOutcome, Registration,
    },
    IdentityRegistry, RegistryError,
};

#[derive(Clone)]
pub struct PostgresIdentityRegistry {
    store: Store,
}

impl PostgresIdentityRegistry {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}

fn unavailable(_: impl std::fmt::Debug) -> RegistryError {
    RegistryError::Unavailable
}

fn actor_row(row: Row) -> Result<ActorRecord, RegistryError> {
    let id: String = row
        .try_get("id")
        .map_err(|_| RegistryError::CorruptRecord)?;
    Ok(ActorRecord {
        id: ActorId::new(id).map_err(|_| RegistryError::CorruptRecord)?,
        active: row
            .try_get("active")
            .map_err(|_| RegistryError::CorruptRecord)?,
    })
}

/// Length framing avoids concatenation aliases. Digest collisions only serialize unrelated
/// keys: the exact C-collated primary key, not this lock number, establishes identity.
fn key_lock(key: &ExternalIdentityKey) -> i64 {
    let mut digest = Sha256::new();
    digest.update(b"cantos-identity-registry-v1");
    for part in [key.issuer(), key.subject()] {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    let bytes = digest.finalize();
    let mut lock = [0u8; 8];
    lock.copy_from_slice(&bytes[..8]);
    i64::from_be_bytes(lock)
}

async fn lock_binding(
    tx: &Tx<'_>,
    key: &ExternalIdentityKey,
) -> Result<Option<ActorRecord>, RegistryError> {
    tx.query_one("SELECT pg_advisory_xact_lock($1)", &[&key_lock(key)])
        .await
        .map_err(unavailable)?;
    tx.query_opt(
        "SELECT a.id,a.active FROM actor_external_identities e JOIN actors a ON a.id=e.actor_id WHERE e.issuer=$1 AND e.subject=$2 FOR SHARE OF a",
        &[&key.issuer(), &key.subject()],
    ).await.map_err(unavailable)?.map(actor_row).transpose()
}

async fn insert_binding(
    tx: &Tx<'_>,
    key: &ExternalIdentityKey,
    actor: &ActorId,
) -> Result<(), RegistryError> {
    let changed = tx.execute(
        "INSERT INTO actor_external_identities(issuer,subject,actor_id) VALUES($1,$2,$3) ON CONFLICT (issuer,subject) DO NOTHING",
        &[&key.issuer(), &key.subject(), &actor.as_str()],
    ).await.map_err(unavailable)?;
    if changed != 1 {
        return Err(IdentityError::IdentityAlreadyBound.into());
    }
    Ok(())
}

impl IdentityRegistry for PostgresIdentityRegistry {
    async fn lookup(
        &self,
        key: &ExternalIdentityKey,
    ) -> Result<Option<ActorRecord>, RegistryError> {
        let client = self.store.connection().await.map_err(unavailable)?;
        client.query_opt(
            "SELECT a.id,a.active FROM actor_external_identities e JOIN actors a ON a.id=e.actor_id WHERE e.issuer=$1 AND e.subject=$2",
            &[&key.issuer(), &key.subject()],
        ).await.map_err(unavailable)?.map(actor_row).transpose()
    }

    async fn register(
        &self,
        key: &ExternalIdentityKey,
        candidate: &ActorId,
    ) -> Result<Registration, RegistryError> {
        let mut client = self.store.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        let binding = lock_binding(&tx, key).await?;
        let candidate_exists = tx
            .query_opt("SELECT id FROM actors WHERE id=$1", &[&candidate.as_str()])
            .await
            .map_err(unavailable)?
            .is_some();
        let result = decide_registration(binding.as_ref(), candidate, candidate_exists)?;
        if let Registration::Created(actor) = &result {
            let changed = tx
                .execute(
                    "INSERT INTO actors(id) VALUES($1) ON CONFLICT DO NOTHING",
                    &[&actor.as_str()],
                )
                .await
                .map_err(unavailable)?;
            if changed != 1 {
                return Err(IdentityError::ActorIdCollision.into());
            }
            insert_binding(&tx, key, actor).await?;
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }

    async fn link(
        &self,
        key: &ExternalIdentityKey,
        target: &ActorId,
    ) -> Result<LinkOutcome, RegistryError> {
        let mut client = self.store.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        let binding = lock_binding(&tx, key).await?;
        let target = tx
            .query_opt(
                "SELECT id,active FROM actors WHERE id=$1 FOR SHARE",
                &[&target.as_str()],
            )
            .await
            .map_err(unavailable)?
            .map(actor_row)
            .transpose()?;
        let outcome = decide_link(target.as_ref(), binding.as_ref().map(|actor| &actor.id))?;
        if outcome == LinkOutcome::Linked {
            let target = target.ok_or(RegistryError::CorruptRecord)?;
            insert_binding(&tx, key, &target.id).await?;
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(outcome)
    }
}
