//! Immutable local manuscript intake on the existing source registry.
use cantos_contracts::{FieldIssue, ImportMetadata, ImportOutcome, ImportRequest, ImportResponse};
use sha2::{Digest, Sha256};
use tokio_postgres::Row;
use uuid::Uuid;

use super::{authenticate, normalized_uuid, Store, StoreError};
use crate::imports::{extract, MAX_SOURCE_BYTES};

pub(super) const IMPORT_COLUMNS: &str = "id,reference,imported_by,import_operation_id,import_digest,original_bytes,original_text,sha256,import_metadata::text AS import_metadata,import_outcome::text AS import_outcome,to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at";

fn receipt_digest(
    metadata: &ImportMetadata,
    outcome: &ImportOutcome,
    sha256: &str,
) -> Result<String, StoreError> {
    // Hash typed serialization, never JSONB's object ordering or PostgreSQL pretty printing.
    let metadata = serde_json::to_vec(metadata).map_err(|_| StoreError::CorruptRevision)?;
    let outcome = serde_json::to_vec(outcome).map_err(|_| StoreError::CorruptRevision)?;
    let mut digest = Sha256::new();
    digest.update(b"cantos/manuscript-import/r1\n");
    digest.update(sha256.as_bytes());
    digest.update(b"\n");
    digest.update(metadata);
    digest.update(b"\n");
    digest.update(outcome);
    Ok(format!("{:x}", digest.finalize()))
}

fn field_issue(path: &str, rule: &str) -> FieldIssue {
    FieldIssue {
        path: path.into(),
        rule: rule.into(),
    }
}

fn metadata_issues(metadata: &ImportMetadata) -> Vec<FieldIssue> {
    fn display_issues(value: &str, limit: usize, path: &str, issues: &mut Vec<FieldIssue>) {
        if value.trim().is_empty() {
            issues.push(field_issue(path, "nonblank"));
        }
        if value.len() > limit {
            issues.push(field_issue(path, "byte_length"));
        }
        if value.chars().any(char::is_control) {
            issues.push(field_issue(path, "control_character"));
        }
    }
    let mut issues = Vec::new();
    if normalized_uuid(&metadata.operation_id).is_err() {
        issues.push(field_issue("/metadata/operation_id", "canonical_uuid"));
    }
    display_issues(&metadata.file_name, 255, "/metadata/file_name", &mut issues);
    display_issues(
        &metadata.reference,
        2048,
        "/metadata/reference",
        &mut issues,
    );
    for (value, path) in [
        (&metadata.rights_holder, "/metadata/rights_holder"),
        (
            &metadata.permission_evidence,
            "/metadata/permission_evidence",
        ),
        (&metadata.usage_scope, "/metadata/usage_scope"),
    ] {
        if let Some(value) = value {
            display_issues(value, 2048, path, &mut issues);
        }
    }
    issues
}

fn source_id_valid(id: &str) -> bool {
    id.len() <= 64
        && !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
}

fn original_text(bytes: &[u8]) -> Option<String> {
    // PostgreSQL text cannot contain NUL; the original bytes remain the authority.
    std::str::from_utf8(bytes)
        .ok()
        .filter(|text| !text.is_empty() && !text.contains('\0'))
        .map(str::to_owned)
}

