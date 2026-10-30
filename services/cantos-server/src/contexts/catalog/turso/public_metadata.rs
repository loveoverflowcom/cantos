//! Public projection reads over metadata publication generations (server.db v31). Every row
//! is rechecked against its pinned revision bytes; the live gates mirror the pure
//! `metadata_publication::work_visible` / `visible_entries` kernels.
use std::collections::BTreeSet;

use super::super::domain::{
    metadata_publication::{GenerationNumber, Pin},
    ContentUnit, ContentUnitId, Work, WorkKind,
};
use super::metadata_publication::{get, pin, positive};
use super::{decode_unit, decode_work, digest};
use crate::platform::store::{checked_number, Row, StoreError, Tx, WORK_OWNER_ACTIVE_FILTER};

/// The pinned Unit snapshots for one page, in one statement over a bound JSON array.
pub(crate) async fn pinned_units(
    tx: &Tx<'_>,
    work: &str,
    kind: WorkKind,
    pins: &[(&ContentUnitId, &Pin)],
) -> Result<Vec<ContentUnit>, StoreError> {
    let request: Vec<_> = pins
        .iter()
        .map(|(unit, pin)| serde_json::json!({"unit": unit.as_str(), "revision": pin.revision().get()}))
        .collect();
    let bound = serde_json::to_string(&request).map_err(|_| StoreError::CorruptRevision)?;
    let rows = tx
        .query(
            "SELECT json_extract(b.value,'$.unit') AS unit_id,r.snapshot,r.snapshot_digest FROM json_each(?1) b JOIN catalog_unit_revisions r ON r.unit_id=json_extract(b.value,'$.unit') AND r.revision=json_extract(b.value,'$.revision') ORDER BY b.key",
            &[&bound],
        )
        .await?;
    if rows.len() != pins.len() {
        return Err(StoreError::CorruptRevision);
    }
    rows.iter()
        .zip(pins)
        .map(|(row, (unit, pin))| {
            let bytes: Vec<u8> = get(row, "snapshot")?;
            if get::<String>(row, "unit_id")? != unit.as_str()
                || get::<String>(row, "snapshot_digest")? != pin.digest()
                || digest(&bytes) != pin.digest()
            {
                return Err(StoreError::CorruptRevision);
            }
            let decoded = decode_unit(&bytes, kind)?;
            if &decoded.id() != unit
                || decoded.work_id().as_str() != work
                || decoded.revision() != pin.revision().get()
            {
                return Err(StoreError::CorruptRevision);
            }
            Ok(decoded)
        })
        .collect()
}

/// One currently visible Work row of the public projection: the pinned Work snapshot and
/// the count of currently visible entries, all from one statement.
pub(crate) struct PublicWorkRow {
    pub work: Work,
    pub generation: GenerationNumber,
    pub first_visible_at: i64,
    pub unit_count: u32,
}

/// Live gates of the only v1 audience (PUBLIC): ACTIVE pointer, current Work not archived and
/// (appended by each query) the platform's active-owner filter. Mirrors
/// `metadata_publication::work_visible`; visible Units mirror `visible_entries`.
const PUBLIC_WORKS: &str = "SELECT p.work_id,p.generation,p.first_visible_at_ms,g.work_revision,g.work_digest,r.snapshot,r.snapshot_digest,\
 (SELECT count(*) FROM catalog_metadata_generation_entries e JOIN catalog_units u ON u.id=e.unit_id AND u.work_id=e.work_id \
  WHERE e.work_id=p.work_id AND e.generation=p.generation AND u.lifecycle<>'archived') AS unit_count \
 FROM catalog_metadata_publication_pointers p \
 JOIN catalog_works w ON w.id=p.work_id \
 JOIN catalog_metadata_generations g ON g.work_id=p.work_id AND g.generation=p.generation \
 JOIN catalog_work_revisions r ON r.work_id=g.work_id AND r.revision=g.work_revision \
 WHERE p.lifecycle='active' AND w.lifecycle<>'archived'";

fn public_work_row(row: &Row) -> Result<PublicWorkRow, StoreError> {
    let id: String = get(row, "work_id")?;
    let pin = pin(row, "work_revision", "work_digest")?;
    let bytes: Vec<u8> = get(row, "snapshot")?;
    if get::<String>(row, "snapshot_digest")? != pin.digest() || digest(&bytes) != pin.digest() {
        return Err(StoreError::CorruptRevision);
    }
    let work = decode_work(&bytes)?;
    if work.id().as_str() != id || work.revision() != pin.revision().get() {
        return Err(StoreError::CorruptRevision);
    }
    let count: i64 = get(row, "unit_count")?;
    Ok(PublicWorkRow {
        work,
        generation: GenerationNumber::new(positive(row, "generation")?)
            .map_err(|_| StoreError::CorruptRevision)?,
        first_visible_at: get(row, "first_visible_at_ms")?,
        unit_count: u32::try_from(count).map_err(|_| StoreError::CorruptRevision)?,
    })
}

/// Keyset over the immutable recency anchor, newest first, then ascending Work ID. Filters
/// apply before `LIMIT`, so a page never counts or skips a hidden Work.
pub(crate) async fn public_works(
    tx: &Tx<'_>,
    after: Option<(i64, &str)>,
    kind: Option<WorkKind>,
    language: Option<&str>,
    search: &str,
    limit: i64,
) -> Result<Vec<PublicWorkRow>, StoreError> {
    let sql = format!(
        "{PUBLIC_WORKS} AND {WORK_OWNER_ACTIVE_FILTER} AND (?1 IS NULL OR p.first_visible_at_ms<?1 OR (p.first_visible_at_ms=?1 AND p.work_id>?2)) \
 AND (?3 IS NULL OR g.kind=?3) AND (?4 IS NULL OR g.language=?4) AND (?5='' OR instr(g.search_text,?5)>0) \
 ORDER BY p.first_visible_at_ms DESC,p.work_id ASC LIMIT ?6"
    );
    let (after_at, after_id) = match after {
        Some((at, id)) => (Some(at), Some(id)),
        None => (None, None),
    };
    let kind = kind.map(WorkKind::as_str);
    let rows = tx
        .query(
            &sql,
            &[&after_at, &after_id, &kind, &language, &search, &limit],
        )
        .await?;
    rows.iter().map(public_work_row).collect()
}

pub(crate) async fn public_work(
    tx: &Tx<'_>,
    work: &str,
) -> Result<Option<PublicWorkRow>, StoreError> {
    let sql = format!("{PUBLIC_WORKS} AND {WORK_OWNER_ACTIVE_FILTER} AND p.work_id=?1");
    tx.query_opt(&sql, &[&work])
        .await?
        .as_ref()
        .map(public_work_row)
        .transpose()
}

/// Current archived flags of one generation's Units, keyed by Unit ID.
pub(crate) async fn archived_units(
    tx: &Tx<'_>,
    work: &str,
    number: GenerationNumber,
) -> Result<BTreeSet<String>, StoreError> {
    let rows = tx
        .query(
            "SELECT e.unit_id FROM catalog_metadata_generation_entries e JOIN catalog_units u ON u.id=e.unit_id WHERE e.work_id=?1 AND e.generation=?2 AND (u.lifecycle='archived' OR u.work_id<>e.work_id)",
            &[&work, &checked_number(number.get())?],
        )
        .await?;
    rows.iter().map(|row| get(row, "unit_id")).collect()
}
