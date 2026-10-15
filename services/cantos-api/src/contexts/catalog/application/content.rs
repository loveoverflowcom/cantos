//! Content commands and authorized reads reuse Catalog's transactional authorization lane.
use cantos_contracts::{AppendContentResponse, ContentHistoryResponse};
use serde::{Deserialize, Serialize};

use super::{authorize, current_actor, unit, work, CatalogStoreError};
use crate::contexts::catalog::{
    domain::{content::*, CatalogError, ContentUnit, ContentUnitId, Lifecycle, OwnerId, Work},
    postgres,
};
use crate::contexts::editorial::{self, permits, Action};
use crate::contexts::ingestion;
use crate::platform::postgres::{authenticate, normalized_uuid, LockMode, Store, StoreError, Tx};

#[path = "content_wire.rs"]
pub mod wire;

#[derive(Clone, Debug, Serialize)]
pub struct AppendContent {
    pub id: ContentRevisionId,
    pub unit_id: ContentUnitId,
    pub expected_revision: u64,
    pub expected_work_revision: u64,
    pub details: ContentDetails,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentReceipt {
    reply: AppendContentResponse,
    owner_required: bool,
}

async fn evidence(
    tx: &Tx<'_>,
    owner: &str,
    details: &ContentDetails,
) -> Result<(), CatalogStoreError> {
    if let Some(source) = details.body().source() {
        if ingestion::source_checksum(tx, owner, source.id())
            .await?
            .as_deref()
            != Some(source.sha256())
        {
            return Err(StoreError::EvidenceUnavailable.into());
        }
    }
    if let Some(script) = details.body().script() {
        let saved = editorial::owned_revision(tx, owner, script.id(), script.revision() as i64)
            .await?
            .ok_or(StoreError::EvidenceUnavailable)?;
        if saved.export_digest != script.export_digest() {
            return Err(StoreError::EvidenceUnavailable.into());
        }
    }
    for right in details.rights_refs() {
        if !editorial::evidence_exists(tx, owner, "rights", right).await? {
            return Err(StoreError::EvidenceUnavailable.into());
        }
    }
    Ok(())
}

/// Who appends: the recorded actor, and whether a rights change is within their authority.
pub(super) struct ContentWriter<'a> {
    pub actor: &'a str,
    pub may_change_rights: bool,
}

pub(super) enum Appended {
    Created(AppendContentResponse),
    Replayed(AppendContentResponse),
}

/// Bounds the command and fingerprints its exact tagged encoding before any transaction.
pub(super) fn fingerprint_append<T: Serialize>(
    operation: &str,
    tagged: &T,
    command: &AppendContent,
) -> Result<(String, String), CatalogStoreError> {
    let operation = normalized_uuid(operation)?;
    let bytes = serde_json::to_vec(tagged).map_err(|_| StoreError::InvalidRequest)?;
    if bytes.len() > 128 * 1024 {
        return Err(CatalogStoreError::Limit);
    }
    if command.expected_revision > i64::MAX as u64
        || command.expected_work_revision == 0
        || command.expected_work_revision > i64::MAX as u64
    {
        return Err(CatalogError::InvalidRevision.into());
    }
    Ok((operation, postgres::digest(&bytes)))
}

