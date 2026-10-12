//! Production use cases. Each locks the script, then the acting and owning actors, then
//! settings and rights, in that order, reads the current facts and asks the pure production
//! rules before writing. Locks are never held across synthesis.
use cantos_contracts::{
    ProductionRightsClaimResponse, ProductionSettingsResponse, ProductionSnapshotResponse,
};

use super::domain::{evaluate_for_owner, CommandRejection};
use super::postgres::{self, SnapshotRow};
use crate::contexts::editorial::{
    authorize, find_revision, permits, reviewed_by_active_owner, Access, Action,
};
use crate::platform::postgres::{
    authenticate, checked_number, normalized_uuid, LockMode, StoreError, Tx,
};

mod approve;
pub(super) mod briefs;
mod casting;
mod freeze;

impl From<CommandRejection> for StoreError {
    fn from(rejection: CommandRejection) -> Self {
        match rejection {
            CommandRejection::StaleRevision(head) => StoreError::StaleRevision(head),
            CommandRejection::StaleInputs => StoreError::StaleProductionInputs,
            CommandRejection::InvalidRequest => StoreError::InvalidRequest,
            CommandRejection::Blocked(issues) => StoreError::ProductionBlocked(issues),
        }
    }
}

/// The session lock already prevents concurrent revocation; active actor changes must wait too.
pub(crate) async fn production_actor(tx: &Tx<'_>, token: &str) -> Result<String, StoreError> {
    let actor = authenticate(tx, token).await?;
    if !postgres::lock_actor_active(tx, &actor).await? {
        return Err(StoreError::Unauthenticated);
    }
    Ok(actor)
}

/// Current facts one production decision depends on, read under the script lock.
pub(super) struct Facts {
    pub owner: String,
    pub head: u64,
    pub settings: Option<ProductionSettingsResponse>,
    pub rights: Vec<ProductionRightsClaimResponse>,
    pub reviewed: bool,
}

impl Facts {
    pub fn settings_version(&self) -> u64 {
        self.settings
            .as_ref()
            .map_or(0, |settings| settings.version)
    }
}

pub(super) async fn facts(
    tx: &Tx<'_>,
    actor: &str,
    script: &str,
    action: Action,
    lock: LockMode,
) -> Result<Facts, StoreError> {
    // All production writes lock this row before rights/settings rows. Reads share the same order.
    let (owner, head) = authorize(tx, actor, script, action, lock).await?;
    if actor != owner {
        let access = match postgres::lock_member_role(tx, script, actor).await? {
            Some(role) => Access::from_member_role(&role).ok_or(StoreError::CorruptRevision)?,
            None => return Err(StoreError::NotFound),
        };
        if !permits(access, action) {
            return Err(StoreError::Forbidden);
        }
    }
    let owner_active = postgres::lock_actor_active(tx, &owner).await?;
    Ok(Facts {
        settings: postgres::current_settings(tx, script).await?,
        rights: postgres::current_rights(tx, script, &owner).await?,
        reviewed: owner_active
            && reviewed_by_active_owner(tx, script, checked_number(head)?, &owner).await?,
        owner,
        head,
    })
}

/// A stored snapshot, rebuilt from the exact historical rows it pinned and evaluated against
/// current facts. Numbers and self-consistent hashes alone do not establish which settled
/// receipts were pinned, so the revision, settings and rights rows are read again.
pub(super) async fn snapshot(
    tx: &Tx<'_>,
    row: &SnapshotRow,
    facts: &Facts,
) -> Result<ProductionSnapshotResponse, StoreError> {
    let doc = row.document()?;
    let id = row.id();
    let script = row.script_id();
    let operation_id = row.operation_id();
    for value in [&id, &script, &operation_id] {
        normalized_uuid(value).map_err(|_| StoreError::CorruptRevision)?;
    }
    let pinned_revision = find_revision(tx, &script, checked_number(doc.revision.revision)?)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    let pinned_settings =
        postgres::find_settings(tx, &script, checked_number(doc.settings.version)?)
            .await?
            .ok_or(StoreError::CorruptRevision)?;
    let record_ids: Vec<_> = doc
        .rights
        .iter()
        .map(|claim| claim.claim.declaration.record_id.clone())
        .collect();
    let versions: Vec<_> = doc
        .rights
        .iter()
        .map(|claim| checked_number(claim.claim.version))
        .collect::<Result<_, _>>()?;
    let pinned_rights =
        postgres::pinned_rights(tx, &script, &doc.owner_id, &record_ids, &versions).await?;
    if pinned_rights.len() != doc.rights.len() {
        return Err(StoreError::CorruptRevision);
    }
    super::domain::admit_frozen_document(&doc, &pinned_revision, &pinned_settings, &pinned_rights)
        .map_err(|_| StoreError::CorruptRevision)?;
    let approval = postgres::approval_for(tx, &id, &script).await?;
    if approval
        .as_ref()
        .is_some_and(|approval| approval.input_digest != doc.input_digest)
    {
        return Err(StoreError::CorruptRevision);
    }
    let eligibility = evaluate_for_owner(
        &doc,
        &facts.owner,
        facts.head,
        facts.settings_version(),
        &facts.rights,
        facts.reviewed,
        approval.as_ref(),
        postgres::now(tx).await?,
    );
    Ok(ProductionSnapshotResponse {
        id,
        script_id: script,
        created_by: row.created_by(),
        recorded_at: row.recorded_at(),
        operation_id,
        document: doc,
        approval,
        eligibility,
    })
}

pub(crate) use briefs::audio_brief;
pub(crate) use briefs::producer_token_registered;
