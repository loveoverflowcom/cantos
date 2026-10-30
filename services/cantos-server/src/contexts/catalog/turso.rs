//! Catalog SQL adapter. Application use cases own transactions and authorization; this module
//! only locks and persists their admitted values, rechecking snapshot integrity on every read.
use crate::platform::store::Row;

use super::application::MemberRole;
use super::domain::{ContentUnit, Revision, Work};
use crate::platform::store::{checked_number, StoreError, Tx};

use super::storage;
pub(super) use super::storage::{decode_unit, decode_work, digest};
pub(super) mod content;
pub(super) mod delegation;
pub(super) mod metadata_publication;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "S05 composes the public projection into Discovery"
    )
)]
pub(super) mod public_metadata;
pub(super) mod sample_metadata;
pub(super) mod sample_seed;

const WORK_COLUMNS: &str =
    "w.id,w.owner_id,w.kind,w.lifecycle,w.head_revision,r.revision,r.snapshot,r.snapshot_digest";
const UNIT_COLUMNS: &str = "u.id,u.work_id,u.work_kind,u.kind,u.position,u.lifecycle,u.head_revision,r.revision,r.snapshot,r.snapshot_digest";
const MAX_OPERATION_BYTES: usize = 4 * 1024 * 1024;

fn corrupt<T>(result: Result<T, crate::platform::store::Error>) -> Result<T, StoreError> {
    result.map_err(|_| StoreError::CorruptRevision)
}

fn decode_work_row(row: &Row) -> Result<Work, StoreError> {
    let bytes: Vec<u8> = corrupt(row.try_get("snapshot"))?;
    let stored_digest: String = corrupt(row.try_get("snapshot_digest"))?;
    if digest(&bytes) != stored_digest {
        return Err(StoreError::CorruptRevision);
    }
    let work = decode_work(&bytes)?;
    let revision: i64 = corrupt(row.try_get("revision"))?;
    let owner: String = corrupt(row.try_get("owner_id"))?;
    let id: String = corrupt(row.try_get("id"))?;
    let kind: String = corrupt(row.try_get("kind"))?;
    let lifecycle: String = corrupt(row.try_get("lifecycle"))?;
    if work.id().as_str() != id
        || work.owner_id().as_str() != owner
        || work.kind().as_str() != kind
        || work.lifecycle().as_str() != lifecycle
        || checked_number(work.revision()).map_err(|_| StoreError::CorruptRevision)? != revision
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(work)
}

fn decode_unit_row(row: &Row) -> Result<ContentUnit, StoreError> {
    let bytes: Vec<u8> = corrupt(row.try_get("snapshot"))?;
    let stored_digest: String = corrupt(row.try_get("snapshot_digest"))?;
    if digest(&bytes) != stored_digest {
        return Err(StoreError::CorruptRevision);
    }
    let work_kind: String = corrupt(row.try_get("work_kind"))?;
    let parent_kind = storage::work_kind(&work_kind)?;
    let unit = decode_unit(&bytes, parent_kind)?;
    let id: String = corrupt(row.try_get("id"))?;
    let work_id: String = corrupt(row.try_get("work_id"))?;
    let kind: String = corrupt(row.try_get("kind"))?;
    let lifecycle: String = corrupt(row.try_get("lifecycle"))?;
    let position: i32 = corrupt(row.try_get("position"))?;
    let revision: i64 = corrupt(row.try_get("revision"))?;
    let head: i64 = corrupt(row.try_get("head_revision"))?;
    if unit.id().as_str() != id
        || unit.work_id().as_str() != work_id
        || unit.kind().as_str() != kind
        || unit.lifecycle().as_str() != lifecycle
        || i64::from(unit.position()) != i64::from(position)
        || checked_number(unit.revision()).map_err(|_| StoreError::CorruptRevision)? != revision
        || revision != head
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(unit)
}

