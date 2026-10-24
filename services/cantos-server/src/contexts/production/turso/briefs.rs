//! Production Brief SQL adapter; no cross-context table reads.
use super::super::application::briefs::restore;
use super::super::domain::brief::{ProducerScope, ProductionBrief, MAX_BRIEF_BYTES};
use crate::platform::store::{StoreError, Tx};

pub(in crate::contexts::production) async fn brief_id_owner(
    tx: &Tx<'_>,
    id: &str,
) -> Result<Option<String>, StoreError> {
    Ok(tx
        .query_opt("SELECT owner_id FROM production_briefs WHERE id=?1", &[&id])
        .await?
        .map(|row| row.try_get(0))
        .transpose()?)
}

pub(in crate::contexts::production) async fn now_ms(tx: &Tx<'_>) -> Result<i64, StoreError> {
    Ok(tx.now_micros() / 1_000)
}
use crate::contract::studio as dto;
use crate::platform::store::Row;

fn decode(row: &Row) -> Result<ProductionBrief, StoreError> {
    let bytes: Vec<u8> = row
        .try_get("snapshot")
        .map_err(|_| StoreError::CorruptRevision)?;
    if bytes.is_empty() || bytes.len() > MAX_BRIEF_BYTES {
        return Err(StoreError::CorruptRevision);
    }
    let raw = serde_json::from_slice(&bytes).map_err(|_| StoreError::CorruptRevision)?;
    let brief = restore(raw)?;
    let doc = brief.document();
    if row
        .try_get::<_, String>("id")
        .map_err(|_| StoreError::CorruptRevision)?
        != doc.id
        || row
            .try_get::<_, String>("unit_id")
            .map_err(|_| StoreError::CorruptRevision)?
            != doc.content.unit_id
        || row
            .try_get::<_, String>("content_revision_id")
            .map_err(|_| StoreError::CorruptRevision)?
            != doc.content.id
        || row
            .try_get::<_, i64>("content_sequence")
            .map_err(|_| StoreError::CorruptRevision)?
            != crate::platform::store::checked_number(doc.content.revision)
                .map_err(|_| StoreError::CorruptRevision)?
        || row
            .try_get::<_, String>("owner_id")
            .map_err(|_| StoreError::CorruptRevision)?
            != doc.work.owner_id
        || row
            .try_get::<_, i64>("version")
            .map_err(|_| StoreError::CorruptRevision)?
            != crate::platform::store::checked_number(doc.version)
                .map_err(|_| StoreError::CorruptRevision)?
        || row
            .try_get::<_, String>("digest")
            .map_err(|_| StoreError::CorruptRevision)?
            != doc.digest
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(brief)
}
pub(in crate::contexts::production) async fn load(
    tx: &Tx<'_>,
    unit: &str,
    version: u64,
) -> Result<Option<ProductionBrief>, StoreError> {
    tx.query_opt(
        "SELECT * FROM production_briefs WHERE unit_id=?1 AND version=?2",
        &[&unit, &(crate::platform::store::checked_number(version)?)],
    )
    .await?
    .as_ref()
    .map(decode)
    .transpose()
}
pub(in crate::contexts::production) async fn prior(
    tx: &Tx<'_>,
    owner: &str,
    operation: &str,
) -> Result<Option<(String, ProductionBrief)>, StoreError> {
    let row = tx
        .query_opt(
            "SELECT * FROM production_briefs WHERE owner_id=?1 AND operation_id=?2",
            &[&owner, &operation],
        )
        .await?;
    row.as_ref()
        .map(|row| Ok((row.try_get("request_digest")?, decode(row)?)))
        .transpose()
}
pub(in crate::contexts::production) async fn latest(
    tx: &Tx<'_>,
    unit: &str,
) -> Result<u64, StoreError> {
    tx.query_opt(
        "SELECT version FROM production_brief_heads WHERE unit_id=?1",
        &[&unit],
    )
    .await?
    .map_or(Ok(0), |row| {
        u64::try_from(row.try_get::<_, i64>(0)?).map_err(|_| StoreError::CorruptRevision)
    })
}
pub(in crate::contexts::production) async fn insert(
    tx: &Tx<'_>,
    brief: &ProductionBrief,
    operation: &str,
    request_digest: &str,
) -> Result<(), StoreError> {
    let doc = brief.document();
    let version = crate::platform::store::checked_number(doc.version)?;
    let bytes = serde_json::to_vec(doc).map_err(|_| StoreError::CorruptRevision)?;
    tx.execute("INSERT INTO production_briefs(id,unit_id,content_revision_id,owner_id,version,operation_id,request_digest,digest,snapshot,content_sequence) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",&[&doc.id,&doc.content.unit_id,&doc.content.id,&doc.work.owner_id,&version,&operation,&request_digest,&doc.digest,&bytes,&(crate::platform::store::checked_number(doc.content.revision)?)]).await?;
    if version == 1 {
        tx.execute(
            "INSERT INTO production_brief_heads(unit_id,brief_id,version) VALUES(?1,?2,?3)",
            &[&doc.content.unit_id, &doc.id, &version],
        )
        .await?;
    } else {
        let count=tx.execute("UPDATE production_brief_heads SET brief_id=?2,version=?3 WHERE unit_id=?1 AND version=?4",&[&doc.content.unit_id,&doc.id,&version,&(version-1)]).await?;
        if count != 1 {
            return Err(StoreError::CorruptRevision);
        }
    }
    Ok(())
}

