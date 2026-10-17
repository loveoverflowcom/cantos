//! Acceptance turns a reviewed proposal into the next accepted script revision, keeping every
//! trusted source, attempt and rights binding. It is the only path from a proposal to a head.
use cantos_contracts::{
    AcceptAdaptationRequest, FieldIssue, RevisionResponse, SaveRevisionRequest,
};

use super::super::domain::{admit_edited_proposal, decide_acceptance, AcceptanceDecision};
use super::super::postgres::{self, ActorLock};
use super::context::context_in_transaction;
use crate::contexts::editorial::append_revision;
use crate::platform::postgres::{authenticate, checked_number, normalized_uuid, Store, StoreError};

impl Store {
    pub async fn accept_adaptation(
        &self,
        token: &str,
        id: &str,
        request: AcceptAdaptationRequest,
    ) -> Result<RevisionResponse, StoreError> {
        normalized_uuid(id)?;
        normalized_uuid(&request.operation_id)?;
        checked_number(request.expected_revision)?;
        if !request.reviewed_findings {
            return Err(StoreError::InvalidRequestFields(vec![FieldIssue {
                path: "/reviewed_findings".into(),
                rule: "required_review".into(),
            }]));
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        postgres::lock_actor(&tx, ActorLock::Accept, &actor).await?;
        let (response, context) = context_in_transaction(&tx, &actor, id).await?;
        let binding = context.binding(id);
        let status = response.status;
        let expected_revision = context.request.expected_revision;
        let script_id = context.request.script_id;
        let source = context.source;
        let accepted_revision = response.accepted_revision;
        authenticate(&tx, token).await?;
        let prior = postgres::acceptance_by_operation(&tx, &actor, &request.operation_id).await?;
        let prior = prior.as_ref().map(|(run, prior)| (run.as_str(), prior));
        match decide_acceptance(id, &request, prior, status, expected_revision) {
            AcceptanceDecision::Replay => {
                let saved = accepted_revision.ok_or(StoreError::CorruptRevision)?;
                authenticate(&tx, token).await?;
                tx.commit().await?;
                return Ok(saved);
            }
            AcceptanceDecision::OperationReused => return Err(StoreError::OperationReused),
            AcceptanceDecision::Refused => return Err(StoreError::InvalidRequest),
            AcceptanceDecision::Accept => {}
        }
        let content = admit_edited_proposal(&request.script_json, &source, &binding)
            .map_err(|value| StoreError::InvalidRequestFields(value.issues))?;
        let saved = append_revision(
            &tx,
            &actor,
            &script_id,
            &SaveRevisionRequest {
                expected_revision: request.expected_revision,
                operation_id: request.operation_id.clone(),
                script_json: request.script_json.clone(),
            },
            &content,
        )
        .await?;
        postgres::insert_acceptance(
            &tx,
            postgres::NewAcceptance {
                id,
                actor: &actor,
                request: &request,
                script: &saved.script_id,
                revision: checked_number(saved.revision)?,
            },
        )
        .await?;
        postgres::mark_accepted(&tx, id).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(saved)
    }
}
