//! Transactional Studio catalog commands and their durable outcome.
use super::super::{
    domain::{plan_reorder, ContentUnit, NewContentUnit, NewWork, OwnerId, Work},
    postgres,
};
use super::{
    authorize, current_actor, decode_outcome, links, require, rights, unit, work, CatalogCommand,
    CatalogOutcome, CatalogStoreError, MemberRole, MAX_UNITS,
};
use crate::contexts::editorial::{Access, Action};
use crate::platform::postgres::{authenticate, normalized_uuid, LockMode, Store, StoreError, Tx};

impl Store {
    pub async fn catalog_execute(
        &self,
        token: &str,
        operation: &str,
        command: CatalogCommand,
    ) -> Result<CatalogOutcome, CatalogStoreError> {
        let operation = normalized_uuid(operation)?;
        let bytes = serde_json::to_vec(&command).map_err(|_| StoreError::InvalidRequest)?;
        if bytes.len() > 131072 {
            return Err(CatalogStoreError::Limit);
        }
        let fingerprint = postgres::digest(&bytes);
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        // One bounded metadata lane makes all list epochs and permission changes atomic.
        // This intentionally simple initial implementation is assessed in the read inventory.
        postgres::lock_clock(&tx, LockMode::Update).await?;
        let actor = current_actor(&tx, token).await?;
        let (existing, access) = match &command {
            CatalogCommand::CreateWork { id, .. } => {
                let existing = postgres::load_work(&tx, id.as_str(), LockMode::Update).await?;
                let access = match &existing {
                    Some(work) => authorize(&tx, &actor, work, Action::Write).await?,
                    None => Access::Owner,
                };
                (existing, access)
            }
            CatalogCommand::EditWork { id, .. }
            | CatalogCommand::PatchWork { id, .. }
            | CatalogCommand::ArchiveWork { id, .. } => {
                let work = work(&tx, id, LockMode::Update).await?;
                let action = if matches!(command, CatalogCommand::ArchiveWork { .. }) {
                    Action::Review
                } else {
                    Action::Write
                };
                let access = authorize(&tx, &actor, &work, action).await?;
                (Some(work), access)
            }
            CatalogCommand::CreateUnit { work_id, .. }
            | CatalogCommand::Reorder { work_id, .. }
            | CatalogCommand::SetMember { work_id, .. } => {
                let work = work(&tx, work_id, LockMode::Update).await?;
                let action = if matches!(command, CatalogCommand::SetMember { .. }) {
                    Action::Review
                } else {
                    Action::Write
                };
                let access = authorize(&tx, &actor, &work, action).await?;
                (Some(work), access)
            }
            CatalogCommand::EditUnit { id, .. }
            | CatalogCommand::PatchUnit { id, .. }
            | CatalogCommand::ArchiveUnit { id, .. } => {
                let unit = unit(&tx, id).await?;
                let work = work(&tx, unit.work_id(), LockMode::Update).await?;
                let action = if matches!(command, CatalogCommand::ArchiveUnit { .. }) {
                    Action::Review
                } else {
                    Action::Write
                };
                let access = authorize(&tx, &actor, &work, action).await?;
                (Some(work), access)
            }
        };
        // Every replay reauthorizes current access; recorded results grant no rights.
        authenticate(&tx, token).await?;
        if let Some((prior_fingerprint, result)) =
            postgres::prior_operation(&tx, &actor, &operation).await?
        {
            if prior_fingerprint != fingerprint {
                return Err(CatalogStoreError::OperationReused);
            }
            let outcome = decode_outcome(&result)?;
            authenticate(&tx, token).await?;
            tx.commit().await.map_err(StoreError::from)?;
            return Ok(outcome);
        }
        let at = postgres::now(&tx).await?;
        let outcome = execute(&tx, &actor, command, existing, access, at).await?;
        let result = serde_json::to_vec(&outcome).map_err(|_| StoreError::CorruptRevision)?;
        postgres::record_operation(&tx, &actor, &operation, &fingerprint, &result).await?;
        postgres::bump_clock(&tx).await?;
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(outcome)
    }
}

