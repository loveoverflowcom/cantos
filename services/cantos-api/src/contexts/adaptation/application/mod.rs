//! Adaptation use cases. Each runs under a per-actor advisory lock and the run's row lock: load
//! the frozen input and its current evidence, ask the pure adaptation rules, write the outcome.
use cantos_contracts::{AdaptationProblem, AdaptationProposal, ImportResponse, RevisionResponse};
use uuid::Uuid;

use super::domain::{
    admit_edited_proposal, admit_output_with_minter, typed_digest, SourceRejection,
    SubmissionRejection, TrustedBinding, PROPOSAL_DIGEST,
};
use super::postgres;
use crate::contexts::editorial::load_revision;
use crate::contexts::ingestion::imported_source;
use crate::platform::postgres::{StoreError, Tx};
use crate::script_ir::read_canonical_script;

mod accept;
mod context;
mod submit;

/// Admits caller output with identities minted here: random, and independent of model text and
/// array position. The deterministic kernel consumes them as facts.
pub fn admit_output(
    bytes: &[u8],
    source: &ImportResponse,
    binding: &TrustedBinding,
) -> Result<AdaptationProposal, AdaptationProblem> {
    admit_output_with_minter(bytes, source, binding, || {
        format!("id_{}", Uuid::new_v4().simple())
    })
}

impl From<SourceRejection> for StoreError {
    fn from(rejection: SourceRejection) -> Self {
        match rejection {
            SourceRejection::NotExtracted => StoreError::InvalidRequest,
            SourceRejection::Issues(issues) => StoreError::InvalidRequestFields(issues),
        }
    }
}

impl From<SubmissionRejection> for StoreError {
    fn from(rejection: SubmissionRejection) -> Self {
        match rejection {
            SubmissionRejection::Issues(issues) => StoreError::InvalidRequestFields(issues),
            SubmissionRejection::AlreadySubmitted => StoreError::ProposalAlreadySubmitted,
        }
    }
}

/// A frozen input pins its source; the registry must still hold exactly that import.
async fn check_pinned_source(
    tx: &Tx<'_>,
    actor: &str,
    source: &ImportResponse,
) -> Result<(), StoreError> {
    let current = imported_source(tx, actor, &source.id)
        .await?
        .ok_or(StoreError::NotFound)?;
    if current != *source {
        return Err(StoreError::CorruptRevision);
    }
    Ok(())
}

/// The recorded proposal, which must still match its digest, be canonical Script IR and keep
/// every trusted binding of its run.
async fn stored_proposal(
    tx: &Tx<'_>,
    run: &str,
    source: &ImportResponse,
    binding: &TrustedBinding,
) -> Result<Option<AdaptationProposal>, StoreError> {
    let Some((proposal, digest)) = postgres::proposal(tx, run).await? else {
        return Ok(None);
    };
    if typed_digest(PROPOSAL_DIGEST, &proposal).map_err(|_| StoreError::CorruptRevision)? != digest
    {
        return Err(StoreError::CorruptRevision);
    }
    read_canonical_script(proposal.script_json.as_bytes())
        .map_err(|_| StoreError::CorruptRevision)?;
    admit_edited_proposal(&proposal.script_json, source, binding)
        .map_err(|_| StoreError::CorruptRevision)?;
    Ok(Some(proposal))
}

/// The revision an acceptance of this run created, if it was accepted.
async fn accepted_revision(tx: &Tx<'_>, run: &str) -> Result<Option<RevisionResponse>, StoreError> {
    match postgres::acceptance_link(tx, run).await? {
        Some((script, revision)) => Ok(Some(load_revision(tx, &script, revision).await?)),
        None => Ok(None),
    }
}
