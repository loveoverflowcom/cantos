//! Catalog-owned batched snapshot admission for Media's concrete public listener consumer.
use super::super::domain::{ContentUnit, Work};
use crate::platform::postgres::{StoreError, Tx};
use serde::Serialize;
use sha2::{Digest, Sha256};
#[derive(Clone, Serialize)]
pub(crate) struct ListenerMetadataRequest {
    pub release: String,
    pub work: String,
    pub unit: String,
    pub work_revision: i64,
    pub unit_revision: i64,
    pub content: String,
}
pub(crate) struct ListenerMetadata {
    pub release: String,
    pub work: Work,
    pub unit: ContentUnit,
    pub current_work: Work,
    pub current_unit: ContentUnit,
    pub content: cantos_contracts::ContentRevisionResponse,
}
pub(crate) async fn listener_metadata(
    tx: &Tx<'_>,
    requests: &[ListenerMetadataRequest],
) -> Result<Vec<ListenerMetadata>, StoreError> {
    if requests.len() > 51 {
        return Err(StoreError::InvalidRequest);
    }
    let bytes = serde_json::to_string(requests).map_err(|_| StoreError::CorruptRevision)?;
    let rows=tx.query("SELECT b.release,b.work AS requested_work,b.unit AS requested_unit,b.work_revision,b.unit_revision,b.content AS requested_content,w.owner_id,w.kind,w.head_revision AS current_work_revision,u.head_revision AS current_unit_revision,cr.revision_digest AS content_digest,wr.snapshot AS work,wr.snapshot_digest AS work_digest,ur.snapshot AS unit,ur.snapshot_digest AS unit_digest,cw.snapshot AS current_work,cw.snapshot_digest AS current_work_digest,cu.snapshot AS current_unit,cu.snapshot_digest AS current_unit_digest,cr.snapshot AS content FROM jsonb_to_recordset($1::text::jsonb) AS b(release text,work text,unit text,work_revision bigint,unit_revision bigint,content text) JOIN catalog_works w ON w.id=b.work JOIN catalog_units u ON u.id=b.unit AND u.work_id=w.id JOIN catalog_work_revisions wr ON wr.work_id=w.id AND wr.revision=b.work_revision JOIN catalog_unit_revisions ur ON ur.unit_id=u.id AND ur.revision=b.unit_revision JOIN catalog_work_revisions cw ON cw.work_id=w.id AND cw.revision=w.head_revision JOIN catalog_unit_revisions cu ON cu.unit_id=u.id AND cu.revision=u.head_revision JOIN catalog_content_revisions cr ON cr.id=b.content AND cr.unit_id=u.id",&[&bytes]).await?;
    rows.iter()
        .map(|r| {
            let checked = |key: &str, digest: &str| -> Result<Vec<u8>, StoreError> {
                let bytes: Vec<u8> = r.try_get(key).map_err(|_| StoreError::CorruptRevision)?;
                let expected: String =
                    r.try_get(digest).map_err(|_| StoreError::CorruptRevision)?;
                if format!("{:x}", Sha256::digest(&bytes)) != expected {
                    return Err(StoreError::CorruptRevision);
                }
                Ok(bytes)
            };
            let work = super::decode_work(&checked("work", "work_digest")?)?;
            let unit = super::decode_unit(&checked("unit", "unit_digest")?, work.kind())?;
            let current_work =
                super::decode_work(&checked("current_work", "current_work_digest")?)?;
            let current_unit = super::decode_unit(
                &checked("current_unit", "current_unit_digest")?,
                work.kind(),
            )?;
            let bytes: Vec<u8> = r
                .try_get("content")
                .map_err(|_| StoreError::CorruptRevision)?;
            if bytes.len() > 131072 {
                return Err(StoreError::CorruptRevision);
            }
            let content: cantos_contracts::ContentRevisionResponse =
                serde_json::from_slice(&bytes).map_err(|_| StoreError::CorruptRevision)?;
            let get = |key: &str| {
                r.try_get::<_, String>(key)
                    .map_err(|_| StoreError::CorruptRevision)
            };
            let number = |key: &str| {
                r.try_get::<_, i64>(key)
                    .map_err(|_| StoreError::CorruptRevision)
            };
            let admitted_content = super::super::restore_content(content.clone())
                .map_err(|_| StoreError::CorruptRevision)?;
            if work.id().as_str() != get("requested_work")?
                || unit.id().as_str() != get("requested_unit")?
                || unit.work_id() != work.id()
                || work.owner_id().as_str() != get("owner_id")?
                || work.kind().as_str() != get("kind")?
                || work.revision() as i64 != number("work_revision")?
                || unit.revision() as i64 != number("unit_revision")?
                || current_work.revision() as i64 != number("current_work_revision")?
                || current_unit.revision() as i64 != number("current_unit_revision")?
                || admitted_content.id().as_str() != get("requested_content")?
                || admitted_content.unit_id() != unit.id()
                || admitted_content.work_id() != work.id()
                || admitted_content
                    .revision_digest()
                    .map_err(|_| StoreError::CorruptRevision)?
                    .as_str()
                    != get("content_digest")?
            {
                return Err(StoreError::CorruptRevision);
            }
            Ok(ListenerMetadata {
                release: r
                    .try_get("release")
                    .map_err(|_| StoreError::CorruptRevision)?,
                work,
                unit,
                current_work,
                current_unit,
                content,
            })
        })
        .collect()
}

/// SQL-owned current-public metadata prefilter; never sufficient publication authority.
pub(crate) const LISTENER_CURRENT_FILTER: &str =
    "catalog_listener_discoverable(x.work_id,x.unit_id)";

/// Share the existing Catalog/Media write fence when admitting a new listener-owned identity.
/// This is not Studio authorization; it prevents concurrent metadata/rights/pointer replacement.
pub(crate) async fn lock_listener_catalog(tx: &Tx<'_>) -> Result<(), StoreError> {
    super::lock_clock(tx, crate::platform::postgres::LockMode::Share).await?;
    Ok(())
}