/// The clock is the initial catalog mutation fence and the epoch of paginated reads.
pub(super) async fn lock_clock(tx: &Tx<'_>) -> Result<u64, StoreError> {
    let query = "SELECT revision FROM catalog_clock WHERE singleton=1";
    let row = tx.query_one(query, &[]).await?;
    u64::try_from(corrupt(row.try_get::<_, i64>(0))?).map_err(|_| StoreError::CorruptRevision)
}

pub(super) async fn bump_clock(tx: &Tx<'_>) -> Result<u64, StoreError> {
    let row = tx
        .query_one(
            "UPDATE catalog_clock SET revision=revision+1 WHERE singleton=1 RETURNING revision",
            &[],
        )
        .await?;
    u64::try_from(corrupt(row.try_get::<_, i64>(0))?).map_err(|_| StoreError::CorruptRevision)
}

pub(super) async fn now(tx: &Tx<'_>) -> Result<i64, StoreError> {
    Ok(tx.now_micros() / 1_000)
}

/// Current actor facts in the caller's serialized write unit or pinned snapshot.
pub(super) async fn lock_actor(tx: &Tx<'_>, actor: &str) -> Result<bool, StoreError> {
    crate::platform::store::actor_active(tx, actor).await
}

pub(super) async fn load_work(tx: &Tx<'_>, id: &str) -> Result<Option<Work>, StoreError> {
    let query = format!("SELECT {WORK_COLUMNS} FROM catalog_works w LEFT JOIN catalog_work_revisions r ON r.work_id=w.id AND r.revision=w.head_revision WHERE w.id=?1");
    tx.query_opt(&query, &[&id])
        .await?
        .as_ref()
        .map(|row| {
            let work = decode_work_row(row)?;
            if checked_number(work.revision()).map_err(|_| StoreError::CorruptRevision)?
                != corrupt(row.try_get::<_, i64>("head_revision"))?
            {
                return Err(StoreError::CorruptRevision);
            }
            Ok(work)
        })
        .transpose()
}

/// An indexed exact historical lookup, bounded by the current head. The caller first locks
/// and authorizes the current Work. Identity is current and immutable; lifecycle is historical.
pub(super) async fn load_work_revision(
    tx: &Tx<'_>,
    id: &str,
    revision: Revision,
) -> Result<Option<Work>, StoreError> {
    let row = tx.query_opt(
        "SELECT w.id,w.owner_id,w.kind,r.revision,r.snapshot,r.snapshot_digest FROM catalog_works w JOIN catalog_work_revisions r ON r.work_id=w.id WHERE w.id=?1 AND r.revision=?2 AND r.revision<=w.head_revision",
        &[&id, &checked_number(revision.get())?],
    ).await?;
    row.as_ref()
        .map(|row| {
            let bytes: Vec<u8> = corrupt(row.try_get("snapshot"))?;
            let checksum: String = corrupt(row.try_get("snapshot_digest"))?;
            if digest(&bytes) != checksum {
                return Err(StoreError::CorruptRevision);
            }
            let work = decode_work(&bytes)?;
            if work.id().as_str() != corrupt(row.try_get::<_, String>("id"))?
                || work.owner_id().as_str() != corrupt(row.try_get::<_, String>("owner_id"))?
                || work.kind().as_str() != corrupt(row.try_get::<_, String>("kind"))?
                || work.revision() != revision.get()
                || checked_number(work.revision()).map_err(|_| StoreError::CorruptRevision)?
                    != corrupt(row.try_get::<_, i64>("revision"))?
            {
                return Err(StoreError::CorruptRevision);
            }
            Ok(work)
        })
        .transpose()
}

pub(super) async fn load_unit(tx: &Tx<'_>, id: &str) -> Result<Option<ContentUnit>, StoreError> {
    let query = format!("SELECT {UNIT_COLUMNS} FROM catalog_units u LEFT JOIN catalog_unit_revisions r ON r.unit_id=u.id AND r.revision=u.head_revision WHERE u.id=?1");
    tx.query_opt(&query, &[&id])
        .await?
        .as_ref()
        .map(decode_unit_row)
        .transpose()
}

