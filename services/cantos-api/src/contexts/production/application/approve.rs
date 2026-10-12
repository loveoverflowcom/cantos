//! Owner approval of one frozen snapshot: the exact input identity, eligible when recorded and
//! still current when the transaction commits.
use cantos_contracts::{ApproveProductionRequest, ProductionApprovalResponse};
use uuid::Uuid;

use super::super::domain::{check_approval, require_current_approval};
use super::super::postgres::{self, encode};
use super::{facts, production_actor, snapshot};
use crate::contexts::editorial::Action;
use crate::platform::postgres::{normalized_uuid, LockMode, Store, StoreError};

impl Store {
    pub async fn approve_production(
        &self,
        token: &str,
        script: &str,
        id: &str,
        request: ApproveProductionRequest,
    ) -> Result<ProductionApprovalResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let id = normalized_uuid(id)?;
        let operation = normalized_uuid(&request.operation_id)?;
        let bytes = encode(&request, 64 * 1024)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Review, LockMode::Update).await?;
        if let Some(prior) =
            postgres::approval_by_operation(&tx, &script, &actor, &operation).await?
        {
            if prior.request_bytes() != bytes || prior.snapshot_id() != id {
                return Err(StoreError::OperationReused);
            }
            let result = prior.approval()?;
            tx.commit().await?;
            return Ok(result);
        }
        let row = postgres::find_snapshot(&tx, &id, &script)
            .await?
            .ok_or(StoreError::NotFound)?;
        let candidate = snapshot(&tx, &row, &facts).await?;
        check_approval(
            &candidate.document.input_digest,
            &candidate.eligibility,
            candidate.approval.is_some(),
            &request.input_digest,
        )?;
        postgres::insert_approval(
            &tx,
            postgres::NewApproval {
                id: &Uuid::new_v4().to_string(),
                snapshot: &id,
                script: &script,
                operation: &operation,
                actor: &actor,
                input_digest: &request.input_digest,
                request_bytes: &bytes,
            },
        )
        .await?;
        // A rights term can expire while waiting for a lock. Evaluate again immediately before commit.
        let approved = snapshot(&tx, &row, &facts).await?;
        require_current_approval(&approved.eligibility)?;
        production_actor(&tx, token).await?;
        let result = approved.approval.ok_or(StoreError::CorruptRevision)?;
        tx.commit().await?;
        Ok(result)
    }
}
