//! Caller-owned contexts (`c1`): freeze the exact source, base revision and instructions a
//! caller adapts from, then read, review or cancel that frozen context.
use cantos_contracts::{
    AdaptationContextRequest, AdaptationContextResponse, AdaptationReviewResponse,
    AdaptationStatus, AdaptationSubmissionStatus, CancelAdaptationRequest, FieldIssue,
    RevisionResponse,
};
use uuid::Uuid;

use super::super::domain::{
    check_context_source, decide_cancellation, generation_record_id, prepare_request,
    rights_record_id, typed_digest, validate_caller_generation, CancellationDecision,
    FrozenContext, CONTRACT_VERSION, INPUT_C1_DIGEST, OUTPUT_SCHEMA, PROMPT_VERSION,
};
use super::super::postgres::{self, ActorLock};
use super::{accepted_revision, check_pinned_source, stored_proposal};
use crate::contexts::editorial::{
    authorize, load_revision, register_evidence, script_exists, Action, EvidenceKind,
};
use crate::contexts::ingestion::imported_source;
use crate::platform::postgres::{
    authenticate, checked_number, normalized_uuid, LockMode, Store, StoreError, Tx,
};

/// Loads one caller context under its row lock and re-checks every stored relation: pinned
/// source, admitted proposal, latest submission and acceptance link.
pub(super) async fn context_in_transaction(
    tx: &Tx<'_>,
    actor: &str,
    id: &str,
) -> Result<(AdaptationContextResponse, FrozenContext), StoreError> {
    let row = postgres::lock_run(tx, actor, id)
        .await?
        .ok_or(StoreError::NotFound)?;
    if row.input_version() != "c1" {
        return Err(StoreError::NotFound);
    }
    let context = row.frozen_context()?;
    check_pinned_source(tx, actor, &context.source).await?;
    let status = row.status()?;
    let proposal = stored_proposal(tx, id, &context.source, &context.binding(id)).await?;
    if matches!(
        status,
        AdaptationStatus::Succeeded | AdaptationStatus::Accepted
    ) && proposal.is_none()
    {
        return Err(StoreError::CorruptRevision);
    }
    let latest_submission = postgres::latest_submission(tx, id).await?;
    if let Some(submission) = &latest_submission {
        if submission.context_digest != row.input_digest()
            || !validate_caller_generation(&submission.generation, &context.prompt_version)
                .is_empty()
            || (submission.status == AdaptationSubmissionStatus::Valid
                && submission.proposal != proposal)
        {
            return Err(StoreError::CorruptRevision);
        }
    }
    if proposal.is_some()
        && latest_submission
            .as_ref()
            .is_none_or(|receipt| receipt.status != AdaptationSubmissionStatus::Valid)
    {
        return Err(StoreError::CorruptRevision);
    }
    let accepted_revision = accepted_revision(tx, id).await?;
    if (status == AdaptationStatus::Accepted) != accepted_revision.is_some() {
        return Err(StoreError::CorruptRevision);
    }
    let response = AdaptationContextResponse {
        id: id.into(),
        created_by: actor.into(),
        recorded_at: row.recorded_at(),
        updated_at: row.updated_at(),
        request: context.request.clone(),
        context_version: context.context_version.clone(),
        context_digest: row.input_digest(),
        prompt_version: context.prompt_version.clone(),
        contract_version: context.contract_version.clone(),
        source: context.source.clone(),
        input_revision: context.base.clone(),
        generation_record_id: context.generation_record_id.clone(),
        rights_record_id: context.rights_record_id.clone(),
        system_prompt: context.system_prompt.clone(),
        user_prompt: context.user_prompt.clone(),
        proposal_schema_json: context.proposal_schema_json.clone(),
        status,
        proposal,
        latest_submission,
        accepted_revision,
    };
    Ok((response, context))
}

/// The base a new context or proposal adapts from: nothing for a script not yet created,
/// otherwise the current head, which only its owner may adapt.
pub(super) async fn current_base(
    tx: &Tx<'_>,
    actor: &str,
    script: &str,
    expected: u64,
) -> Result<Option<RevisionResponse>, StoreError> {
    if !script_exists(tx, script).await? {
        return if expected == 0 {
            Ok(None)
        } else {
            Err(StoreError::NotFound)
        };
    }
    let (owner, head) = authorize(tx, actor, script, Action::Review, LockMode::Update).await?;
    if owner != actor {
        return Err(StoreError::Forbidden);
    }
    if head != expected {
        return Err(StoreError::StaleRevision(head));
    }
    Ok(Some(
        load_revision(tx, script, checked_number(expected)?).await?,
    ))
}

