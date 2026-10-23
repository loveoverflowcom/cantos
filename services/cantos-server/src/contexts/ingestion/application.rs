//! Ingestion use cases: record an import receipt once per operation and read it back to its
//! owner, plus the legacy text view of a source cited by a script.
use cantos_contracts::{ImportRequest, ImportResponse, SourceResponse};
use uuid::Uuid;

use super::domain::extraction::extract;
use super::domain::receipt::{
    legacy_source_intact, original_text, receipt_digest, sha256_hex, source_id_valid,
    AdmittedImport,
};
use super::turso;
use crate::contexts::editorial::{
    authorize, ensure_evidence, register_evidence, Action, EvidenceKind,
};
use crate::platform::store::{authenticate, normalized_uuid, LockMode, Store, StoreError};

impl Store {
    /// The stopped-file operator has validated these bounded text inputs. No HTTP route calls
    /// this capability; the owning contexts still coordinate evidence and source atomically.
    pub(crate) async fn record_source_for_operator(
        &self,
        owner: &str,
        id: &str,
        reference: &str,
        text: &str,
    ) -> Result<(), StoreError> {
        let sha256 = sha256_hex(text.as_bytes());
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        ensure_evidence(&tx, owner, EvidenceKind::Source, id, reference).await?;
        turso::insert_text_source(&tx, owner, id, reference, text, &sha256).await?;
        tx.commit().await?;
        Ok(())
    }

    /// A retry gets the exact immutable receipt, including a failed extraction.
    /// Extraction is pure and bounded; no provider or external resource is contacted.
    pub async fn import_manuscript(
        &self,
        token: &str,
        request: ImportRequest,
    ) -> Result<ImportResponse, StoreError> {
        let request =
            AdmittedImport::try_from(request).map_err(StoreError::InvalidRequestFields)?;
        let sha256 = sha256_hex(request.bytes());
        let outcome = extract(request.bytes(), request.metadata().format);
        let import_digest = receipt_digest(request.metadata(), &outcome, &sha256)
            .map_err(|_| StoreError::CorruptRevision)?;
        let outcome_json = serde_json::to_string(&outcome).map_err(|_| StoreError::Unavailable)?;
        let original_text = original_text(request.bytes());
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        if let Some((prior, bytes)) =
            turso::import_by_operation(&tx, &actor, &request.metadata().operation_id).await?
        {
            if bytes != request.bytes() || prior.metadata != *request.metadata() {
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
        turso::insert_import(
            &tx,
            turso::NewImport {
                actor: &actor,
                id: &id,
                input: &request,
                original_text: original_text.as_deref(),
                sha256: &sha256,
                outcome_json: &outcome_json,
                import_digest: &import_digest,
            },
        )
        .await?;
        let response = turso::load_import(&tx, &actor, &id).await?;
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
        let client = self.connection().await?;
        let tx = client.read().await?;
        let actor = authenticate(&tx, token).await?;
        let result = turso::find_import(&tx, &actor, id)
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
        let client = self.connection().await?;
        let tx = client.read().await?;
        let actor = authenticate(&tx, token).await?;
        let (owner, _) = authorize(&tx, &actor, &script, Action::Read, LockMode::Share).await?;
        let source = turso::linked_source(&tx, &owner, id, &script)
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