/// Load an exact historical unit, checking its immutable parent and snapshot bytes.
pub(super) async fn load_unit_revision(
    tx: &Tx<'_>,
    id: &str,
    revision: Revision,
) -> Result<Option<ContentUnit>, StoreError> {
    let row = tx.query_opt(
        "SELECT u.id,u.work_id,u.work_kind,u.kind,r.revision,r.snapshot,r.snapshot_digest FROM catalog_units u JOIN catalog_unit_revisions r ON r.unit_id=u.id WHERE u.id=?1 AND r.revision=?2 AND r.revision<=u.head_revision",
        &[&id, &checked_number(revision.get())?],
    ).await?;
    row.as_ref()
        .map(|row| {
            let bytes: Vec<u8> = corrupt(row.try_get("snapshot"))?;
            let checksum: String = corrupt(row.try_get("snapshot_digest"))?;
            if digest(&bytes) != checksum {
                return Err(StoreError::CorruptRevision);
            }
            let kind: String = corrupt(row.try_get("work_kind"))?;
            let unit = decode_unit(&bytes, storage::work_kind(&kind)?)?;
            if unit.id().as_str() != corrupt(row.try_get::<_, String>("id"))?
                || unit.work_id().as_str() != corrupt(row.try_get::<_, String>("work_id"))?
                || unit.kind().as_str() != corrupt(row.try_get::<_, String>("kind"))?
                || unit.revision() != revision.get()
                || checked_number(unit.revision()).map_err(|_| StoreError::CorruptRevision)?
                    != corrupt(row.try_get::<_, i64>("revision"))?
            {
                return Err(StoreError::CorruptRevision);
            }
            Ok(unit)
        })
        .transpose()
}

pub(super) async fn member_role(
    tx: &Tx<'_>,
    work: &str,
    actor: &str,
) -> Result<Option<String>, StoreError> {
    let role = tx
        .query_opt(
            "SELECT role FROM catalog_members WHERE work_id=?1 AND actor_id=?2",
            &[&work, &actor],
        )
        .await?
        .map(|row| row.try_get::<_, String>(0))
        .transpose()?;
    if role
        .as_deref()
        .is_some_and(|role| MemberRole::parse(role).is_none())
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(role)
}

pub(super) async fn save_work(
    tx: &Tx<'_>,
    work: &Work,
    actor: &str,
    new: bool,
) -> Result<(), StoreError> {
    let snapshot = storage::encode(work)?;
    let snapshot_digest = digest(&snapshot);
    let revision = checked_number(work.revision())?;
    let id = work.id().as_str();
    if new {
        tx.execute("INSERT INTO catalog_works(id,owner_id,kind,head_revision,lifecycle) VALUES(?1,?2,?3,?4,?5)", &[&id,&work.owner_id().as_str(),&work.kind().as_str(),&revision,&work.lifecycle().as_str()]).await?;
    }
    tx.execute("INSERT INTO catalog_work_revisions(work_id,revision,snapshot,snapshot_digest,recorded_by) VALUES(?1,?2,?3,?4,?5)", &[&id,&revision,&snapshot,&snapshot_digest,&actor]).await?;
    if !new {
        let previous = revision.checked_sub(1).ok_or(StoreError::CorruptRevision)?;
        let changed = tx.execute("UPDATE catalog_works SET head_revision=?2,lifecycle=?3 WHERE id=?1 AND head_revision=?4", &[&id,&revision,&work.lifecycle().as_str(),&previous]).await?;
        if changed != 1 {
            return Err(StoreError::CorruptRevision);
        }
    }
    Ok(())
}