pub(in crate::contexts::production) struct GrantRow {
    pub response: dto::ProducerGrantResponse,
    pub owner: String,
    pub token_hash: Vec<u8>,
    pub operation: String,
    pub request_digest: String,
}
const GRANT_SELECT:&str="SELECT g.*,b.snapshot FROM production_producer_grants g JOIN production_briefs b ON b.id=g.brief_id";
async fn grant_query(
    tx: &Tx<'_>,
    predicate: &str,
    value: &(dyn crate::platform::store::Param + Sync),
) -> Result<Option<GrantRow>, StoreError> {
    let query = format!("{GRANT_SELECT} WHERE {predicate}=?1");
    let Some(row) = tx.query_opt(&query, &[value]).await? else {
        return Ok(None);
    };
    let bytes: Vec<u8> = row.try_get("snapshot")?;
    if bytes.len() > MAX_BRIEF_BYTES {
        return Err(StoreError::CorruptRevision);
    }
    let brief = restore(serde_json::from_slice(&bytes).map_err(|_| StoreError::CorruptRevision)?)?;
    let work = brief.document().work.id.clone();
    // The grant adapter gets Work identity from its immutable owning brief, never Catalog SQL.
    let response = dto::ProducerGrantResponse {
        id: row.try_get("id")?,
        work_id: work,
        unit_id: row.try_get("unit_id")?,
        brief_id: row.try_get("brief_id")?,
        brief_digest: row.try_get("brief_digest")?,
        permission: "read_instructions".into(),
        expires_at: row.try_get("expires_at")?,
    };
    ProducerScope::admit(&response).map_err(|_| StoreError::CorruptRevision)?;
    if response.unit_id != brief.document().content.unit_id
        || response.brief_id != brief.document().id
        || response.brief_digest != brief.document().digest
        || row.try_get::<_, String>("owner_id")? != brief.document().work.owner_id
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(Some(GrantRow {
        response,
        owner: row.try_get("owner_id")?,
        token_hash: row.try_get("token_hash")?,
        operation: row.try_get("operation_id")?,
        request_digest: row.try_get("request_digest")?,
    }))
}
pub(in crate::contexts::production) async fn by_token(
    tx: &Tx<'_>,
    hash: &[u8],
) -> Result<Option<GrantRow>, StoreError> {
    grant_query(tx, "g.token_hash", &hash).await
}
pub(in crate::contexts::production) async fn by_id(
    tx: &Tx<'_>,
    id: &str,
) -> Result<Option<GrantRow>, StoreError> {
    grant_query(tx, "g.id", &id).await
}
pub(in crate::contexts::production) async fn grant_prior(
    tx: &Tx<'_>,
    owner: &str,
    operation: &str,
) -> Result<Option<GrantRow>, StoreError> {
    let row = tx
        .query_opt(
            "SELECT id FROM production_producer_grants WHERE owner_id=?1 AND operation_id=?2",
            &[&owner, &operation],
        )
        .await?;
    match row {
        Some(row) => by_id(tx, &row.try_get::<_, String>(0)?).await,
        None => Ok(None),
    }
}
pub(in crate::contexts::production) async fn insert_grant(
    tx: &Tx<'_>,
    row: &GrantRow,
    now: i64,
) -> Result<(), StoreError> {
    let d = &row.response;
    tx.execute("INSERT INTO production_producer_grants(id,unit_id,brief_id,owner_id,brief_digest,token_hash,expires_at,created_at,operation_id,request_digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",&[&d.id,&d.unit_id,&d.brief_id,&row.owner,&d.brief_digest,&row.token_hash,&d.expires_at,&now,&row.operation,&row.request_digest]).await?;
    Ok(())
}
pub(in crate::contexts::production) async fn revoked(
    tx: &Tx<'_>,
    id: &str,
) -> Result<bool, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT grant_id FROM production_producer_revocations WHERE grant_id=?1",
            &[&id],
        )
        .await?
        .is_some())
}
pub(in crate::contexts::production) async fn revoke(
    tx: &Tx<'_>,
    id: &str,
    actor: &str,
    now: i64,
) -> Result<(), StoreError> {
    tx.execute("INSERT INTO production_producer_revocations SELECT ?1,?2,?3 WHERE NOT EXISTS(SELECT 1 FROM production_producer_revocations WHERE grant_id=?1)",&[&id,&actor,&now]).await?;
    Ok(())
}
