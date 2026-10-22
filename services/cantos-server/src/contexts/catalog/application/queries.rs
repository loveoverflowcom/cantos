//! Authorized Studio catalog reads, deterministic pages and revision history.
use super::super::{
    domain::{ContentUnit, ContentUnitId, Work, WorkId},
    postgres,
};
use super::{
    authorize, current_actor, cursor_after, page_limit, unit, work, CatalogCursor, CatalogPage,
    CatalogStoreError,
};
use crate::contexts::editorial::Action;
use crate::platform::store::{authenticate, normalized_uuid, LockMode, Store, StoreError, Tx};

impl Store {
    pub async fn catalog_work(&self, token: &str, id: &WorkId) -> Result<Work, CatalogStoreError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        postgres::lock_clock(&tx, LockMode::Share).await?;
        let actor = current_actor(&tx, token).await?;
        let work = work(&tx, id, LockMode::Share).await?;
        authorize(&tx, &actor, &work, Action::Read).await?;
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(work)
    }
    pub async fn catalog_unit(
        &self,
        token: &str,
        id: &ContentUnitId,
    ) -> Result<ContentUnit, CatalogStoreError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        postgres::lock_clock(&tx, LockMode::Share).await?;
        let actor = current_actor(&tx, token).await?;
        let unit = unit(&tx, id).await?;
        let work = work(&tx, unit.work_id(), LockMode::Share).await?;
        authorize(&tx, &actor, &work, Action::Read).await?;
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(unit)
    }

    pub async fn catalog_works(
        &self,
        token: &str,
        limit: u64,
        include_archived: bool,
        cursor: Option<&CatalogCursor>,
    ) -> Result<CatalogPage<Work>, CatalogStoreError> {
        let limit = page_limit(limit)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        let epoch = postgres::lock_clock(&tx, LockMode::Share).await?;
        let actor = current_actor(&tx, token).await?;
        let scope = format!("works:{actor}:{include_archived}");
        let after = cursor_after(cursor, &scope, epoch)?;
        if let Some(after) = after {
            normalized_uuid(after).map_err(|_| CatalogStoreError::InvalidCursor)?;
        }
        let mut items =
            postgres::list_works(&tx, &actor, after, include_archived, limit + 1).await?;
        for work in &items {
            authorize(&tx, &actor, work, Action::Read).await?;
        }
        let more = items.len() > limit as usize;
        items.truncate(limit as usize);
        let next = if more {
            items.last().map(|work| CatalogCursor {
                scope,
                epoch,
                after: work.id().as_str().into(),
            })
        } else {
            None
        };
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(CatalogPage { items, next })
    }

    pub async fn catalog_units(
        &self,
        token: &str,
        id: &WorkId,
        limit: u64,
        include_archived: bool,
        cursor: Option<&CatalogCursor>,
    ) -> Result<CatalogPage<ContentUnit>, CatalogStoreError> {
        let limit = page_limit(limit)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        let epoch = postgres::lock_clock(&tx, LockMode::Share).await?;
        let actor = current_actor(&tx, token).await?;
        let work = work(&tx, id, LockMode::Share).await?;
        authorize(&tx, &actor, &work, Action::Read).await?;
        let scope = format!("units:{actor}:{}:{include_archived}", id.as_str());
        let page = units_page(&tx, id, &scope, epoch, limit, include_archived, cursor).await?;
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(page)
    }

    pub async fn catalog_work_history(
        &self,
        token: &str,
        id: &WorkId,
        limit: u64,
        cursor: Option<&CatalogCursor>,
    ) -> Result<CatalogPage<Work>, CatalogStoreError> {
        let limit = page_limit(limit)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        let epoch = postgres::lock_clock(&tx, LockMode::Share).await?;
        let actor = current_actor(&tx, token).await?;
        let work = work(&tx, id, LockMode::Share).await?;
        authorize(&tx, &actor, &work, Action::Read).await?;
        let scope = format!("history:{actor}:{}", id.as_str());
        let after = cursor_after(cursor, &scope, epoch)?
            .map(|after| {
                after
                    .parse::<u64>()
                    .map_err(|_| CatalogStoreError::InvalidCursor)
            })
            .transpose()?
            .unwrap_or(0);
        if after > i64::MAX as u64 {
            return Err(CatalogStoreError::InvalidCursor);
        }
        let mut items = postgres::work_history(&tx, id.as_str(), after, limit + 1).await?;
        let more = items.len() > limit as usize;
        items.truncate(limit as usize);
        let next = if more {
            items.last().map(|work| CatalogCursor {
                scope,
                epoch,
                after: work.revision().to_string(),
            })
        } else {
            None
        };
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(CatalogPage { items, next })
    }
}

/// One deterministic unit page of an already authorized Work. `scope` binds the cursor to the
/// caller identity and query; the clock `epoch` makes it stale after any Catalog write.
pub(super) async fn units_page(
    tx: &Tx<'_>,
    id: &WorkId,
    scope: &str,
    epoch: u64,
    limit: i64,
    include_archived: bool,
    cursor: Option<&CatalogCursor>,
) -> Result<CatalogPage<ContentUnit>, CatalogStoreError> {
    let after = cursor_after(cursor, scope, epoch)?
        .map(|after| {
            let (position, id) = after
                .split_once(':')
                .ok_or(CatalogStoreError::InvalidCursor)?;
            let position = position
                .parse::<u32>()
                .map_err(|_| CatalogStoreError::InvalidCursor)?;
            if position > i32::MAX as u32 {
                return Err(CatalogStoreError::InvalidCursor);
            }
            normalized_uuid(id).map_err(|_| CatalogStoreError::InvalidCursor)?;
            Ok((position, id))
        })
        .transpose()?;
    let mut items =
        postgres::list_units(tx, id.as_str(), after, include_archived, limit + 1).await?;
    let more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next = if more {
        items.last().map(|unit| CatalogCursor {
            scope: scope.into(),
            epoch,
            after: format!("{}:{}", unit.position(), unit.id().as_str()),
        })
    } else {
        None
    };
    Ok(CatalogPage { items, next })
}