/// Shared append decision and effects. The caller has authenticated, taken the Catalog clock
/// for update, locked `work` for update and authorized a write on it; it rechecks its own
/// credential before commit.
pub(super) async fn append_authorized(
    tx: &Tx<'_>,
    writer: ContentWriter<'_>,
    operation: &str,
    fingerprint: &str,
    work: &Work,
    unit: &ContentUnit,
    command: AppendContent,
) -> Result<Appended, CatalogStoreError> {
    let actor = writer.actor;
    let current = postgres::content::load(tx, unit.id().as_str(), None).await?;
    // Rights changes retain Catalog's owner-only policy, including retry after later edits.
    if let Some((prior_fingerprint, bytes)) =
        postgres::prior_operation(tx, actor, operation).await?
    {
        if prior_fingerprint != fingerprint {
            return Err(CatalogStoreError::OperationReused);
        }
        let receipt: ContentReceipt =
            serde_json::from_slice(&bytes).map_err(|_| StoreError::CorruptRevision)?;
        let reply = receipt.reply;
        let pinned =
            wire::restore(reply.content.clone()).map_err(|_| StoreError::CorruptRevision)?;
        let stored = postgres::content::load(tx, unit.id().as_str(), Some(pinned.id().as_str()))
            .await?
            .ok_or(StoreError::CorruptRevision)?;
        if pinned != stored
            || pinned.work_id() != work.id()
            || pinned.recorded_by().as_str() != actor
            || reply.work_revision == 0
            || reply.work_revision > i64::MAX as u64
        {
            return Err(StoreError::CorruptRevision.into());
        }
        // The immutable receipt records the original authorization requirement, protected
        // by the same checksum as the result; later metadata cannot weaken a replay.
        if receipt.owner_required && !writer.may_change_rights {
            return Err(StoreError::Forbidden.into());
        }
        return Ok(Appended::Replayed(reply));
    }
    let previous_rights = current
        .as_ref()
        .map(|value| value.details().rights_refs())
        .unwrap_or(&work.details().rights_refs);
    let owner_required =
        rights_change_requires_owner(previous_rights, command.details.rights_refs());
    if owner_required && !writer.may_change_rights {
        return Err(StoreError::Forbidden.into());
    }
    let at = postgres::now(tx)
        .await?
        .max(work.updated_at())
        .max(current.as_ref().map_or(0, ContentRevision::recorded_at));
    let updated = work.touch(command.expected_work_revision, at)?;
    if unit.lifecycle() == Lifecycle::Archived {
        return Err(CatalogError::UnitArchived.into());
    }
    let revision = next_content_revision(
        current.as_ref().map_or(0, ContentRevision::revision),
        command.expected_revision,
    )?;
    if let Some(existing_unit) = postgres::content::unit_for_id(tx, command.id.as_str()).await? {
        return Err(if existing_unit == unit.id().as_str() {
            CatalogStoreError::DuplicateId
        } else {
            StoreError::NotFound.into()
        });
    }
    evidence(tx, work.owner_id().as_str(), &command.details).await?;
    let content = ContentRevision::new(ContentRevisionFacts {
        id: command.id,
        work_id: work.id().clone(),
        unit_id: unit.id().clone(),
        revision,
        details: command.details,
        recorded_by: OwnerId::parse(actor)?,
        recorded_at: at,
    })
    .map_err(wire::content_error)?;
    postgres::content::save(tx, &content).await?;
    postgres::save_work(tx, &updated, actor, false).await?;
    let reply = AppendContentResponse {
        content: wire::response(&content)?,
        work_revision: updated.revision(),
    };
    let result = serde_json::to_vec(&ContentReceipt {
        reply: reply.clone(),
        owner_required,
    })
    .map_err(|_| StoreError::CorruptRevision)?;
    postgres::record_operation(tx, actor, operation, fingerprint, &result).await?;
    postgres::bump_clock(tx).await?;
    Ok(Appended::Created(reply))
}

impl Store {
    pub async fn append_content(
        &self,
        token: &str,
        operation: &str,
        command: AppendContent,
    ) -> Result<AppendContentResponse, CatalogStoreError> {
        // A distinct command tag keeps operation IDs shared safely with legacy Catalog writes.
        let (operation, fingerprint) =
            fingerprint_append(operation, &("append_content_v1", &command), &command)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        postgres::lock_clock(&tx, LockMode::Update).await?;
        let actor = current_actor(&tx, token).await?;
        let unit = unit(&tx, &command.unit_id).await?;
        let work = work(&tx, unit.work_id(), LockMode::Update).await?;
        let access = authorize(&tx, &actor, &work, Action::Write).await?;
        let writer = ContentWriter {
            actor: &actor,
            may_change_rights: permits(access, Action::Review),
        };
        let (Appended::Created(reply) | Appended::Replayed(reply)) =
            append_authorized(&tx, writer, &operation, &fingerprint, &work, &unit, command).await?;
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(reply)
    }

    pub async fn content_revision(
        &self,
        token: &str,
        id: &ContentUnitId,
        revision: Option<&ContentRevisionId>,
    ) -> Result<ContentRevision, CatalogStoreError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        postgres::lock_clock(&tx, LockMode::Share).await?;
        let actor = current_actor(&tx, token).await?;
        let unit = unit(&tx, id).await?;
        let work = work(&tx, unit.work_id(), LockMode::Share).await?;
        authorize(&tx, &actor, &work, Action::Read).await?;
        let value =
            postgres::content::load(&tx, id.as_str(), revision.map(ContentRevisionId::as_str))
                .await?
                .ok_or(StoreError::NotFound)?;
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(value)
    }

    pub async fn content_history(
        &self,
        token: &str,
        id: &ContentUnitId,
        after: u64,
        limit: u64,
    ) -> Result<ContentHistoryResponse, CatalogStoreError> {
        let limit = super::page_limit(limit)?;
        if after > i64::MAX as u64 {
            return Err(StoreError::InvalidRequest.into());
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        postgres::lock_clock(&tx, LockMode::Share).await?;
        let actor = current_actor(&tx, token).await?;
        let unit = unit(&tx, id).await?;
        let work = work(&tx, unit.work_id(), LockMode::Share).await?;
        authorize(&tx, &actor, &work, Action::Read).await?;
        let reply = history_page(&tx, id, after, limit).await?;
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(reply)
    }
}

/// One bounded history page of an already authorized unit.
pub(super) async fn history_page(
    tx: &Tx<'_>,
    id: &ContentUnitId,
    after: u64,
    limit: i64,
) -> Result<ContentHistoryResponse, CatalogStoreError> {
    let mut values = postgres::content::history(tx, id.as_str(), after, limit + 1).await?;
    let more = values.len() > limit as usize;
    values.truncate(limit as usize);
    let next_after = if more {
        values.last().map(ContentRevision::revision)
    } else {
        None
    };
    let revisions = values
        .iter()
        .map(wire::summary)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ContentHistoryResponse {
        revisions,
        next_after,
    })
}
