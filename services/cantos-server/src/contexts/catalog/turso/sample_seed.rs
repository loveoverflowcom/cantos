//! Storage adapter for the bounded offline sample operator's immutable action receipts.
use crate::contexts::catalog::domain::{
    metadata_disclosure::APPROVED_SAMPLE_SOURCE_SHA256, sample_seed_request::SampleSeedRequest,
};
use crate::platform::store::{Row, StoreError, Tx};

pub(crate) async fn operation(tx: &Tx<'_>, id: &str) -> Result<Option<Row>, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT fingerprint FROM catalog_sample_seed_actions WHERE operation_id=?1",
            &[&id],
        )
        .await?)
}

pub(crate) async fn latest(tx: &Tx<'_>) -> Result<Option<Row>, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT action FROM catalog_sample_seed_actions ORDER BY rowid DESC LIMIT 1",
            &[],
        )
        .await?)
}

pub(crate) async fn insert(
    tx: &Tx<'_>,
    actor: &str,
    action: &str,
    request: &SampleSeedRequest,
    fingerprint: &str,
    now: i64,
) -> Result<(), StoreError> {
    tx.execute("INSERT INTO catalog_sample_seed_actions(operation_id,actor_id,action,source_sha256,operator_reference,evidence_reference,statement,fingerprint,recorded_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)", &[&request.operation_id,&actor,&action,&APPROVED_SAMPLE_SOURCE_SHA256,&request.operator_reference,&request.evidence_reference,&request.rights_statement,&fingerprint,&now]).await?;
    Ok(())
}
