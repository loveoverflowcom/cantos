//! Delegation grant SQL. Rows are restored through the pure constructors; any disagreement
//! with them is corruption, never a weaker grant. Callers hold the Catalog clock fence.
use crate::platform::store::Row;

use crate::contexts::catalog::domain::{
    delegation::{DelegatedAction, DelegatedActions, DelegationGrant, DelegationGrantId},
    OwnerId, WorkId,
};
use crate::platform::store::{StoreError, Tx};

pub(in crate::contexts::catalog) struct GrantRecord {
    pub grant: DelegationGrant,
    pub revoked_at: Option<i64>,
    pub request_digest: String,
}

const SELECT: &str = "SELECT g.id,g.work_id,g.grantor_id,g.delegate_id,g.work_read,g.content_read,g.content_write,g.issued_at,g.expires_at,g.request_digest,r.revoked_at FROM catalog_delegation_grants g LEFT JOIN catalog_delegation_revocations r ON r.grant_id=g.id";

fn corrupt<T>(result: Result<T, crate::platform::store::Error>) -> Result<T, StoreError> {
    result.map_err(|_| StoreError::CorruptRevision)
}

fn decode(row: &Row) -> Result<GrantRecord, StoreError> {
    let text = |name: &str| corrupt(row.try_get::<_, String>(name));
    let flag = |name: &str| corrupt(row.try_get::<_, bool>(name));
    let id = text("id")?;
    let granted = [
        (flag("work_read")?, DelegatedAction::WorkRead),
        (flag("content_read")?, DelegatedAction::ContentRead),
        (flag("content_write")?, DelegatedAction::ContentWrite),
    ];
    let actions = DelegatedActions::new(
        granted
            .into_iter()
            .filter(|(granted, _)| *granted)
            .map(|(_, action)| action),
    );
    let grant = DelegationGrant::restore(
        DelegationGrantId::parse(&id).map_err(|_| StoreError::CorruptRevision)?,
        WorkId::parse(&text("work_id")?).map_err(|_| StoreError::CorruptRevision)?,
        OwnerId::parse(&text("grantor_id")?).map_err(|_| StoreError::CorruptRevision)?,
        OwnerId::parse(&text("delegate_id")?).map_err(|_| StoreError::CorruptRevision)?,
        actions.map_err(|_| StoreError::CorruptRevision)?,
        corrupt(row.try_get("issued_at"))?,
        corrupt(row.try_get("expires_at"))?,
    )
    .map_err(|_| StoreError::CorruptRevision)?;
    // Canonical ID spelling is part of identity; a lookup must not find a different spelling.
    if grant.id().as_str() != id || grant.work_id().as_str() != text("work_id")? {
        return Err(StoreError::CorruptRevision);
    }
    Ok(GrantRecord {
        grant,
        revoked_at: corrupt(row.try_get("revoked_at"))?,
        request_digest: text("request_digest")?,
    })
}

async fn one(
    tx: &Tx<'_>,
    predicate: &str,
    params: &[&(dyn crate::platform::store::Param + Sync)],
) -> Result<Option<GrantRecord>, StoreError> {
    tx.query_opt(&format!("{SELECT} WHERE {predicate}"), params)
        .await?
        .as_ref()
        .map(decode)
        .transpose()
}

pub(in crate::contexts::catalog) async fn by_token(
    tx: &Tx<'_>,
    hash: &[u8; 32],
) -> Result<Option<GrantRecord>, StoreError> {
    one(tx, "g.token_hash=?1", &[&hash.as_slice()]).await
}

pub(in crate::contexts::catalog) async fn by_id(
    tx: &Tx<'_>,
    id: &DelegationGrantId,
) -> Result<Option<GrantRecord>, StoreError> {
    one(tx, "g.id=?1", &[&id.as_str()]).await
}

pub(in crate::contexts::catalog) async fn by_operation(
    tx: &Tx<'_>,
    grantor: &str,
    operation: &str,
) -> Result<Option<GrantRecord>, StoreError> {
    one(
        tx,
        "g.grantor_id=?1 AND g.operation_id=?2",
        &[&grantor, &operation],
    )
    .await
}

pub(in crate::contexts::catalog) async fn list(
    tx: &Tx<'_>,
    work: &WorkId,
    after: Option<&str>,
    limit: i64,
) -> Result<Vec<GrantRecord>, StoreError> {
    tx.query(
        &format!("{SELECT} WHERE g.work_id=?1 AND (?2 IS NULL OR g.id>?2) ORDER BY g.id LIMIT ?3"),
        &[&work.as_str(), &after, &limit],
    )
    .await?
    .iter()
    .map(decode)
    .collect()
}

pub(in crate::contexts::catalog) async fn insert(
    tx: &Tx<'_>,
    grant: &DelegationGrant,
    hash: &[u8; 32],
    operation: &str,
    request_digest: &str,
) -> Result<(), StoreError> {
    let actions = grant.actions();
    tx.execute(
        "INSERT INTO catalog_delegation_grants(id,work_id,grantor_id,delegate_id,work_read,content_read,content_write,token_hash,issued_at,expires_at,operation_id,request_digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        &[
            &grant.id().as_str(),
            &grant.work_id().as_str(),
            &grant.grantor().as_str(),
            &grant.delegate().as_str(),
            &actions.contains(DelegatedAction::WorkRead),
            &actions.contains(DelegatedAction::ContentRead),
            &actions.contains(DelegatedAction::ContentWrite),
            &hash.as_slice(),
            &grant.issued_at(),
            &grant.expires_at(),
            &operation,
            &request_digest,
        ],
    )
    .await?;
    Ok(())
}

/// Idempotent: the first revocation's actor and time are the settled record.
pub(in crate::contexts::catalog) async fn revoke(
    tx: &Tx<'_>,
    grant: &DelegationGrantId,
    actor: &str,
    now: i64,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO catalog_delegation_revocations(grant_id,revoked_by,revoked_at) SELECT ?1,?2,?3 WHERE NOT EXISTS(SELECT 1 FROM catalog_delegation_revocations WHERE grant_id=?1)",
        &[&grant.as_str(), &actor, &now],
    )
    .await?;
    Ok(())
}

pub(in crate::contexts::catalog) async fn record_lineage(
    tx: &Tx<'_>,
    grant: &DelegationGrantId,
    actor: &str,
    operation: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO catalog_delegated_operations(grant_id,actor_id,operation_id) VALUES(?1,?2,?3)",
        &[&grant.as_str(), &actor, &operation],
    )
    .await?;
    Ok(())
}

pub(in crate::contexts::catalog) async fn lineage(
    tx: &Tx<'_>,
    actor: &str,
    operation: &str,
) -> Result<Option<String>, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT grant_id FROM catalog_delegated_operations WHERE actor_id=?1 AND operation_id=?2",
            &[&actor, &operation],
        )
        .await?
        .map(|row| row.get(0)))
}

/// Delegate and grantor actors are locked like any Catalog caller, so deactivation waits.
pub(in crate::contexts::catalog) async fn actor_active(
    tx: &Tx<'_>,
    actor: &OwnerId,
) -> Result<bool, StoreError> {
    super::lock_actor(tx, actor.as_str()).await
}
