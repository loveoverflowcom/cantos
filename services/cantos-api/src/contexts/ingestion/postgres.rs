//! The source registry: immutable original bytes, import metadata and extraction outcomes.
//! Every load re-checks the bytes, metadata and outcome against the stored receipt digest.
use cantos_contracts::{ImportMetadata, ImportResponse};
use tokio_postgres::Row;

use super::domain::extraction::MAX_SOURCE_BYTES;
use super::domain::receipt::{metadata_issues, original_text, receipt_digest, sha256_hex};
use crate::platform::postgres::{StoreError, Tx};

const IMPORT_COLUMNS: &str = "id,reference,imported_by,import_operation_id,import_digest,original_bytes,original_text,sha256,import_metadata::text AS import_metadata,import_outcome::text AS import_outcome,to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at";

fn decode_import(row: &Row) -> Result<(ImportResponse, Vec<u8>), StoreError> {
    let bytes: Vec<u8> = row
        .try_get("original_bytes")
        .map_err(|_| StoreError::CorruptRevision)?;
    let sha256: String = row.get("sha256");
    if bytes.len() > MAX_SOURCE_BYTES || sha256_hex(&bytes) != sha256 {
        return Err(StoreError::CorruptRevision);
    }
    let metadata: ImportMetadata = serde_json::from_str(&row.get::<_, String>("import_metadata"))
        .map_err(|_| StoreError::CorruptRevision)?;
    if !metadata_issues(&metadata).is_empty()
        || metadata.operation_id != row.get::<_, String>("import_operation_id")
        || metadata.reference != row.get::<_, String>("reference")
    {
        return Err(StoreError::CorruptRevision);
    }
    let original_text_value: Option<String> = row.get("original_text");
    if original_text_value != original_text(&bytes) {
        return Err(StoreError::CorruptRevision);
    }
    let outcome = serde_json::from_str(&row.get::<_, String>("import_outcome"))
        .map_err(|_| StoreError::CorruptRevision)?;
    if receipt_digest(&metadata, &outcome, &sha256).map_err(|_| StoreError::CorruptRevision)?
        != row.get::<_, String>("import_digest")
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok((
        ImportResponse {
            id: row.get("id"),
            imported_by: row.get("imported_by"),
            recorded_at: row.get("recorded_at"),
            sha256,
            byte_len: bytes.len() as u64,
            metadata,
            outcome,
            original_text: original_text_value,
        },
        bytes,
    ))
}

/// Serializes import operations of one actor without granting UPDATE on actor identities.
/// A hash collision only adds contention; queries still enforce the exact owner.
pub(super) async fn lock_actor_imports(tx: &Tx<'_>, actor: &str) -> Result<(), StoreError> {
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended('manuscript-import:' || $1::text, 0))",
        &[&actor],
    )
    .await?;
    Ok(())
}

pub(super) async fn import_by_operation(
    tx: &Tx<'_>,
    actor: &str,
    operation: &str,
) -> Result<Option<(ImportResponse, Vec<u8>)>, StoreError> {
    let query = format!("SELECT {IMPORT_COLUMNS} FROM source_records WHERE imported_by=$1 AND import_operation_id=$2");
    tx.query_opt(&query, &[&actor, &operation])
        .await?
        .as_ref()
        .map(decode_import)
        .transpose()
}

pub(super) struct NewImport<'a> {
    pub actor: &'a str,
    pub id: &'a str,
    pub metadata: &'a ImportMetadata,
    pub original_text: Option<&'a str>,
    pub sha256: &'a str,
    pub original_bytes: &'a [u8],
    pub metadata_json: &'a str,
    pub outcome_json: &'a str,
    pub import_digest: &'a str,
}

pub(super) async fn insert_import(tx: &Tx<'_>, new: NewImport<'_>) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO source_records(owner_id,id,reference,original_text,sha256,original_bytes,imported_by,import_operation_id,import_metadata,import_outcome,import_digest) VALUES($1,$2,$3,$4,$5,$6,$1,$7,$8::text::jsonb,$9::text::jsonb,$10)",
        &[&new.actor, &new.id, &new.metadata.reference, &new.original_text, &new.sha256, &new.original_bytes, &new.metadata.operation_id, &new.metadata_json, &new.outcome_json, &new.import_digest],
    )
    .await?;
    Ok(())
}

/// A source this transaction just inserted.
pub(super) async fn load_import(
    tx: &Tx<'_>,
    owner: &str,
    id: &str,
) -> Result<ImportResponse, StoreError> {
    let query = format!("SELECT {IMPORT_COLUMNS} FROM source_records WHERE owner_id=$1 AND id=$2");
    decode_import(&tx.query_one(&query, &[&owner, &id]).await?).map(|(response, _)| response)
}

pub(super) async fn find_import(
    tx: &Tx<'_>,
    actor: &str,
    id: &str,
) -> Result<Option<(ImportResponse, Vec<u8>)>, StoreError> {
    let query = format!("SELECT {IMPORT_COLUMNS} FROM source_records WHERE owner_id=$1 AND id=$2 AND imported_by=$1");
    tx.query_opt(&query, &[&actor, &id])
        .await?
        .as_ref()
        .map(decode_import)
        .transpose()
}

/// A source the actor imported, for adaptation contexts that pin it by checksum.
pub(crate) async fn imported_source(
    tx: &Tx<'_>,
    actor: &str,
    id: &str,
) -> Result<Option<ImportResponse>, StoreError> {
    let query = format!(
        "SELECT {} FROM source_records WHERE owner_id=$1 AND imported_by=$1 AND id=$2",
        IMPORT_COLUMNS
    );
    tx.query_opt(&query, &[&actor, &id])
        .await?
        .as_ref()
        .map(|row| decode_import(row).map(|(source, _)| source))
        .transpose()
}

/// A text source cited by an accepted revision of `script`, as the legacy source route reads it.
pub(super) struct LinkedSource {
    pub id: String,
    pub reference: String,
    pub original_text: Option<String>,
    pub original_bytes: Option<Vec<u8>>,
    pub sha256: String,
    pub recorded_at: String,
}

pub(super) async fn linked_source(
    tx: &Tx<'_>,
    owner: &str,
    id: &str,
    script: &str,
) -> Result<Option<LinkedSource>, StoreError> {
    let row = tx.query_opt("SELECT id,reference,original_text,original_bytes,sha256,to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at FROM source_records s WHERE owner_id=$1 AND id=$2 AND EXISTS(SELECT 1 FROM revision_evidence e WHERE e.script_id=$3 AND e.owner_id=s.owner_id AND e.kind='source' AND e.evidence_id=s.id)", &[&owner,&id,&script]).await?;
    Ok(row.map(|row: Row| LinkedSource {
        id: row.get("id"),
        reference: row.get("reference"),
        original_text: row.get("original_text"),
        original_bytes: row.get("original_bytes"),
        sha256: row.get("sha256"),
        recorded_at: row.get("recorded_at"),
    }))
}
