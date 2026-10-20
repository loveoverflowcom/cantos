//! Editorial use cases. Each runs in one transaction: authenticate, lock and load facts, ask the
//! pure editorial rules, persist the outcome and commit.
use super::{ReviewFacts, RevisionFacts, RevisionHistory, RevisionOperation};

use super::domain::{
    decide_review, decide_save, export_digest, permits, Access, Action, PriorOperation,
    ReviewDecision, SaveDecision,
};
use super::postgres;
use crate::platform::postgres::{
    authenticate, checked_number, normalized_uuid, LockMode, Store, StoreError, Tx,
};
use crate::script_ir::{read_script, ScriptContent};

/// Script access for `actor`, with the script row locked in `lock` mode. Returns the owner and
/// the current head revision. A stranger cannot learn that the script exists.
pub(crate) async fn authorize(
    tx: &Tx<'_>,
    actor: &str,
    script: &str,
    action: Action,
    lock: LockMode,
) -> Result<(String, u64), StoreError> {
    let (owner, head) = postgres::lock_script(tx, script, lock)
        .await?
        .ok_or(StoreError::NotFound)?;
    let access = if owner == actor {
        Access::Owner
    } else {
        match postgres::member_role(tx, script, actor).await? {
            Some(role) => Access::from_member_role(&role).ok_or(StoreError::CorruptRevision)?,
            None => Access::None,
        }
    };
    if !permits(access, action) {
        return Err(if access == Access::None {
            StoreError::NotFound
        } else {
            StoreError::Forbidden
        });
    }
    Ok((
        owner,
        u64::try_from(head).map_err(|_| StoreError::CorruptRevision)?,
    ))
}

/// Shared storage-acceptance path. The caller commits this together with its own receipt.
pub(crate) async fn append_revision(
    tx: &Tx<'_>,
    actor: &str,
    script: &str,
    request: RevisionOperation<'_>,
    content: &ScriptContent,
) -> Result<RevisionFacts, StoreError> {
    let operation = normalized_uuid(request.operation_id)?;
    let expected = checked_number(request.expected_revision)?;
    let export = content.export_bytes();
    let digest = content.content_digest().to_string();
    let export_hash = export_digest(&export);
    // Concurrent first saves contend on the unique ID, then the same locked head.
    // A failed/stale/unauthorized first save rolls this provisional script back too.
    postgres::create_script(tx, script, actor).await?;
    let (owner, head) = authorize(tx, actor, script, Action::Write, LockMode::Update).await?;
    let prior = postgres::accepted_operation(tx, script, actor, &operation).await?;
    let prior_export = prior.as_ref().map(postgres::AcceptedOperation::export);
    let prior_summary = prior
        .as_ref()
        .zip(prior_export.as_deref())
        .map(|(row, bytes)| PriorOperation {
            revision: row.revision(),
            expected_revision: row.expected_revision(),
            export: bytes,
        });
    let revision = match decide_save(head, request.expected_revision, &export, prior_summary) {
        SaveDecision::Replay { .. } => {
            return prior.ok_or(StoreError::CorruptRevision)?.into_revision();
        }
        SaveDecision::Append { revision } => revision,
        SaveDecision::Stale { current_revision } => {
            return Err(StoreError::StaleRevision(current_revision))
        }
        SaveDecision::OperationReused => return Err(StoreError::OperationReused),
        SaveDecision::RevisionLimit => return Err(StoreError::InvalidRequest),
    };
    for (kind, id) in content.evidence_refs() {
        if !postgres::evidence_exists(tx, &owner, kind, id).await? {
            return Err(StoreError::EvidenceUnavailable);
        }
    }
    let revision_number = checked_number(revision)?;
    postgres::insert_revision(
        tx,
        postgres::NewRevision {
            script,
            revision: revision_number,
            expected,
            operation: &operation,
            actor,
            export: &export,
            content_digest: &digest,
            export_digest: &export_hash,
        },
    )
    .await?;
    for (kind, id) in content.evidence_refs() {
        postgres::link_revision_evidence(tx, script, revision_number, &owner, kind, id).await?;
    }
    if !postgres::advance_head(tx, script, revision_number, expected).await? {
        return Err(StoreError::StaleRevision(head));
    }
    postgres::load_revision(tx, script, revision_number).await
}