async fn execute(
    tx: &Tx<'_>,
    actor: &str,
    command: CatalogCommand,
    existing: Option<Work>,
    access: Access,
    at: i64,
) -> Result<CatalogOutcome, CatalogStoreError> {
    if let CatalogCommand::CreateWork { id, kind, details } = command {
        if existing.is_some() {
            return Err(CatalogStoreError::DuplicateId);
        }
        let work = Work::new(NewWork {
            id,
            kind,
            owner_id: OwnerId::parse(actor)?,
            details,
            at,
        })?;
        rights(tx, work.owner_id(), work.details()).await?;
        postgres::save_work(tx, &work, actor, true).await?;
        return Ok(CatalogOutcome {
            work,
            units: Vec::new(),
        });
    }
    let work = existing.ok_or(StoreError::CorruptRevision)?;
    let at = at.max(work.updated_at());
    let mut changed_units = Vec::new();
    let updated = match command {
        CatalogCommand::CreateWork { .. } => return Err(StoreError::CorruptRevision.into()),
        CatalogCommand::EditWork {
            expected_revision,
            details,
            ..
        } => {
            if work.details().rights_refs != details.rights_refs
                || work.details().visibility != details.visibility
            {
                require(access, Action::Review)?;
            }
            let updated = work.edit(expected_revision, details, at)?;
            rights(tx, updated.owner_id(), updated.details()).await?;
            updated
        }
        CatalogCommand::PatchWork {
            expected_revision,
            patch,
            ..
        } => {
            let details = patch.apply(work.details());
            if work.details().rights_refs != details.rights_refs
                || work.details().visibility != details.visibility
            {
                require(access, Action::Review)?;
            }
            let updated = work.edit(expected_revision, details, at)?;
            rights(tx, updated.owner_id(), updated.details()).await?;
            updated
        }
        CatalogCommand::ArchiveWork {
            expected_revision, ..
        } => work.archive(expected_revision, at)?,
        CatalogCommand::CreateUnit {
            expected_work_revision,
            id,
            kind,
            details,
            ..
        } => {
            let updated = work.touch(expected_work_revision, at)?;
            if let Some(existing_unit) = postgres::load_unit(tx, id.as_str()).await? {
                return Err(if existing_unit.work_id() == work.id() {
                    CatalogStoreError::DuplicateId
                } else {
                    StoreError::NotFound.into()
                });
            }
            let units = postgres::units_for_reorder(tx, work.id().as_str()).await?;
            if units.len() >= MAX_UNITS {
                return Err(CatalogStoreError::Limit);
            }
            let position = units
                .iter()
                .map(ContentUnit::position)
                .max()
                .map(|position| position.checked_add(1).ok_or(CatalogStoreError::Limit))
                .transpose()?
                .unwrap_or(0);
            let unit = ContentUnit::new(
                &work,
                NewContentUnit {
                    id,
                    kind,
                    details,
                    position,
                    at,
                },
            )?;
            links(tx, work.owner_id(), unit.details()).await?;
            postgres::save_unit(tx, &unit, actor, true).await?;
            changed_units.push(unit);
            updated
        }
        CatalogCommand::EditUnit {
            id,
            expected_revision,
            expected_work_revision,
            details,
        } => {
            let updated = work.touch(expected_work_revision, at)?;
            let unit = unit(tx, &id).await?;
            let changed =
                unit.edit(&work, expected_revision, details, at.max(unit.updated_at()))?;
            links(tx, work.owner_id(), changed.details()).await?;
            postgres::save_unit(tx, &changed, actor, false).await?;
            changed_units.push(changed);
            updated
        }
        CatalogCommand::PatchUnit {
            id,
            expected_revision,
            expected_work_revision,
            patch,
        } => {
            let updated = work.touch(expected_work_revision, at)?;
            let unit = unit(tx, &id).await?;
            let details = patch.apply(unit.details());
            let changed =
                unit.edit(&work, expected_revision, details, at.max(unit.updated_at()))?;
            links(tx, work.owner_id(), changed.details()).await?;
            postgres::save_unit(tx, &changed, actor, false).await?;
            changed_units.push(changed);
            updated
        }
        CatalogCommand::ArchiveUnit {
            id,
            expected_revision,
            expected_work_revision,
        } => {
            let updated = work.touch(expected_work_revision, at)?;
            let unit = unit(tx, &id).await?;
            let changed = unit.archive(&work, expected_revision, at.max(unit.updated_at()))?;
            postgres::save_unit(tx, &changed, actor, false).await?;
            changed_units.push(changed);
            updated
        }
        CatalogCommand::Reorder {
            expected_revision,
            ordered,
            ..
        } => {
            let updated = work.touch(expected_revision, at)?;
            let units = postgres::units_for_reorder(tx, work.id().as_str()).await?;
            if units.len() > MAX_UNITS {
                return Err(CatalogStoreError::Limit);
            }
            let plan = plan_reorder(&work, &units, &ordered)?;
            for assignment in plan.assignments() {
                let unit = units
                    .iter()
                    .find(|unit| unit.id() == assignment.unit_id())
                    .ok_or(StoreError::CorruptRevision)?;
                if unit.position() != assignment.position() {
                    let changed = unit.with_position(
                        &work,
                        unit.revision(),
                        assignment.position(),
                        at.max(unit.updated_at()),
                    )?;
                    postgres::save_unit(tx, &changed, actor, false).await?;
                }
            }
            updated
        }
        CatalogCommand::SetMember {
            expected_revision,
            actor_id,
            role,
            ..
        } => {
            let updated = work.with_permission_change(expected_revision, at)?;
            if &actor_id == work.owner_id() {
                return Err(StoreError::InvalidRequest.into());
            }
            if role.is_some() && !postgres::lock_actor(tx, actor_id.as_str()).await? {
                return Err(StoreError::NotFound.into());
            }
            postgres::set_member(
                tx,
                work.id().as_str(),
                actor_id.as_str(),
                role.map(MemberRole::as_str),
            )
            .await?;
            updated
        }
    };
    postgres::save_work(tx, &updated, actor, false).await?;
    Ok(CatalogOutcome {
        work: updated,
        units: changed_units,
    })
}
