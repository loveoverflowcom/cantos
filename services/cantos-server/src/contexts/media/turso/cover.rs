//! Release-pinned public cover facts (ADR 0033) and the owner's append-only disclosure rows.
//! A page reads its pinned artwork, assets and latest decisions in three bounded statements,
//! never one per item. Every row is restored through Media constructors; nothing is decided here.
use std::collections::BTreeMap;

use crate::contexts::catalog::domain::{OwnerId, WorkId};
use crate::contexts::media::domain::{
    artwork::ArtworkChange,
    cover_disclosure::{CoverDisclosureRecord, StoredDisclosure},
};
use crate::platform::store::{checked_number, Row, StoreError, Tx};

use super::{
    artwork::change_row,
    image::{asset_row, corrupt, StoredImageAsset},
};

/// The facts one representative release needs for `project_public_cover`, owned by the read.
#[derive(Clone, Default)]
pub(in crate::contexts::media) struct PinnedCoverFacts {
    /// The latest artwork change at or before the pinned Work revision (a clear is kept).
    pub change: Option<ArtworkChange>,
    /// The admitted asset that change selects, with its private location for the origin read.
    pub asset: Option<StoredImageAsset>,
    /// The latest decision by server sequence for this Work and that asset digest.
    pub disclosure: Option<CoverDisclosureRecord>,
    /// A stored row of this Work failed admission. The cover alone falls back; the page stays.
    pub corrupt: bool,
    pub metadata_revision: Option<u64>,
}

/// One pinned Work snapshot of a representative release.
pub(in crate::contexts::media) struct PinnedWork {
    pub work_id: String,
    pub owner_id: String,
    pub revision: u64,
}

const EFFECTIVE_CHANGES: &str = "WITH pinned AS (SELECT json_extract(value,'$[0]') AS work_id,json_extract(value,'$[1]') AS owner_id,json_extract(value,'$[2]') AS revision FROM json_each(?1))
SELECT c.* FROM pinned p JOIN media_work_artwork_changes c ON c.work_id=p.work_id AND c.owner_id=p.owner_id
WHERE c.work_metadata_revision=(SELECT max(c2.work_metadata_revision) FROM media_work_artwork_changes c2 WHERE c2.work_id=p.work_id AND c2.owner_id=p.owner_id AND c2.work_metadata_revision<=p.revision)";
const SELECTED_ASSETS: &str =
    "SELECT * FROM media_image_assets WHERE id IN (SELECT value FROM json_each(?1))";
const LATEST_DISCLOSURES: &str = "WITH wanted AS (SELECT json_extract(value,'$[0]') AS work_id,json_extract(value,'$[1]') AS digest FROM json_each(?1))
SELECT d.*,m.work_metadata_revision AS metadata_revision,m.evidence_reference AS metadata_evidence FROM wanted w JOIN media_cover_disclosures d ON d.work_id=w.work_id AND d.image_asset_digest=w.digest LEFT JOIN media_metadata_cover_bindings m ON m.work_id=d.work_id AND m.disclosure_sequence=d.sequence
WHERE d.sequence=(SELECT max(d2.sequence) FROM media_cover_disclosures d2 WHERE d2.work_id=w.work_id AND d2.image_asset_digest=w.digest)";

fn json<T: serde::Serialize>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(|_| StoreError::CorruptRevision)
}