impl Store {
    /// Both head and historical reads reauthorize against current PostgreSQL facts.
    pub(crate) async fn load_editorial_revision(
        &self,
        token: &str,
        script: &str,
        revision: Option<u64>,
    ) -> Result<RevisionFacts, StoreError> {
        let script = normalized_uuid(script)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (_, head) = authorize(&tx, &actor, &script, Action::Read, LockMode::Share).await?;
        let revision = checked_number(revision.unwrap_or(head))?;
        let response = postgres::find_revision(&tx, &script, revision)
            .await?
            .ok_or(StoreError::NotFound)?;
        tx.commit().await?;
        Ok(response)
    }

    pub(crate) async fn save_editorial_revision(
        &self,
        token: &str,
        script: &str,
        request: RevisionOperation<'_>,
        script_json: &str,
    ) -> Result<RevisionFacts, StoreError> {
        let script = normalized_uuid(script)?;
        normalized_uuid(request.operation_id)?;
        checked_number(request.expected_revision)?;
        // Pure admission happens before starting a transaction; rejection writes nothing.
        let content = read_script(script_json.as_bytes()).map_err(StoreError::InvalidScript)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let result = append_revision(&tx, &actor, &script, request, &content).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        // A dropped response after this commit is reconciled by the same operation key.
        Ok(result)
    }

    /// Every page is reauthorized and pinned to the locked head observed for that page.
    pub(crate) async fn editorial_history(
        &self,
        token: &str,
        script: &str,
        after: u64,
        limit: u64,
    ) -> Result<RevisionHistory, StoreError> {
        if !(1..=50).contains(&limit) {
            return Err(StoreError::InvalidRequest);
        }
        let script = normalized_uuid(script)?;
        let after = checked_number(after)?;
        let limit = checked_number(limit)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (_, head) = authorize(&tx, &actor, &script, Action::Read, LockMode::Share).await?;
        let revisions = postgres::revisions_after(&tx, &script, after, limit).await?;
        let last = revisions.last().map(|revision| revision.revision);
        let reviews = match last {
            Some(last) => {
                postgres::reviews_between(&tx, &script, after, checked_number(last)?).await?
            }
            None => Vec::new(),
        };
        tx.commit().await?;
        Ok(RevisionHistory {
            revisions,
            reviews,
            next_after: last.filter(|last| *last < head),
        })
    }

    /// Editorial review is an owner action, distinct from storage acceptance and publication.
    pub(crate) async fn review_editorial_revision(
        &self,
        token: &str,
        script: &str,
        revision: u64,
        operation_id: &str,
    ) -> Result<ReviewFacts, StoreError> {
        let script = normalized_uuid(script)?;
        let operation = normalized_uuid(operation_id)?;
        let requested_revision = revision;
        let revision = checked_number(revision)?;
        if revision < 1 {
            return Err(StoreError::InvalidRequest);
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = authenticate(&tx, token).await?;
        let (_, head) = authorize(&tx, &actor, &script, Action::Review, LockMode::Update).await?;
        let prior_revision = postgres::review_operation(&tx, &script, &actor, &operation).await?;
        let existing = postgres::review_exists(&tx, &script, revision).await?;
        match decide_review(head, requested_revision, prior_revision, existing) {
            ReviewDecision::OperationReused => return Err(StoreError::OperationReused),
            ReviewDecision::Stale { current_revision } => {
                return Err(StoreError::StaleRevision(current_revision))
            }
            ReviewDecision::Replay => {
                if !existing {
                    return Err(StoreError::CorruptRevision);
                }
            }
            ReviewDecision::Create => {
                // Never record a review for corrupted stored content.
                postgres::load_revision(&tx, &script, revision).await?;
                postgres::insert_review(&tx, &script, revision, &operation, &actor).await?;
            }
        }
        if prior_revision.is_none() {
            postgres::insert_review_operation(&tx, &script, &actor, &operation, revision).await?;
        }
        let result = postgres::load_review(&tx, &script, revision).await?;
        tx.commit().await?;
        Ok(result)
    }
}
