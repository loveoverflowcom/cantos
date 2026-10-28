//! Bounded, source-pinned sample reads. The caller owns one read transaction; no private facts
//! or publication rights leave this boundary. This facade never creates playback authority.
use crate::contexts::{
    catalog::{
        domain::{
            metadata_disclosure::{
                approved_sample_ids, fictional_sample_ids, fictional_sample_subject,
                ApprovedSampleSource, APPROVED_SAMPLE_SOURCE_SHA256,
                FICTIONAL_SAMPLE_SOURCE_SHA256,
            },
            ContentUnit, Lifecycle, Revision, Visibility, Work,
        },
        turso,
    },
    ingestion,
};
use crate::platform::store::{actor_active, Row, StoreError, Tx};

pub(crate) struct SampleDisclosedWork {
    pub work: Work,
    pub first_at: i64,
    pub units: Vec<ContentUnit>,
}

fn get<T: crate::platform::store::FromColumn>(row: &Row, name: &str) -> Result<T, StoreError> {
    row.try_get(name).map_err(|_| StoreError::CorruptRevision)
}

async fn latest(tx: &Tx<'_>, kind: &str, id: &str) -> Result<Option<Row>, StoreError> {
    turso::sample_metadata::latest(tx, kind, id).await
}

async fn exact_source(
    tx: &Tx<'_>,
    row: &Row,
    owner: &str,
) -> Result<Option<ApprovedSampleSource>, StoreError> {
    let stored_owner: String = get(row, "owner_id")?;
    let source_id: String = get(row, "source_id")?;
    let source_hash: String = get(row, "source_sha256")?;
    let rights_id: String = get(row, "rights_id")?;
    let rights_kind: String = get(row, "rights_kind")?;
    let subject: String = get(row, "subject_id")?;
    let expected_hash = if fictional_sample_subject(&subject) {
        FICTIONAL_SAMPLE_SOURCE_SHA256
    } else {
        APPROVED_SAMPLE_SOURCE_SHA256
    };
    if stored_owner != owner || rights_kind != "rights" || source_hash != expected_hash {
        return Err(StoreError::CorruptRevision);
    }
    if !super::sample_metadata_rights::bound(tx, owner, &rights_id, &source_id, expected_hash)
        .await?
    {
        return Ok(None);
    }
    let Some(bytes) = ingestion::source_bytes(tx, owner, &source_id).await? else {
        return Ok(None);
    };
    let source = ApprovedSampleSource::parse(&bytes).map_err(|_| StoreError::CorruptRevision)?;
    if source.source_sha256() != expected_hash {
        return Err(StoreError::CorruptRevision);
    }
    Ok(Some(source))
}

fn revision(row: &Row, column: &str) -> Result<Revision, StoreError> {
    let raw: i64 = get(row, column)?;
    Revision::new(u64::try_from(raw).map_err(|_| StoreError::CorruptRevision)?)
        .map_err(|_| StoreError::CorruptRevision)
}

async fn pinned_work(tx: &Tx<'_>, row: &Row, work_id: &str) -> Result<Work, StoreError> {
    let rev = revision(row, "work_revision")?;
    let expected: String = get(row, "work_digest")?;
    let pin = turso::sample_metadata::work_digest(tx, work_id, rev.get() as i64).await?;
    if get::<String>(&pin, "snapshot_digest")? != expected {
        return Err(StoreError::CorruptRevision);
    }
    turso::load_work_revision(tx, work_id, rev)
        .await?
        .ok_or(StoreError::CorruptRevision)
}

async fn pinned_unit(tx: &Tx<'_>, row: &Row, unit_id: &str) -> Result<ContentUnit, StoreError> {
    let rev = revision(row, "unit_revision")?;
    let expected: String = get(row, "unit_digest")?;
    let pin = turso::sample_metadata::unit_digest(tx, unit_id, rev.get() as i64).await?;
    if get::<String>(&pin, "snapshot_digest")? != expected {
        return Err(StoreError::CorruptRevision);
    }
    turso::load_unit_revision(tx, unit_id, rev)
        .await?
        .ok_or(StoreError::CorruptRevision)
}

/// Return only a currently eligible exact-sample disclosure; callers must still union it with
/// independently eligible releases in the same transaction. Work and units are independent.
pub(crate) async fn sample_disclosed_work(
    tx: &Tx<'_>,
    id: &str,
) -> Result<Option<SampleDisclosedWork>, StoreError> {
    let Some((_, ids)) = approved_sample_ids()
        .iter()
        .chain(fictional_sample_ids())
        .find(|(work, _)| *work == id)
    else {
        return Ok(None);
    };
    let Some(row) = latest(tx, "work", id).await? else {
        return Ok(None);
    };
    if get::<String>(&row, "decision")? != "disclosed" {
        return Ok(None);
    }
    let current = turso::load_work(tx, id)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    if current.lifecycle() == Lifecycle::Archived
        || current.details().visibility != Visibility::Public
        || !actor_active(tx, current.owner_id().as_str()).await?
    {
        return Ok(None);
    }
    let Some(source) = exact_source(tx, &row, current.owner_id().as_str()).await? else {
        return Ok(None);
    };
    let work = pinned_work(tx, &row, id).await?;
    if work.owner_id() != current.owner_id()
        || work.lifecycle() == Lifecycle::Archived
        || work.details().visibility != Visibility::Public
        || !source.matches_work(&work)
    {
        return Err(StoreError::CorruptRevision);
    }
    let first = turso::sample_metadata::first_work_disclosure(tx, id).await?;
    let first_at: i64 = get(&first, "first_at")?;
    if first_at < 0 {
        return Err(StoreError::CorruptRevision);
    }
    let mut units = Vec::new();
    for id in ids {
        let Some(row) = latest(tx, "unit", id).await? else {
            continue;
        };
        if get::<String>(&row, "decision")? != "disclosed" {
            continue;
        }
        let current_unit = turso::load_unit(tx, id)
            .await?
            .ok_or(StoreError::CorruptRevision)?;
        if current_unit.work_id() != current.id() {
            return Err(StoreError::CorruptRevision);
        }
        if current_unit.lifecycle() == Lifecycle::Archived {
            continue;
        }
        let Some(source) = exact_source(tx, &row, current.owner_id().as_str()).await? else {
            continue;
        };
        let unit_work = pinned_work(tx, &row, current.id().as_str()).await?;
        let unit = pinned_unit(tx, &row, id).await?;
        if unit_work.owner_id() != current.owner_id()
            || unit_work.lifecycle() == Lifecycle::Archived
            || unit_work.details().visibility != Visibility::Public
            || !source.matches_work(&unit_work)
            || unit.work_id() != current.id()
            || unit.lifecycle() == Lifecycle::Archived
            || !source.matches_unit(&unit)
        {
            return Err(StoreError::CorruptRevision);
        }
        units.push(unit);
    }
    Ok(Some(SampleDisclosedWork {
        work,
        first_at,
        units,
    }))
}
