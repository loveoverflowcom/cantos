//! A caller submits generated output against a frozen context. Every submission, valid or not,
//! is an immutable receipt; only a valid one records a proposal, and only once.
use cantos_contracts::{
    AdaptationSubmissionReceipt, AdaptationSubmissionStatus, FieldIssue,
    SubmitAdaptationProposalRequest,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::super::domain::{check_submission, validate_caller_generation};
use super::super::postgres::{self, ActorLock};
use super::admit_output;
use super::context::{context_in_transaction, current_base};
use crate::platform::postgres::{authenticate, normalized_uuid, Store, StoreError};

impl Store {
    pub async fn submit_adaptation_proposal(
        &self,
        token: &str,
        id: &str,
        request: SubmitAdaptationProposalRequest,
    ) -> Result<AdaptationSubmissionReceipt, StoreError> {
        normalized_uuid(id)?;
        normalized_uuid(&request.operation_id)?;
        if request.proposal_json.len() > 1024 * 1024 {
            return Err(StoreError::InvalidRequestFields(vec![FieldIssue {
                path: "/proposal_json".into(),
                rule: "byte_length".into(),
            }]));
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        postgres::lock_actor(&tx, ActorLock::Submit, &actor).await?;
        let (response, context) = context_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        if let Some((receipt, prior)) =
            postgres::submission_by_operation(&tx, &actor, &request.operation_id).await?
        {
            if receipt.run_id != id || prior != request {
                return Err(StoreError::OperationReused);
            }
            if !validate_caller_generation(&receipt.generation, &context.prompt_version).is_empty()
                || receipt.context_digest != response.context_digest
            {
                return Err(StoreError::CorruptRevision);
            }
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(receipt);
        }
        check_submission(
            &response.context_digest,
            &request.context_digest,
            validate_caller_generation(&request.generation, &context.prompt_version),
            response.status,
        )?;
        current_base(
            &tx,
            &actor,
            &context.request.script_id,
            context.request.expected_revision,
        )
        .await?;
        let admitted = admit_output(
            request.proposal_json.as_bytes(),
            &context.source,
            &context.binding(id),
        );
        let (status, proposal, problem) = match admitted {
            Ok(proposal) => (AdaptationSubmissionStatus::Valid, Some(proposal), None),
            Err(error) => (AdaptationSubmissionStatus::Invalid, None, Some(error)),
        };
        let submitted_at = postgres::submission_time(&tx).await?;
        let receipt = AdaptationSubmissionReceipt {
            id: Uuid::new_v4().to_string(),
            run_id: id.into(),
            operation_id: request.operation_id.clone(),
            context_digest: request.context_digest.clone(),
            submitted_by: actor.clone(),
            submitted_at,
            output_sha256: format!("{:x}", Sha256::digest(request.proposal_json.as_bytes())),
            generation: request.generation.clone(),
            status,
            problem,
            proposal: proposal.clone(),
        };
        let number = postgres::next_submission_number(&tx, id).await?;
        postgres::insert_submission(
            &tx,
            postgres::NewSubmission {
                id,
                actor: &actor,
                number,
                request: &request,
                receipt: &receipt,
            },
        )
        .await?;
        if let Some(proposal) = proposal {
            postgres::insert_proposal(&tx, id, &proposal).await?;
            if !postgres::mark_proposal_received(&tx, id).await? {
                return Err(StoreError::ProposalAlreadySubmitted);
            }
        }
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(receipt)
    }
}
