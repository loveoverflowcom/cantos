//! Owner-only public-display decisions for a Work cover image (ADR 0033 rule 2). One short write
//! transaction: current owner authorization, the exact image binding, replay and the server's
//! per-Work order. No object I/O, no publication or visibility change, no Theatre effect besides
//! the next projection reading the latest decision.
use crate::contract::studio as dto;

use super::{
    domain::cover_disclosure::{
        decide_cover_disclosure, CoverDisclosureDecision, CoverDisclosureError, DisclosureFacts,
        RecordCoverDisclosure,
    },
    turso::cover as sql,
};
use crate::{
    contexts::catalog::{self, domain::WorkId, CatalogStoreError},
    platform::store::{Store, StoreError},
};

#[derive(Debug)]
pub enum CoverDisclosureApplicationError {
    Storage(StoreError),
    Catalog(CatalogStoreError),
    Disclosure(CoverDisclosureError),
}
impl From<StoreError> for CoverDisclosureApplicationError {
    fn from(value: StoreError) -> Self {
        Self::Storage(value)
    }
}
impl From<CatalogStoreError> for CoverDisclosureApplicationError {
    fn from(value: CatalogStoreError) -> Self {
        Self::Catalog(value)
    }
}
impl From<CoverDisclosureError> for CoverDisclosureApplicationError {
    fn from(value: CoverDisclosureError) -> Self {
        Self::Disclosure(value)
    }
}

#[derive(Clone)]
pub struct CoverDisclosureService {
    store: Store,
}
impl CoverDisclosureService {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    /// Record a clearance or revocation by the current active Work owner. A matching operation
    /// returns the originally recorded decision unchanged.
    pub async fn record(
        &self,
        token: &str,
        work: &str,
        raw: dto::RecordCoverDisclosureRequest,
    ) -> Result<dto::CoverDisclosureResponse, CoverDisclosureApplicationError> {
        let work_id = WorkId::parse(work).map_err(|_| StoreError::NotFound)?;
        let request = RecordCoverDisclosure::admit(&raw)?;
        let mut connection = self.store.connection().await?;
        let tx = connection.transaction().await.map_err(StoreError::from)?;
        let owner = catalog::owner_work(&tx, token, &work_id).await?;
        let prior =
            sql::prior_disclosure(&tx, &owner.actor, request.operation_id().as_str()).await?;
        let image_bound = sql::image_bound(
            &tx,
            &owner.actor,
            owner.work.id(),
            request.image_asset_digest(),
        )
        .await?;
        let next_sequence = sql::next_sequence(&tx, owner.work.id()).await?;
        let now_ms = catalog::owner_work_now(&tx).await?;
        let record = match decide_cover_disclosure(
            &request,
            DisclosureFacts {
                work: &owner.work,
                prior: prior.as_ref(),
                image_bound,
                next_sequence,
                now_ms,
            },
        )? {
            CoverDisclosureDecision::Replay(record) => record,
            CoverDisclosureDecision::Apply(record) => {
                sql::insert_disclosure(&tx, &record).await?;
                record
            }
        };
        catalog::recheck_owner_work(&tx, token, &owner).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(record.response())
    }
}