/// Cover facts for each distinct pinned Work of a page, keyed by Work ID. At most three
/// statements run whatever the page size; an empty page runs none.
pub(in crate::contexts::media) async fn pinned_cover_facts(
    tx: &Tx<'_>,
    pinned: &[PinnedWork],
) -> Result<BTreeMap<String, PinnedCoverFacts>, StoreError> {
    let mut facts: BTreeMap<String, PinnedCoverFacts> = BTreeMap::new();
    if pinned.is_empty() {
        return Ok(facts);
    }
    let mut owners = BTreeMap::new();
    let mut keys = Vec::new();
    for p in pinned {
        if owners
            .insert(p.work_id.clone(), p.owner_id.clone())
            .is_some()
        {
            return Err(StoreError::CorruptRevision);
        }
        keys.push((
            p.work_id.as_str(),
            p.owner_id.as_str(),
            checked_number(p.revision)?,
        ));
        facts.insert(p.work_id.clone(), PinnedCoverFacts::default());
    }
    // A row that fails admission marks only its own Work, so one damaged cover cannot hide
    // other titles (ADR 0033 rule 3). Statement failures still fail the read.
    for row in tx.query(EFFECTIVE_CHANGES, &[&json(&keys)?]).await? {
        let work: String = corrupt(row.try_get("work_id"))?;
        let entry = facts.get_mut(&work).ok_or(StoreError::CorruptRevision)?;
        match change_row(&row) {
            Ok(change) if change.work_id().as_str() == work && entry.change.is_none() => {
                entry.change = Some(change);
            }
            _ => entry.corrupt = true,
        }
    }
    let selected: Vec<String> = facts
        .values()
        .filter(|f| !f.corrupt)
        .filter_map(|f| f.change.as_ref()?.change().selection())
        .map(|s| s.image_asset_id().as_str().to_owned())
        .collect();
    if selected.is_empty() {
        return Ok(facts);
    }
    let mut assets = BTreeMap::new();
    for row in tx.query(SELECTED_ASSETS, &[&json(&selected)?]).await? {
        let id: String = corrupt(row.try_get("id"))?;
        // A damaged asset row stays absent: the kernel turns a missing asset into Corrupt.
        if let Ok(stored) = asset_row(&row) {
            if stored.asset.id().as_str() == id {
                assets.insert(id, stored);
            }
        }
    }
    let mut wanted = Vec::new();
    for (work, entry) in &mut facts {
        if entry.corrupt {
            continue;
        }
        let Some(selection) = entry.change.as_ref().and_then(|c| c.change().selection()) else {
            continue;
        };
        entry.asset = assets.get(selection.image_asset_id().as_str()).cloned();
        wanted.push((work.clone(), selection.image_asset_digest().to_owned()));
    }
    for row in tx.query(LATEST_DISCLOSURES, &[&json(&wanted)?]).await? {
        let work: String = corrupt(row.try_get("work_id"))?;
        let entry = facts.get_mut(&work).ok_or(StoreError::CorruptRevision)?;
        let Ok(record) = disclosure_row(&row) else {
            entry.corrupt = true;
            continue;
        };
        let revision: Option<i64> = corrupt(row.try_get("metadata_revision"))?;
        let evidence: Option<String> = corrupt(row.try_get("metadata_evidence"))?;
        match (revision, evidence) {
            (Some(revision), Some(evidence))
                if revision > 0
                    && !evidence.trim().is_empty()
                    && evidence.len() <= 1024
                    && !evidence.chars().any(char::is_control) =>
            {
                entry.metadata_revision = Some(revision as u64);
            }
            (None, None) => {}
            _ => entry.corrupt = true,
        }
        // A decision recorded by anyone but the pinned owner is no consent: leave it undecided.
        if owners.get(&work).map(String::as_str) != Some(record.owner_id().as_str()) {
            continue;
        }
        if record.work_id().as_str() != work || entry.disclosure.replace(record).is_some() {
            entry.corrupt = true;
        }
    }
    Ok(facts)
}

fn disclosure_row(row: &Row) -> Result<CoverDisclosureRecord, StoreError> {
    let recorded_by: String = corrupt(row.try_get("recorded_by"))?;
    let record = CoverDisclosureRecord::restore(StoredDisclosure {
        work_id: corrupt(row.try_get("work_id"))?,
        sequence: corrupt(row.try_get("sequence"))?,
        owner_id: corrupt(row.try_get("owner_id"))?,
        image_asset_digest: corrupt(row.try_get("image_asset_digest"))?,
        decision: corrupt(row.try_get("decision"))?,
        valid_until_ms: corrupt(row.try_get("valid_until_ms"))?,
        recorded_at_ms: corrupt(row.try_get("recorded_at_ms"))?,
        operation_id: corrupt(row.try_get("operation_id"))?,
        fingerprint: corrupt(row.try_get("fingerprint"))?,
    })
    .map_err(|_| StoreError::CorruptRevision)?;
    if recorded_by != record.owner_id().as_str() {
        return Err(StoreError::CorruptRevision);
    }
    Ok(record)
}

