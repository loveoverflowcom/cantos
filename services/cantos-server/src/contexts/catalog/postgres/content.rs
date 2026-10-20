//! Indexed immutable content storage. Transactions/access live in application::content.
use tokio_postgres::Row;

use crate::contexts::catalog::domain::content::ContentRevision;
use crate::contexts::catalog::wire::content as wire;
use crate::platform::postgres::{checked_number, StoreError, Tx};

const COLUMNS: &str =
    "r.id,r.unit_id,r.revision,r.snapshot,r.revision_digest,r.recorded_by,u.work_id";
const MAX_SNAPSHOT_BYTES: usize = 128 * 1024;

fn decode(row: &Row) -> Result<ContentRevision, StoreError> {
    let corrupt = || StoreError::CorruptRevision;
    let bytes: Vec<u8> = row.try_get("snapshot").map_err(|_| corrupt())?;
    if bytes.is_empty() || bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(corrupt());
    }
    let raw = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
    let value = wire::restore(raw).map_err(|_| corrupt())?;
    let digest: String = row.try_get("revision_digest").map_err(|_| corrupt())?;
    let id: String = row.try_get("id").map_err(|_| corrupt())?;
    let unit: String = row.try_get("unit_id").map_err(|_| corrupt())?;
    let work: String = row.try_get("work_id").map_err(|_| corrupt())?;
    let actor: String = row.try_get("recorded_by").map_err(|_| corrupt())?;
    let revision: i64 = row.try_get("revision").map_err(|_| corrupt())?;
    if value.revision_digest().map_err(|_| corrupt())?.as_str() != digest
        || value.id().as_str() != id
        || value.unit_id().as_str() != unit
        || value.work_id().as_str() != work
        || value.recorded_by().as_str() != actor
        || checked_number(value.revision()).map_err(|_| corrupt())? != revision
    {
        return Err(corrupt());
    }
    Ok(value)
}

pub(in crate::contexts::catalog) async fn unit_for_id(
    tx: &Tx<'_>,
    id: &str,
) -> Result<Option<String>, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT unit_id FROM catalog_content_revisions WHERE id=$1",
            &[&id],
        )
        .await?
        .map(|row| row.get(0)))
}

pub(in crate::contexts::catalog) async fn load(
    tx: &Tx<'_>,
    unit: &str,
    id: Option<&str>,
) -> Result<Option<ContentRevision>, StoreError> {
    let query = if id.is_some() {
        format!("SELECT {COLUMNS} FROM catalog_content_revisions r JOIN catalog_units u ON u.id=r.unit_id WHERE r.unit_id=$1 AND r.id=$2")
    } else {
        format!("SELECT {COLUMNS},h.revision AS head_sequence FROM catalog_content_heads h JOIN catalog_content_revisions r ON r.id=h.revision_id AND r.unit_id=h.unit_id AND r.revision=h.revision JOIN catalog_units u ON u.id=r.unit_id WHERE h.unit_id=$1")
    };
    let row = if let Some(id) = id {
        tx.query_opt(&query, &[&unit, &id]).await?
    } else {
        tx.query_opt(&query, &[&unit]).await?
    };
    row.as_ref().map(decode).transpose()
}

pub(in crate::contexts::catalog) async fn save(
    tx: &Tx<'_>,
    value: &ContentRevision,
) -> Result<(), StoreError> {
    let snapshot =
        serde_json::to_vec(&wire::response(value).map_err(|_| StoreError::CorruptRevision)?)
            .map_err(|_| StoreError::CorruptRevision)?;
    if snapshot.len() > MAX_SNAPSHOT_BYTES {
        return Err(StoreError::InvalidRequest);
    }
    let id = value.id().as_str();
    let unit = value.unit_id().as_str();
    let revision = checked_number(value.revision())?;
    let digest = value
        .revision_digest()
        .map_err(|_| StoreError::CorruptRevision)?;
    tx.execute("INSERT INTO catalog_content_revisions(id,unit_id,revision,snapshot,revision_digest,recorded_by) VALUES($1,$2,$3,$4,$5,$6)",
        &[&id,&unit,&revision,&snapshot,&digest.as_str(),&value.recorded_by().as_str()]).await?;
    if revision == 1 {
        tx.execute(
            "INSERT INTO catalog_content_heads(unit_id,revision_id,revision) VALUES($1,$2,$3)",
            &[&unit, &id, &revision],
        )
        .await?;
    } else {
        let previous = revision - 1;
        let changed = tx.execute("UPDATE catalog_content_heads SET revision_id=$2,revision=$3 WHERE unit_id=$1 AND revision=$4", &[&unit,&id,&revision,&previous]).await?;
        if changed != 1 {
            return Err(StoreError::CorruptRevision);
        }
    }
    Ok(())
}

pub(in crate::contexts::catalog) async fn history(
    tx: &Tx<'_>,
    unit: &str,
    after: u64,
    limit: i64,
) -> Result<Vec<ContentRevision>, StoreError> {
    let query = format!("SELECT {COLUMNS} FROM catalog_content_revisions r JOIN catalog_units u ON u.id=r.unit_id WHERE r.unit_id=$1 AND r.revision>$2 ORDER BY r.revision LIMIT $3");
    tx.query(&query, &[&unit, &checked_number(after)?, &limit])
        .await?
        .iter()
        .map(decode)
        .collect()
}
