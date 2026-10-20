//! Catalog-owned facts and atomic metadata advancement for the private Media artwork consumer.
//! Media never reads or writes Catalog tables directly. Reads admit archived history; only
//! the existing Work transition decides whether a new metadata write is allowed.
use super::{current_actor, work, CatalogStoreError};
use crate::contexts::catalog::{
    domain::{OwnerId, Revision, Work, WorkId},
    postgres,
};
use crate::platform::postgres::{authenticate, LockMode, StoreError, Tx};

pub(crate) struct OwnerWork {
    pub work: Work,
    pub actor: OwnerId,
}

/// Preserve Catalog's session → clock fence → active actor → Work lock ordering. Every wait
/// is followed by authentication; membership never grants this owner-only capability.
pub(crate) async fn owner_work(
    tx: &Tx<'_>,
    token: &str,
    id: &WorkId,
    lock: LockMode,
) -> Result<OwnerWork, CatalogStoreError> {
    authenticate(tx, token).await?;
    postgres::lock_clock(tx, lock).await?;
    let actor = current_actor(tx, token).await?;
    let work = work(tx, id, lock).await?;
    if actor != work.owner_id().as_str() {
        return Err(StoreError::NotFound.into());
    }
    let actor = OwnerId::parse(&actor).map_err(|_| StoreError::CorruptRevision)?;
    let owner = OwnerWork { work, actor };
    recheck_owner_work(tx, token, &owner).await?;
    Ok(owner)
}

/// Exact historical snapshots are resolved after current ownership and active identity. A
/// missing/future revision never substitutes the current head, and old lifecycle stays old.
pub(crate) async fn owner_work_revision(
    tx: &Tx<'_>,
    token: &str,
    id: &WorkId,
    requested: Option<Revision>,
) -> Result<OwnerWork, CatalogStoreError> {
    let mut owner = owner_work(tx, token, id, LockMode::Share).await?;
    if let Some(revision) = requested {
        owner.work = postgres::load_work_revision(tx, id.as_str(), revision)
            .await?
            .ok_or(StoreError::NotFound)?;
        if owner.work.owner_id() != &owner.actor {
            return Err(StoreError::CorruptRevision.into());
        }
    }
    recheck_owner_work(tx, token, &owner).await?;
    Ok(owner)
}

/// The caller transaction also writes Media's change and original operation receipt. This
/// facade does not commit: rollback must discard all four effects, including Catalog's epoch.
/// Rereading the current locked head prevents a stale value from another transaction being saved.
pub(crate) async fn advance_owner_work(
    tx: &Tx<'_>,
    token: &str,
    id: &WorkId,
    expected: Revision,
    at: i64,
) -> Result<Work, CatalogStoreError> {
    let owner = owner_work(tx, token, id, LockMode::Update).await?;
    // Same authoritative monotonic timestamp policy as Catalog command dispatch.
    let at = at.max(owner.work.updated_at());
    let touched = owner.work.touch(expected.get(), at)?;
    postgres::save_work(tx, &touched, owner.actor.as_str(), false).await?;
    postgres::bump_clock(tx).await?;
    recheck_owner_work(tx, token, &owner).await?;
    Ok(touched)
}

pub(crate) async fn owner_work_now(tx: &Tx<'_>) -> Result<i64, CatalogStoreError> {
    let now = postgres::now(tx).await?;
    if now < 0 {
        return Err(StoreError::CorruptRevision.into());
    }
    Ok(now)
}

pub(crate) async fn recheck_owner_work(
    tx: &Tx<'_>,
    token: &str,
    owner: &OwnerWork,
) -> Result<(), CatalogStoreError> {
    if authenticate(tx, token).await? != owner.actor.as_str()
        || owner.work.owner_id() != &owner.actor
    {
        return Err(StoreError::Unauthenticated.into());
    }
    Ok(())
}
