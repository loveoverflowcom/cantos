//! Retired provider runs (`a1`): no new run starts, existing receipts stay readable and
//! cancellable, and an overdue dispatch resolves to ambiguous rather than being resent.
use cantos_contracts::{
    AdaptationProviderMetadata, AdaptationRunResponse, AdaptationStatus, CancelAdaptationRequest,
    ImportOutcome, StartAdaptationRequest,
};

use super::super::domain::{decide_cancellation, CancellationDecision, FrozenAdaptation};
use super::super::postgres::{self, ActorLock};
use super::{accepted_revision, check_pinned_source, stored_proposal};
use crate::platform::postgres::{authenticate, normalized_uuid, Store, StoreError, Tx};

/// Loads one retired run under its row lock, first resolving an overdue dispatch.
pub(super) async fn run_in_transaction(
    tx: &Tx<'_>,
    actor: &str,
    id: &str,
) -> Result<(AdaptationRunResponse, FrozenAdaptation), StoreError> {
    let initial = postgres::lock_run(tx, actor, id)
        .await?
        .ok_or(StoreError::NotFound)?;
    if initial.input_version() != "a1" {
        return Err(StoreError::NotFound);
    }
    let frozen = initial.frozen_adaptation()?;
    check_pinned_source(tx, actor, &frozen.source).await?;
    postgres::expire_dispatch(tx, id).await?;
    let row = postgres::reload_run(tx, actor, id).await?;
    let status = row.status()?;
    let proposal = stored_proposal(tx, id, &frozen.source, &frozen.binding(id)).await?;
    if matches!(
        status,
        AdaptationStatus::Succeeded | AdaptationStatus::Accepted
    ) && proposal.is_none()
    {
        return Err(StoreError::CorruptRevision);
    }
    let observation = postgres::provider_observation(tx, id).await?;
    let accepted_revision = accepted_revision(tx, id).await?;
    if (status == AdaptationStatus::Accepted) != accepted_revision.is_some() {
        return Err(StoreError::CorruptRevision);
    }
    let run_problem = row.problem()?;
    let response = AdaptationRunResponse {
        id: id.into(),
        created_by: actor.into(),
        recorded_at: row.recorded_at(),
        updated_at: row.updated_at(),
        request: frozen.request.clone(),
        source_sha256: frozen.source.sha256.clone(),
        extractor_version: match &frozen.source.outcome {
            ImportOutcome::Parsed { extraction } => extraction.extractor_version.clone(),
            ImportOutcome::Failed { .. } => return Err(StoreError::CorruptRevision),
        },
        input_content_digest: frozen.base.as_ref().map(|base| base.content_digest.clone()),
        input_export_digest: frozen.base.as_ref().map(|base| base.export_digest.clone()),
        generation_record_id: frozen.generation_record_id.clone(),
        rights_record_id: frozen.rights_record_id.clone(),
        provider: frozen.provider.clone(),
        status,
        proposal,
        problem: run_problem,
        usage: observation.as_ref().and_then(|value| value.usage.clone()),
        cost: observation.and_then(|value| value.cost),
        accepted_revision,
    };
    Ok((response, frozen))
}

impl Store {
    /// The historical inference start endpoint is retired; existing receipts remain readable.
    pub async fn start_adaptation(
        &self,
        _token: &str,
        _request: StartAdaptationRequest,
    ) -> Result<AdaptationRunResponse, StoreError> {
        Err(StoreError::Unavailable)
    }

    pub fn adaptation_provider_metadata(&self) -> Option<AdaptationProviderMetadata> {
        None
    }

    pub async fn load_adaptation(
        &self,
        token: &str,
        id: &str,
    ) -> Result<AdaptationRunResponse, StoreError> {
        normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (response, _) = run_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(response)
    }

    pub async fn cancel_adaptation(
        &self,
        token: &str,
        id: &str,
        request: CancelAdaptationRequest,
    ) -> Result<AdaptationRunResponse, StoreError> {
        normalized_uuid(id)?;
        normalized_uuid(&request.operation_id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        postgres::lock_actor(&tx, ActorLock::Cancel, &actor).await?;
        let (response, _) = run_in_transaction(&tx, &actor, id).await?;
        authenticate(&tx, token).await?;
        let prior = postgres::cancellation(&tx, &actor, &request.operation_id).await?;
        match decide_cancellation(id, prior.as_deref(), response.status) {
            CancellationDecision::Replay => {}
            CancellationDecision::OperationReused => return Err(StoreError::OperationReused),
            CancellationDecision::Refused => return Err(StoreError::InvalidRequest),
            CancellationDecision::Cancel { transition } => {
                if transition {
                    postgres::cancel_legacy_run(&tx, id).await?;
                }
                postgres::insert_cancellation(&tx, &actor, &request.operation_id, id).await?;
            }
        }
        let (response, _) = run_in_transaction(&tx, &actor, id).await?;
        tx.commit().await?;
        Ok(response)
    }
}
