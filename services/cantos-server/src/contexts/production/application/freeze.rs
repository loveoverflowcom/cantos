//! Freezing preserves the exact previewed inputs as an immutable snapshot. Pending or missing
//! rights can be frozen and inspected, never approved.
use crate::contract::studio::{
    FreezeProductionRequest, ProductionEligibility, ProductionSnapshotResponse,
};
use uuid::Uuid;

use super::super::domain::{check_freeze, check_input_identity, prepare_inputs};
use super::super::postgres::{self, encode};
use super::{facts, production_actor, snapshot};
use crate::contexts::editorial::{load_revision, Action};
use crate::platform::store::{checked_number, normalized_uuid, LockMode, Store, StoreError};

impl Store {
    pub async fn freeze_production(
        &self,
        token: &str,
        script: &str,
        request: FreezeProductionRequest,
    ) -> Result<ProductionSnapshotResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let operation = normalized_uuid(&request.operation_id)?;
        let bytes = encode(&request, 64 * 1024)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Review, LockMode::Update).await?;
        if let Some(prior) =
            postgres::snapshot_by_operation(&tx, &script, &actor, &operation).await?
        {
            if prior.request_bytes() != bytes {
                return Err(StoreError::OperationReused);
            }
            let result = snapshot(&tx, &prior.snapshot(), &facts).await?;
            tx.commit().await?;
            return Ok(result);
        }
        let settings = check_freeze(
            facts.head,
            facts.settings.as_ref(),
            request.expected_revision,
            request.settings_version,
        )?;
        let revision = crate::contexts::editorial::document::revision(
            load_revision(&tx, &script, checked_number(facts.head)?).await?,
        );
        let prepared = prepare_inputs(
            &facts.owner,
            &revision,
            settings,
            &facts.rights,
            facts.reviewed,
            postgres::now(&tx).await?,
        )
        .map_err(StoreError::InvalidRequestFields)?;
        let document = prepared.document();
        check_input_identity(&document.input_digest, &request.input_digest)?;
        let document_bytes = encode(&document, 4 * 1024 * 1024)?;
        let id = Uuid::new_v4().to_string();
        postgres::insert_snapshot(
            &tx,
            postgres::NewSnapshot {
                id: &id,
                script: &script,
                owner: &facts.owner,
                script_revision: checked_number(facts.head)?,
                settings_version: checked_number(settings.version)?,
                operation: &operation,
                actor: &actor,
                input_digest: &document.input_digest,
                request_bytes: &bytes,
                document_bytes: &document_bytes,
            },
        )
        .await?;
        let row = postgres::load_snapshot(&tx, &id, &script).await?;
        let result = snapshot(&tx, &row, &facts).await?;
        production_actor(&tx, token).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn production_snapshot(
        &self,
        token: &str,
        script: &str,
        id: &str,
    ) -> Result<ProductionSnapshotResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let id = normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Read, LockMode::Share).await?;
        let row = postgres::find_snapshot(&tx, &id, &script)
            .await?
            .ok_or(StoreError::NotFound)?;
        let result = snapshot(&tx, &row, &facts).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn production_eligibility(
        &self,
        token: &str,
        script: &str,
        id: &str,
    ) -> Result<ProductionEligibility, StoreError> {
        Ok(self
            .production_snapshot(token, script, id)
            .await?
            .eligibility)
    }
}
