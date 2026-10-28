//! Persistence for catalog sample-disclosure decisions; authorization belongs to application.
use crate::contexts::catalog::domain::metadata_disclosure::{
    fictional_sample_subject, APPROVED_SAMPLE_SOURCE_SHA256, FICTIONAL_SAMPLE_SOURCE_SHA256,
};

fn table(id: &str) -> &'static str {
    if fictional_sample_subject(id) {
        "catalog_fictional_metadata_disclosures"
    } else {
        "catalog_sample_metadata_disclosures"
    }
}
use crate::platform::store::{Row, StoreError, Tx};

pub(crate) async fn operation(
    tx: &Tx<'_>,
    actor: &str,
    op: &str,
) -> Result<Option<Row>, StoreError> {
    Ok(tx.query_opt("SELECT fingerprint,work_id,subject_kind,subject_id,sequence,decision,recorded_at_ms FROM catalog_sample_metadata_disclosures WHERE owner_id=?1 AND operation_id=?2", &[&actor, &op]).await?)
}
pub(crate) async fn max_sequence(
    tx: &Tx<'_>,
    kind: &str,
    subject: &str,
) -> Result<Row, StoreError> {
    Ok(tx.query_one(&format!("SELECT coalesce(max(sequence),0) AS sequence FROM {} WHERE subject_kind=?1 AND subject_id=?2", table(subject)), &[&kind, &subject]).await?)
}
pub(crate) async fn latest(tx: &Tx<'_>, kind: &str, id: &str) -> Result<Option<Row>, StoreError> {
    Ok(tx.query_opt(&format!("SELECT * FROM {} WHERE subject_kind=?1 AND subject_id=?2 ORDER BY sequence DESC LIMIT 1", table(id)), &[&kind, &id]).await?)
}
pub(crate) async fn work_digest(tx: &Tx<'_>, id: &str, rev: i64) -> Result<Row, StoreError> {
    Ok(tx
        .query_one(
            "SELECT snapshot_digest FROM catalog_work_revisions WHERE work_id=?1 AND revision=?2",
            &[&id, &rev],
        )
        .await?)
}
pub(crate) async fn unit_digest(tx: &Tx<'_>, id: &str, rev: i64) -> Result<Row, StoreError> {
    Ok(tx
        .query_one(
            "SELECT snapshot_digest FROM catalog_unit_revisions WHERE unit_id=?1 AND revision=?2",
            &[&id, &rev],
        )
        .await?)
}
pub(crate) async fn first_work_disclosure(tx: &Tx<'_>, id: &str) -> Result<Row, StoreError> {
    Ok(tx.query_one(&format!("SELECT min(recorded_at_ms) AS first_at FROM {} WHERE subject_kind='work' AND subject_id=?1 AND decision='disclosed'", table(id)), &[&id]).await?)
}
#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_disclosed(
    tx: &Tx<'_>,
    work: &str,
    kind: &str,
    subject: &str,
    sequence: i64,
    actor: &str,
    work_rev: i64,
    work_digest: &str,
    unit_rev: Option<i64>,
    unit_digest: &Option<String>,
    source_id: &str,
    rights_id: &str,
    now: i64,
    operation: &str,
    fingerprint: &str,
) -> Result<(), StoreError> {
    let source = if fictional_sample_subject(subject) {
        FICTIONAL_SAMPLE_SOURCE_SHA256
    } else {
        APPROVED_SAMPLE_SOURCE_SHA256
    };
    tx.execute(&format!("INSERT INTO {}(work_id,subject_kind,subject_id,sequence,owner_id,decision,work_revision,work_digest,unit_revision,unit_digest,source_id,source_sha256,rights_kind,rights_id,recorded_by,recorded_at_ms,operation_id,fingerprint) VALUES(?1,?2,?3,?4,?5,'disclosed',?6,?7,?8,?9,?10,?11,'rights',?12,?5,?13,?14,?15)", table(subject)), &[&work,&kind,&subject,&sequence,&actor,&work_rev,&work_digest,&unit_rev,unit_digest,&source_id,&source,&rights_id,&now,&operation,&fingerprint]).await?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_withdrawn(
    tx: &Tx<'_>,
    work: &str,
    kind: &str,
    subject: &str,
    sequence: i64,
    actor: &str,
    now: i64,
    operation: &str,
    fingerprint: &str,
) -> Result<(), StoreError> {
    tx.execute(&format!("INSERT INTO {}(work_id,subject_kind,subject_id,sequence,owner_id,decision,recorded_by,recorded_at_ms,operation_id,fingerprint) VALUES(?1,?2,?3,?4,?5,'withdrawn',?5,?6,?7,?8)", table(subject)), &[&work,&kind,&subject,&sequence,&actor,&now,&operation,&fingerprint]).await?;
    Ok(())
}
