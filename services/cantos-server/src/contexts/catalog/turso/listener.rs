//! Catalog-owned batched snapshot admission for Media's concrete public listener consumer.
use super::super::domain::{ContentUnit, Work};
use crate::platform::store::{StoreError, Tx};
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
    pub content: crate::contract::studio::ContentRevisionResponse,
}
/// One bounded join over the requested releases' pinned revisions, their current heads and their
/// content revision; the request batch is a JSON array read by `json_each`, never spliced.
const METADATA: &str = "SELECT json_extract(b.value,'$.release') AS release_id,\
 json_extract(b.value,'$.work') AS requested_work,\
 json_extract(b.value,'$.unit') AS requested_unit,\
 json_extract(b.value,'$.work_revision') AS work_revision,\
 json_extract(b.value,'$.unit_revision') AS unit_revision,\
 json_extract(b.value,'$.content') AS requested_content,\
 w.owner_id,w.kind,w.head_revision AS current_work_revision,u.head_revision AS current_unit_revision,\
 cr.revision_digest AS content_digest,\
 wr.snapshot AS work,wr.snapshot_digest AS work_digest,\
 ur.snapshot AS unit,ur.snapshot_digest AS unit_digest,\
 cw.snapshot AS current_work,cw.snapshot_digest AS current_work_digest,\
 cu.snapshot AS current_unit,cu.snapshot_digest AS current_unit_digest,\
 cr.snapshot AS content \
 FROM json_each(?1) b \
 JOIN catalog_works w ON w.id=json_extract(b.value,'$.work') \
 JOIN catalog_units u ON u.id=json_extract(b.value,'$.unit') AND u.work_id=w.id \
 JOIN catalog_work_revisions wr ON wr.work_id=w.id AND wr.revision=json_extract(b.value,'$.work_revision') \
 JOIN catalog_unit_revisions ur ON ur.unit_id=u.id AND ur.revision=json_extract(b.value,'$.unit_revision') \
 JOIN catalog_work_revisions cw ON cw.work_id=w.id AND cw.revision=w.head_revision \
 JOIN catalog_unit_revisions cu ON cu.unit_id=u.id AND cu.revision=u.head_revision \
 JOIN catalog_content_revisions cr ON cr.id=json_extract(b.value,'$.content') AND cr.unit_id=u.id";
pub(crate) async fn listener_metadata(
    tx: &Tx<'_>,
    requests: &[ListenerMetadataRequest],
) -> Result<Vec<ListenerMetadata>, StoreError> {
    if requests.len() > 51 {
        return Err(StoreError::InvalidRequest);
    }
    let bytes = serde_json::to_string(requests).map_err(|_| StoreError::CorruptRevision)?;
    let rows = tx.query(METADATA, &[&bytes]).await?;
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
            let work = super::super::storage::decode_work(&checked("work", "work_digest")?)?;
            let unit =
                super::super::storage::decode_unit(&checked("unit", "unit_digest")?, work.kind())?;
            let current_work = super::super::storage::decode_work(&checked(
                "current_work",
                "current_work_digest",
            )?)?;
            let current_unit = super::super::storage::decode_unit(
                &checked("current_unit", "current_unit_digest")?,
                work.kind(),
            )?;
            let bytes: Vec<u8> = r
                .try_get("content")
                .map_err(|_| StoreError::CorruptRevision)?;
            if bytes.len() > 131072 {
                return Err(StoreError::CorruptRevision);
            }
            let content: crate::contract::studio::ContentRevisionResponse =
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
                    .try_get("release_id")
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

/// SQL-owned current-public metadata prefilter over the listener index alias `x`. It replaces
/// PostgreSQL's `catalog_listener_discoverable` function: neither archived, and the Work's
/// current head revision names `public` visibility. Never sufficient publication authority.
pub(crate) const LISTENER_CURRENT_FILTER: &str = "EXISTS(SELECT 1 FROM catalog_works d_w \
JOIN catalog_units d_u ON d_u.work_id=d_w.id AND d_u.id=x.unit_id \
JOIN catalog_work_revisions d_r ON d_r.work_id=d_w.id AND d_r.revision=d_w.head_revision \
WHERE d_w.id=x.work_id AND d_w.lifecycle<>'archived' AND d_u.lifecycle<>'archived' \
AND json_extract(CAST(d_r.snapshot AS TEXT),'$.details.visibility')='public')";

/// Lifecycle-only half for generation admission; editorial visibility never gates that lane.
pub(crate) const LISTENER_ACTIVE_FILTER: &str = "EXISTS(SELECT 1 FROM catalog_works d_w JOIN catalog_units d_u ON d_u.work_id=d_w.id AND d_u.id=x.unit_id WHERE d_w.id=x.work_id AND d_w.lifecycle<>'archived' AND d_u.lifecycle<>'archived')";

/// Anonymous Library candidate relation over Media's listener alias `x`: generation membership
/// and an exact association precede representative selection. Media still admits every audio fact.
/// Catalog owns every metadata/access table named here; no editorial head is authority.
pub(crate) const LISTENER_PUBLIC_GENERATION_FILTER: &str = "EXISTS(SELECT 1 FROM catalog_metadata_publication_pointers gp JOIN catalog_metadata_generation_entries ge ON ge.work_id=gp.work_id AND ge.generation=gp.generation JOIN catalog_metadata_generation_audio ga ON ga.work_id=ge.work_id AND ga.generation=ge.generation AND ga.unit_id=ge.unit_id JOIN catalog_works gw ON gw.id=gp.work_id JOIN catalog_units gu ON gu.id=ge.unit_id AND gu.work_id=gw.id WHERE gp.work_id=x.work_id AND gp.lifecycle='active' AND gw.lifecycle<>'archived' AND gu.lifecycle<>'archived' AND ge.unit_id=x.unit_id AND ga.release_id=x.release_id AND NOT EXISTS(SELECT 1 FROM catalog_unit_viewer_policies uv WHERE uv.unit_id=ge.unit_id AND uv.mode<>'inherit') AND NOT EXISTS(SELECT 1 FROM catalog_viewer_policies vp WHERE vp.work_id=gp.work_id AND vp.audience<>'public'))";
