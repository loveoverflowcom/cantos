//! Shared projections use the S06 evaluator, never a SQL/UI copy. One caller-owned snapshot.
use super::super::{
    domain::{
        metadata_publication::{visible_entries, Live, PublicationState},
        viewer_access::*,
        WorkId,
    },
    turso::{self, metadata_publication as generations, public_metadata, viewer_reads},
};
use super::metadata_publication_read::{PublicUnit, PublicWork};
use crate::platform::{
    identity::domain::ActorId,
    store::{StoreError, Tx},
};

pub(crate) struct SharedWork {
    pub row: PublicWork,
    pub entries: super::super::domain::metadata_publication::Generation,
    pub deadline_us: Option<i64>,
    hidden: std::collections::BTreeSet<String>,
}

pub(crate) async fn shared_work(
    tx: &Tx<'_>,
    id: &WorkId,
    actor: &ActorId,
) -> Result<Option<SharedWork>, StoreError> {
    let PublicationState::Published(published) = generations::state(tx, id.as_str()).await? else {
        return Ok(None);
    };
    let Some(current) = turso::load_work(tx, id.as_str()).await? else {
        return Ok(None);
    };
    let work = generations::pinned_work(tx, id.as_str(), published.generation().work()).await?;
    let owner_active = crate::platform::store::actor_active(tx, work.owner_id().as_str()).await?;
    let mut rights = !work.details().rights_refs.is_empty();
    for reference in &work.details().rights_refs {
        rights &= crate::contexts::editorial::evidence_exists(
            tx,
            work.owner_id().as_str(),
            "rights",
            reference,
        )
        .await?;
    }
    let access = viewer_reads::facts(tx, id.as_str(), work.owner_id(), actor).await?;
    let viewer = Some(Viewer {
        actor,
        active: true,
        memberships: &access.members,
    });
    let facts = WorkFacts {
        published: published.lifecycle() == Live::Active,
        owner_active,
        archived: current.lifecycle().as_str() == "archived",
        rights,
        gate: Gate {
            owner: work.owner_id(),
            audience: access.audience,
            grants: &access.grants,
        },
    };
    if !can_view_work(facts, viewer, tx.now_micros()) {
        return Ok(None);
    }
    let mut hidden =
        public_metadata::archived_units(tx, id.as_str(), published.generation().number()).await?;
    let rules =
        super::super::turso::viewer_access::units::rules(tx, id.as_str(), work.owner_id()).await?;
    for entry in published.generation().entries() {
        let unit = entry.unit();
        if !can_view_unit(
            facts,
            true,
            hidden.contains(unit.as_str()),
            true,
            rules.audience(unit.as_str(), work.owner_id()),
            viewer,
            tx.now_micros(),
        ) {
            hidden.insert(unit.as_str().into());
        }
    }
    let unit_count = visible_entries(published.generation(), |unit| {
        hidden.contains(unit.as_str())
    })
    .len();
    let deadline_us = [access.deadline_us, rules.deadline_us]
        .into_iter()
        .flatten()
        .min();
    Ok(Some(SharedWork {
        row: PublicWork {
            work,
            generation: published.generation().number(),
            first_visible_at: published.first_visible_at(),
            unit_count: u32::try_from(unit_count).map_err(|_| StoreError::CorruptRevision)?,
        },
        entries: published.generation().clone(),
        deadline_us,
        hidden,
    }))
}

pub(crate) async fn shared_units(
    tx: &Tx<'_>,
    work: &SharedWork,
    after: u32,
    limit: u8,
) -> Result<(Vec<PublicUnit>, Option<u32>), StoreError> {
    let id = work.row.work.id().as_str();
    let mut rows: Vec<_> =
        visible_entries(&work.entries, |unit| work.hidden.contains(unit.as_str()))
            .into_iter()
            .filter(|row| row.position > after)
            .take(usize::from(limit) + 1)
            .collect();
    let next = if rows.len() > usize::from(limit) {
        rows.pop();
        rows.last().map(|row| row.position)
    } else {
        None
    };
    let pins: Vec<_> = rows
        .iter()
        .map(|row| (row.entry.unit(), row.entry.pin()))
        .collect();
    let units = public_metadata::pinned_units(tx, id, work.row.work.kind(), &pins).await?;
    Ok((
        rows.into_iter()
            .zip(units)
            .map(|(row, unit)| PublicUnit {
                unit,
                ordinal: row.ordinal,
                position: row.position,
                audio: row.entry.audio().cloned(),
            })
            .collect(),
        next,
    ))
}

pub(crate) use viewer_reads::{candidates as shared_candidates, scope as shared_scope};

pub(crate) async fn shared_unit(
    tx: &Tx<'_>,
    work: &SharedWork,
    unit: &super::super::domain::ContentUnitId,
) -> Result<Option<PublicUnit>, StoreError> {
    let id = work.row.work.id().as_str();
    let Some(row) = visible_entries(&work.entries, |unit| work.hidden.contains(unit.as_str()))
        .into_iter()
        .find(|row| row.entry.unit() == unit)
    else {
        return Ok(None);
    };
    let mut pinned = public_metadata::pinned_units(
        tx,
        id,
        work.row.work.kind(),
        &[(row.entry.unit(), row.entry.pin())],
    )
    .await?;
    Ok(Some(PublicUnit {
        unit: pinned.pop().ok_or(StoreError::CorruptRevision)?,
        ordinal: row.ordinal,
        position: row.position,
        audio: row.entry.audio().cloned(),
    }))
}
