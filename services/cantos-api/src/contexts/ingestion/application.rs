//! Ingestion use cases: record an import receipt once per operation and read it back to its
//! owner, plus the legacy text view of a source cited by a script.
use cantos_contracts::{ImportRequest, ImportResponse, SourceResponse};
use uuid::Uuid;

use super::domain::extraction::{extract, MAX_SOURCE_BYTES};
use super::domain::receipt::{
    field_issue, legacy_source_intact, metadata_issues, original_text, receipt_digest, sha256_hex,
    source_id_valid,
};
use super::postgres;
use crate::contexts::editorial::{authorize, register_evidence, Action, EvidenceKind};
use crate::platform::postgres::{authenticate, normalized_uuid, LockMode, Store, StoreError};

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
        let sha256 = sha256_hex(&request.original_bytes);
        let outcome = extract(&request.original_bytes, request.metadata.format);
        let import_digest = receipt_digest(&request.metadata, &outcome, &sha256)
            .map_err(|_| StoreError::CorruptRevision)?;
        let metadata_json =
            serde_json::to_string(&request.metadata).map_err(|_| StoreError::InvalidRequest)?;
        let outcome_json = serde_json::to_string(&outcome).map_err(|_| StoreError::Unavailable)?;
        let original_text = original_text(&request.original_bytes);
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        postgres::lock_actor_imports(&tx, &actor).await?;
        authenticate(&tx, token).await?;
        if let Some((prior, bytes)) =
            postgres::import_by_operation(&tx, &actor, &request.metadata.operation_id).await?
        {
            if bytes != request.original_bytes || prior.metadata != request.metadata {
                return Err(StoreError::OperationReused);
            }
            tx.commit().await?;
            return Ok(prior);
        }
        let id = format!("src_{}", Uuid::new_v4().simple());
        register_evidence(
            &tx,
            &actor,
            EvidenceKind::Source,
            &id,
            "Imported manuscript; rights recorded as supplied, never publication clearance",
        )
        .await?;
        postgres::insert_import(
            &tx,
            postgres::NewImport {
                actor: &actor,
                id: &id,
                metadata: &request.metadata,
                original_text: original_text.as_deref(),
                sha256: &sha256,
                original_bytes: &request.original_bytes,
                metadata_json: &metadata_json,
                outcome_json: &outcome_json,
                import_digest: &import_digest,
            },
        )
        .await?;
        let response = postgres::load_import(&tx, &actor, &id).await?;
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
        let result = postgres::find_import(&tx, &actor, id)
            .await?
            .ok_or(StoreError::NotFound)?;
        tx.commit().await?;
        Ok(result)
    }

    /// Private source bytes are reachable only through a currently authorized script link.
    pub async fn source(
        &self,
        token: &str,
        script: &str,
        id: &str,
    ) -> Result<SourceResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (owner, _) = authorize(&tx, &actor, &script, Action::Read, LockMode::Share).await?;
        let source = postgres::linked_source(&tx, &owner, id, &script)
            .await?
            .ok_or(StoreError::NotFound)?;
        // The legacy source contract is text-only; binary intake uses /imports/{id}/original.
        let original_text = source.original_text.ok_or(StoreError::InvalidRequest)?;
        if !legacy_source_intact(
            &original_text,
            source.original_bytes.as_deref(),
            &source.sha256,
        ) {
            return Err(StoreError::CorruptRevision);
        }
        let response = SourceResponse {
            id: source.id,
            reference: source.reference,
            original_text,
            sha256: source.sha256,
            recorded_at: source.recorded_at,
        };
        tx.commit().await?;
        Ok(response)
    }
}