impl Store {
    pub async fn create_adaptation_context(
        &self,
        token: &str,
        request: AdaptationContextRequest,
    ) -> Result<AdaptationContextResponse, StoreError> {
        normalized_uuid(&request.operation_id)?;
        normalized_uuid(&request.script_id)?;
        checked_number(request.expected_revision)?;
        if !request.rights_authorization {
            return Err(StoreError::InvalidRequestFields(vec![FieldIssue {
                path: "/rights_authorization".into(),
                rule: "required_authorization".into(),
            }]));
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        postgres::lock_actor(&tx, ActorLock::Context, &actor).await?;
        authenticate(&tx, token).await?;
        if let Some((prior, version)) =
            postgres::run_by_operation(&tx, &actor, &request.operation_id).await?
        {
            if version != "c1" {
                return Err(StoreError::OperationReused);
            }
            let (response, context) = context_in_transaction(&tx, &actor, &prior).await?;
            if context.request != request {
                return Err(StoreError::OperationReused);
            }
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(response);
        }
        let source = imported_source(&tx, &actor, &request.source_id)
            .await?
            .ok_or(StoreError::NotFound)?;
        check_context_source(&request, &source)?;
        let base = current_base(&tx, &actor, &request.script_id, request.expected_revision).await?;
        let prepared =
            prepare_request(&source, base.as_ref().map(|base| base.script_json.as_str()))
                .map_err(|error| StoreError::InvalidRequestFields(error.issues))?;
        let id = Uuid::new_v4().to_string();
        let context = FrozenContext {
            request: request.clone(),
            source,
            base,
            context_version: "c1".into(),
            prompt_version: PROMPT_VERSION.into(),
            contract_version: CONTRACT_VERSION.into(),
            system_prompt: prepared.system,
            user_prompt: prepared.prompt,
            proposal_schema_json: OUTPUT_SCHEMA.into(),
            generation_record_id: generation_record_id(&id),
            rights_record_id: rights_record_id(&id),
        };
        for (kind, evidence, description) in [
            (
                EvidenceKind::Generation,
                &context.generation_record_id,
                "Caller adaptation submission binding; generation provenance is declared and unverified",
            ),
            (
                EvidenceKind::Rights,
                &context.rights_record_id,
                "Creator source/export/adaptation-use assertion pinned to source checksum; legal and publication eligibility remain unknown",
            ),
        ] {
            register_evidence(&tx, &actor, kind, evidence, description).await?;
        }
        let digest =
            typed_digest(INPUT_C1_DIGEST, &context).map_err(|_| StoreError::CorruptRevision)?;
        let json = serde_json::to_string(&context).map_err(|_| StoreError::Unavailable)?;
        postgres::insert_context_run(
            &tx,
            postgres::NewContextRun {
                id: &id,
                actor: &actor,
                operation: &request.operation_id,
                source: &request.source_id,
                script: &request.script_id,
                frozen_json: &json,
                digest: &digest,
            },
        )
        .await?;
        let (response, _) = context_in_transaction(&tx, &actor, &id).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(response)
    }

    pub async fn load_adaptation_context(
        &self,
        token: &str,
        id: &str,
    ) -> Result<AdaptationContextResponse, StoreError> {
        normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (response, _) = context_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(response)
    }

    pub async fn load_adaptation_review(
        &self,
        token: &str,
        id: &str,
    ) -> Result<AdaptationReviewResponse, StoreError> {
        normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (context, _) = context_in_transaction(&tx, &actor, id).await?;
        let response = AdaptationReviewResponse::Caller {
            context: Box::new(context),
        };
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(response)
    }

    /// A cancelled caller context cannot receive a corrected output or accept its proposal.
    pub async fn cancel_adaptation_context(
        &self,
        token: &str,
        id: &str,
        request: CancelAdaptationRequest,
    ) -> Result<AdaptationContextResponse, StoreError> {
        normalized_uuid(id)?;
        normalized_uuid(&request.operation_id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        postgres::lock_actor(&tx, ActorLock::Cancel, &actor).await?;
        let (response, _) = context_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        let prior = postgres::cancellation(&tx, &actor, &request.operation_id).await?;
        match decide_cancellation(id, prior.as_deref(), response.status) {
            CancellationDecision::Replay => {}
            CancellationDecision::OperationReused => return Err(StoreError::OperationReused),
            CancellationDecision::Refused => return Err(StoreError::InvalidRequest),
            CancellationDecision::Cancel { transition } => {
                if transition {
                    postgres::cancel_context(&tx, id).await?;
                }
                postgres::insert_cancellation(&tx, &actor, &request.operation_id, id).await?;
            }
        }
        let (response, _) = context_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(response)
    }
}