pub(super) async fn save_unit(
    tx: &Tx<'_>,
    unit: &ContentUnit,
    actor: &str,
    new: bool,
) -> Result<(), StoreError> {
    let snapshot = storage::encode(unit)?;
    let snapshot_digest = digest(&snapshot);
    let revision = checked_number(unit.revision())?;
    let position = i32::try_from(unit.position()).map_err(|_| StoreError::InvalidRequest)?;
    let id = unit.id().as_str();
    if new {
        // The parent kind comes from the already locked work, never a caller-controlled value.
        let changed = tx.execute("INSERT INTO catalog_units(id,work_id,work_kind,kind,head_revision,position,lifecycle) SELECT ?1,w.id,w.kind,?3,?4,?5,?6 FROM catalog_works w WHERE w.id=?2", &[&id,&unit.work_id().as_str(),&unit.kind().as_str(),&revision,&position,&unit.lifecycle().as_str()]).await?;
        if changed != 1 {
            return Err(StoreError::CorruptRevision);
        }
    }
    tx.execute("INSERT INTO catalog_unit_revisions(unit_id,revision,snapshot,snapshot_digest,recorded_by) VALUES(?1,?2,?3,?4,?5)", &[&id,&revision,&snapshot,&snapshot_digest,&actor]).await?;
    if !new {
        let previous = revision.checked_sub(1).ok_or(StoreError::CorruptRevision)?;
        let changed = tx.execute("UPDATE catalog_units SET head_revision=?2,position=?3,lifecycle=?4 WHERE id=?1 AND head_revision=?5", &[&id,&revision,&position,&unit.lifecycle().as_str(),&previous]).await?;
        if changed != 1 {
            return Err(StoreError::CorruptRevision);
        }
    }
    Ok(())
}

pub(super) async fn prior_operation(
    tx: &Tx<'_>,
    actor: &str,
    operation: &str,
) -> Result<Option<(String, Vec<u8>)>, StoreError> {
    let row = tx
        .query_opt(
            "SELECT fingerprint,result_bytes,result_digest FROM catalog_operations WHERE actor_id=?1 AND operation_id=?2",
            &[&actor, &operation],
        )
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let fingerprint: String = corrupt(row.try_get("fingerprint"))?;
    let bytes: Vec<u8> = corrupt(row.try_get("result_bytes"))?;
    let stored_digest: String = corrupt(row.try_get("result_digest"))?;
    let fingerprint_valid = fingerprint.len() == 64
        && fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if bytes.is_empty()
        || bytes.len() > MAX_OPERATION_BYTES
        || digest(&bytes) != stored_digest
        || !fingerprint_valid
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(Some((fingerprint, bytes)))
}

pub(super) async fn record_operation(
    tx: &Tx<'_>,
    actor: &str,
    operation: &str,
    fingerprint: &str,
    result: &[u8],
) -> Result<(), StoreError> {
    if result.is_empty() || result.len() > MAX_OPERATION_BYTES {
        return Err(StoreError::InvalidRequest);
    }
    tx.execute("INSERT INTO catalog_operations(actor_id,operation_id,fingerprint,result_bytes,result_digest) VALUES(?1,?2,?3,?4,?5)", &[&actor,&operation,&fingerprint,&result,&digest(result)]).await?;
    Ok(())
}

pub(super) async fn list_works(
    tx: &Tx<'_>,
    actor: &str,
    after: Option<&str>,
    include_archived: bool,
    limit: i64,
) -> Result<Vec<Work>, StoreError> {
    let query = format!("SELECT {WORK_COLUMNS} FROM catalog_works w LEFT JOIN catalog_work_revisions r ON r.work_id=w.id AND r.revision=w.head_revision WHERE (w.owner_id=?1 OR EXISTS(SELECT 1 FROM catalog_members m WHERE m.work_id=w.id AND m.actor_id=?1)) AND (?2 IS NULL OR w.id>?2) AND (?3 OR w.lifecycle<>'archived') ORDER BY w.id LIMIT ?4");
    tx.query(&query, &[&actor, &after, &include_archived, &limit])
        .await?
        .iter()
        .map(decode_work_row)
        .collect()
}

