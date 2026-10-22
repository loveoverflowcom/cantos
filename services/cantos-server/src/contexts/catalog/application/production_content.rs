//! Catalog-owned, transactional facts for the concrete Production Brief consumer.
use super::{current_actor, require, unit, work, CatalogStoreError};
use crate::contexts::catalog::{
    domain::{
        content::{ContentRevision, ContentRevisionId},
        ContentUnit, ContentUnitId, OwnerId, Work,
    },
    postgres,
};
use crate::contexts::editorial::{Access, Action};
use crate::platform::store::{LockMode, StoreError, Tx};

pub(crate) struct ProductionContent {
    pub work: Work,
    pub unit: ContentUnit,
    pub head: ContentRevision,
}

/// All Catalog mutations share the clock fence. This consumer uses the same ordering, and
/// requires owner authority before resolving private underlying source/export bytes.
pub(crate) async fn production_content(
    tx: &Tx<'_>,
    token: Option<&str>,
    expected_owner: Option<&OwnerId>,
    id: &ContentUnitId,
    lock: LockMode,
) -> Result<ProductionContent, CatalogStoreError> {
    postgres::lock_clock(tx, lock).await?;
    let actor = if let Some(token) = token {
        current_actor(tx, token).await?
    } else {
        let owner = expected_owner.ok_or(StoreError::Unauthenticated)?;
        if !postgres::lock_actor(tx, owner.as_str()).await? {
            return Err(StoreError::Unauthenticated.into());
        }
        owner.as_str().into()
    };
    let unit = unit(tx, id).await?;
    let work = work(tx, unit.work_id(), lock).await?;
    if actor != work.owner_id().as_str() {
        return Err(StoreError::NotFound.into());
    }
    require(Access::Owner, Action::Review)?;
    let head = postgres::content::load(tx, id.as_str(), None)
        .await?
        .ok_or(StoreError::NotFound)?;
    Ok(ProductionContent { work, unit, head })
}
pub(crate) async fn pinned_content(
    tx: &Tx<'_>,
    unit: &ContentUnitId,
    id: &ContentRevisionId,
) -> Result<ContentRevision, CatalogStoreError> {
    postgres::content::load(tx, unit.as_str(), Some(id.as_str()))
        .await?
        .ok_or_else(|| StoreError::NotFound.into())
}
pub(crate) fn restore_content(
    raw: cantos_contracts::ContentRevisionResponse,
) -> Result<ContentRevision, CatalogStoreError> {
    super::content::wire::restore(raw)
}