pub(super) fn decode_import(row: &Row) -> Result<(ImportResponse, Vec<u8>), StoreError> {
    let bytes: Vec<u8> = row
        .try_get("original_bytes")
        .map_err(|_| StoreError::CorruptRevision)?;
    let sha256: String = row.get("sha256");
    if bytes.len() > MAX_SOURCE_BYTES || format!("{:x}", Sha256::digest(&bytes)) != sha256 {
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
    if receipt_digest(&metadata, &outcome, &sha256)? != row.get::<_, String>("import_digest") {
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

impl Store {
    /// A retry gets the exact immutable receipt, including a failed extraction.
    /// Extraction is pure and bounded; no provider or external resource is contacted.
    pub async fn import_manuscript(
        &self,
        token: &str,
        request: ImportRequest,
    ) -> Result<ImportResponse, StoreError> {
        let mut issues = metadata_issues(&request.metadata);
        if request.original_bytes.len() > MAX_SOURCE_BYTES {
            issues.push(field_issue("/original_bytes", "byte_length"));
        }
        if !issues.is_empty() {
            return Err(StoreError::InvalidRequestFields(issues));
        }
        let sha256 = format!("{:x}", Sha256::digest(&request.original_bytes));
        let outcome = extract(&request.original_bytes, request.metadata.format);
        let import_digest = receipt_digest(&request.metadata, &outcome, &sha256)?;
        let metadata_json =
            serde_json::to_string(&request.metadata).map_err(|_| StoreError::InvalidRequest)?;
        let outcome_json = serde_json::to_string(&outcome).map_err(|_| StoreError::Unavailable)?;
        let original_text = original_text(&request.original_bytes);
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        // Serializes operations for this actor without granting UPDATE on actor identities.
        // A hash collision only adds contention; queries still enforce the exact owner.
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended('manuscript-import:' || $1::text, 0))",
            &[&actor],
        )
        .await?;
        authenticate(&tx, token).await?;
        let query = format!("SELECT {IMPORT_COLUMNS} FROM source_records WHERE imported_by=$1 AND import_operation_id=$2");
        if let Some(row) = tx
            .query_opt(&query, &[&actor, &request.metadata.operation_id])
            .await?
        {
            let (prior, bytes) = decode_import(&row)?;
            if bytes != request.original_bytes || prior.metadata != request.metadata {
                return Err(StoreError::OperationReused);
            }
            tx.commit().await?;
            return Ok(prior);
        }
        let id = format!("src_{}", Uuid::new_v4().simple());
        tx.execute(
            "INSERT INTO script_evidence(owner_id,kind,id,description) VALUES($1,'source',$2,'Imported manuscript; rights recorded as supplied, never publication clearance')",
            &[&actor, &id],
        )
        .await?;
        tx.execute(
            "INSERT INTO source_records(owner_id,id,reference,original_text,sha256,original_bytes,imported_by,import_operation_id,import_metadata,import_outcome,import_digest) VALUES($1,$2,$3,$4,$5,$6,$1,$7,$8::text::jsonb,$9::text::jsonb,$10)",
            &[&actor, &id, &request.metadata.reference, &original_text, &sha256, &request.original_bytes, &request.metadata.operation_id, &metadata_json, &outcome_json, &import_digest],
        )
        .await?;
        let query =
            format!("SELECT {IMPORT_COLUMNS} FROM source_records WHERE owner_id=$1 AND id=$2");
        let (response, _) = decode_import(&tx.query_one(&query, &[&actor, &id]).await?)?;
        tx.commit().await?;
        Ok(response)
    }

    pub async fn load_import(&self, token: &str, id: &str) -> Result<ImportResponse, StoreError> {
        self.read_import(token, id)
            .await
            .map(|(receipt, _)| receipt)
    }

    /// Owner-scoped original bytes; filename metadata is never a response header or path.
    pub async fn original_import(&self, token: &str, id: &str) -> Result<Vec<u8>, StoreError> {
        self.read_import(token, id).await.map(|(_, bytes)| bytes)
    }

    async fn read_import(
        &self,
        token: &str,
        id: &str,
    ) -> Result<(ImportResponse, Vec<u8>), StoreError> {
        if !source_id_valid(id) {
            return Err(StoreError::InvalidRequest);
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let query = format!("SELECT {IMPORT_COLUMNS} FROM source_records WHERE owner_id=$1 AND id=$2 AND imported_by=$1");
        let row = tx
            .query_opt(&query, &[&actor, &id])
            .await?
            .ok_or(StoreError::NotFound)?;
        let result = decode_import(&row)?;
        tx.commit().await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use cantos_contracts::{ImportFormat, ImportMetadata};

    use super::{field_issue, metadata_issues};

    #[test]
    fn metadata_diagnostics_collect_independent_safe_paths_and_rules() {
        let metadata = ImportMetadata {
            operation_id: "invalid operation".into(),
            file_name: "Đ".repeat(128),
            format: ImportFormat::Txt,
            reference: " \n".into(),
            rights_holder: Some(" ".into()),
            permission_evidence: None,
            usage_scope: Some("private\0scope".into()),
        };
        assert_eq!(
            metadata_issues(&metadata),
            vec![
                field_issue("/metadata/operation_id", "canonical_uuid"),
                field_issue("/metadata/file_name", "byte_length"),
                field_issue("/metadata/reference", "nonblank"),
                field_issue("/metadata/reference", "control_character"),
                field_issue("/metadata/rights_holder", "nonblank"),
                field_issue("/metadata/usage_scope", "control_character"),
            ]
        );
    }
}
