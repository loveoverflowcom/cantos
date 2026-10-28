//! Storage adapter for the bounded offline sample operator's immutable action receipts.
use crate::contexts::catalog::domain::{
    metadata_disclosure::FICTIONAL_SAMPLE_SOURCE_SHA256, sample_seed_request::SampleSeedRequest,
};
use crate::platform::store::{Row, StoreError, Tx};

pub(crate) async fn operation(
    tx: &Tx<'_>,
    id: &str,
    source: &str,
) -> Result<Option<Row>, StoreError> {
    let sql = if source == FICTIONAL_SAMPLE_SOURCE_SHA256 {
        "SELECT fingerprint FROM catalog_fictional_seed_actions WHERE operation_id=?1"
    } else {
        "SELECT fingerprint FROM catalog_sample_seed_actions WHERE operation_id=?1"
    };
    Ok(tx.query_opt(sql, &[&id]).await?)
}

pub(crate) async fn latest(tx: &Tx<'_>, source: &str) -> Result<Option<Row>, StoreError> {
    let sql = if source == FICTIONAL_SAMPLE_SOURCE_SHA256 {
        "SELECT action FROM catalog_fictional_seed_actions ORDER BY rowid DESC LIMIT 1"
    } else {
        "SELECT action FROM catalog_sample_seed_actions ORDER BY rowid DESC LIMIT 1"
    };
    Ok(tx.query_opt(sql, &[]).await?)
}

pub(crate) async fn insert(
    tx: &Tx<'_>,
    actor: &str,
    action: &str,
    request: &SampleSeedRequest,
    fingerprint: &str,
    now: i64,
) -> Result<(), StoreError> {
    let sql = if request.source_sha256 == FICTIONAL_SAMPLE_SOURCE_SHA256 {
        "INSERT INTO catalog_fictional_seed_actions(operation_id,actor_id,action,source_sha256,operator_reference,evidence_reference,statement,fingerprint,recorded_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)"
    } else {
        "INSERT INTO catalog_sample_seed_actions(operation_id,actor_id,action,source_sha256,operator_reference,evidence_reference,statement,fingerprint,recorded_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)"
    };
    tx.execute(
        sql,
        &[
            &request.operation_id,
            &actor,
            &action,
            &request.source_sha256,
            &request.operator_reference,
            &request.evidence_reference,
            &request.rights_statement,
            &fingerprint,
            &now,
        ],
    )
    .await?;
    Ok(())
}