pub(in crate::contexts::media) async fn prior_disclosure(
    tx: &Tx<'_>,
    owner: &OwnerId,
    operation: &str,
) -> Result<Option<CoverDisclosureRecord>, StoreError> {
    tx.query_opt(
        "SELECT * FROM media_cover_disclosures WHERE owner_id=?1 AND operation_id=?2",
        &[&owner.as_str(), &operation],
    )
    .await?
    .as_ref()
    .map(disclosure_row)
    .transpose()
}

pub(in crate::contexts::media) async fn next_sequence(
    tx: &Tx<'_>,
    work: &WorkId,
) -> Result<u64, StoreError> {
    let row = tx
        .query_one(
            "SELECT coalesce(max(sequence),0)+1 AS next FROM media_cover_disclosures WHERE work_id=?1",
            &[&work.as_str()],
        )
        .await?;
    u64::try_from(corrupt(row.try_get::<_, i64>("next"))?).map_err(|_| StoreError::CorruptRevision)
}

/// Whether the digest names an admitted image of exactly this Work and owner.
pub(in crate::contexts::media) async fn image_bound(
    tx: &Tx<'_>,
    owner: &OwnerId,
    work: &WorkId,
    digest: &str,
) -> Result<bool, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT id FROM media_image_assets WHERE owner_id=?1 AND work_id=?2 AND digest=?3 LIMIT 1",
            &[&owner.as_str(), &work.as_str(), &digest],
        )
        .await?
        .is_some())
}

pub(in crate::contexts::media) async fn insert_disclosure(
    tx: &Tx<'_>,
    record: &CoverDisclosureRecord,
) -> Result<(), StoreError> {
    let decision = record.decision();
    tx.execute(
        "INSERT INTO media_cover_disclosures(work_id,sequence,owner_id,image_asset_digest,decision,valid_until_ms,recorded_by,recorded_at_ms,operation_id,fingerprint) VALUES(?1,?2,?3,?4,?5,?6,?3,?7,?8,?9)",
        &[
            &record.work_id().as_str(),
            &checked_number(record.sequence())?,
            &record.owner_id().as_str(),
            &record.image_asset_digest(),
            &decision.kind(),
            &decision.valid_until_ms(),
            &record.recorded_at_ms(),
            &record.operation_id().as_str(),
            &record.fingerprint(),
        ],
    )
    .await
    .map_err(|error| {
        if error.constraint() == Some(cantos_store_turso::error::ConstraintKind::Unique) {
            StoreError::OperationReused
        } else {
            StoreError::from(error)
        }
    })?;
    Ok(())
}

pub(in crate::contexts::media) async fn metadata_binding(
    tx: &Tx<'_>,
    record: &CoverDisclosureRecord,
) -> Result<Option<(u64, String)>, StoreError> {
    let row = tx.query_opt("SELECT work_metadata_revision,evidence_reference FROM media_metadata_cover_bindings WHERE work_id=?1 AND disclosure_sequence=?2",
        &[&record.work_id().as_str(), &checked_number(record.sequence())?]).await?;
    row.map(|row| {
        let revision: i64 = corrupt(row.try_get("work_metadata_revision"))?;
        let evidence: String = corrupt(row.try_get("evidence_reference"))?;
        if revision <= 0
            || evidence.trim().is_empty()
            || evidence.len() > 1024
            || evidence.chars().any(char::is_control)
        {
            return Err(StoreError::CorruptRevision);
        }
        Ok((revision as u64, evidence))
    })
    .transpose()
}

pub(in crate::contexts::media) async fn insert_metadata_binding(
    tx: &Tx<'_>,
    record: &CoverDisclosureRecord,
    revision: u64,
    evidence: &str,
) -> Result<(), StoreError> {
    tx.execute("INSERT INTO media_metadata_cover_bindings(work_id,disclosure_sequence,work_metadata_revision,evidence_reference) VALUES(?1,?2,?3,?4)",
        &[&record.work_id().as_str(), &checked_number(record.sequence())?, &checked_number(revision)?, &evidence]).await?;
    Ok(())
}

#[cfg(test)]
#[path = "../../../test_support/cover_turso_tests.rs"]
mod tests;