pub(super) async fn list_units(
    tx: &Tx<'_>,
    work: &str,
    after: Option<(u32, &str)>,
    include_archived: bool,
    limit: i64,
) -> Result<Vec<ContentUnit>, StoreError> {
    let position = after.map(|(position, _)| i64::from(position));
    let id = after.map(|(_, id)| id);
    let query = format!("SELECT {UNIT_COLUMNS} FROM catalog_units u LEFT JOIN catalog_unit_revisions r ON r.unit_id=u.id AND r.revision=u.head_revision WHERE u.work_id=?1 AND (?2 IS NULL OR (u.position,u.id)>(?2,?3)) AND (?4 OR u.lifecycle<>'archived') ORDER BY u.position,u.id LIMIT ?5");
    tx.query(&query, &[&work, &position, &id, &include_archived, &limit])
        .await?
        .iter()
        .map(decode_unit_row)
        .collect()
}

pub(super) async fn work_history(
    tx: &Tx<'_>,
    work: &str,
    after: u64,
    limit: i64,
) -> Result<Vec<Work>, StoreError> {
    // Lifecycle is historical here; identity remains anchored in the immutable work head.
    let query = "SELECT w.id,w.owner_id,w.kind,w.head_revision,r.revision,r.snapshot,r.snapshot_digest FROM catalog_works w JOIN catalog_work_revisions r ON r.work_id=w.id WHERE w.id=?1 AND r.revision>?2 ORDER BY r.revision LIMIT ?3";
    tx.query(query, &[&work, &checked_number(after)?, &limit])
        .await?
        .iter()
        .map(|row| {
            let bytes: Vec<u8> = corrupt(row.try_get("snapshot"))?;
            let stored_digest: String = corrupt(row.try_get("snapshot_digest"))?;
            if digest(&bytes) != stored_digest {
                return Err(StoreError::CorruptRevision);
            }
            let value = decode_work(&bytes)?;
            if value.id().as_str() != corrupt(row.try_get::<_, String>("id"))?
                || value.owner_id().as_str() != corrupt(row.try_get::<_, String>("owner_id"))?
                || value.kind().as_str() != corrupt(row.try_get::<_, String>("kind"))?
                || checked_number(value.revision()).map_err(|_| StoreError::CorruptRevision)?
                    != corrupt(row.try_get::<_, i64>("revision"))?
            {
                return Err(StoreError::CorruptRevision);
            }
            Ok(value)
        })
        .collect()
}

/// One extra row lets the application reject an overgrown aggregate rather than truncate it.
pub(super) async fn units_for_reorder(
    tx: &Tx<'_>,
    work: &str,
) -> Result<Vec<ContentUnit>, StoreError> {
    list_units(tx, work, None, true, 1001).await
}

pub(super) async fn set_member(
    tx: &Tx<'_>,
    work: &str,
    actor: &str,
    role: Option<&str>,
) -> Result<(), StoreError> {
    match role {
        Some(role) => {
            tx.execute("INSERT INTO catalog_members(work_id,actor_id,role) VALUES(?1,?2,?3) ON CONFLICT(work_id,actor_id) DO UPDATE SET role=EXCLUDED.role", &[&work,&actor,&role]).await?;
        }
        None => {
            tx.execute(
                "DELETE FROM catalog_members WHERE work_id=?1 AND actor_id=?2",
                &[&work, &actor],
            )
            .await?;
        }
    }
    Ok(())
}

mod listener;
pub(crate) use listener::{
    listener_metadata, ListenerMetadata, ListenerMetadataRequest, LISTENER_CURRENT_FILTER,
};

/// Move every existing position outside the final bounded range before a permutation.
/// The immutable revision snapshots keep the original positions until each changed unit saves.
pub(super) async fn stage_reorder(tx: &Tx<'_>, work: &str) -> Result<(), StoreError> {
    tx.execute(
        "UPDATE catalog_units SET position=position+2147483648 WHERE work_id=?1",
        &[&work],
    )
    .await?;
    Ok(())
}
pub(super) async fn finish_reorder(tx: &Tx<'_>, work: &str) -> Result<(), StoreError> {
    tx.execute("UPDATE catalog_units SET position=position-2147483648 WHERE work_id=?1 AND position>=2147483648", &[&work]).await?;
    Ok(())
}
