//! Persistence for metadata publication generations (server.db v31) and their audio pins
//! (v32). The application owns the transaction, owner authority and the pure decision; this
//! module stores admitted values and rechecks every pin against the stored revision bytes when
//! it reads them back.
use std::collections::BTreeMap;

use super::super::domain::{
    metadata_publication::{
        AudioAssociation, AudioBasis, AudioRef, Generation, GenerationNumber, Live, Pin,
        PublicationState, PublicationVersion, Published,
    },
    ContentUnitId, Revision, Work,
};
use super::{decode_work, digest};
use crate::platform::store::{checked_number, Row, StoreError, Tx};

pub(super) fn get<T: crate::platform::store::FromColumn>(
    row: &Row,
    name: &str,
) -> Result<T, StoreError> {
    row.try_get(name).map_err(|_| StoreError::CorruptRevision)
}

pub(super) fn positive(row: &Row, name: &str) -> Result<u64, StoreError> {
    let raw: i64 = get(row, name)?;
    u64::try_from(raw)
        .ok()
        .filter(|value| *value > 0)
        .ok_or(StoreError::CorruptRevision)
}

pub(super) fn pin(row: &Row, revision: &str, digest: &str) -> Result<Pin, StoreError> {
    let revision =
        Revision::new(positive(row, revision)?).map_err(|_| StoreError::CorruptRevision)?;
    Pin::new(revision, &get::<String>(row, digest)?).map_err(|_| StoreError::CorruptRevision)
}

/// The receipt of one settled owner operation.
pub(crate) struct Receipt {
    pub work_id: String,
    pub version: u64,
    pub fingerprint: String,
}

pub(crate) async fn operation(
    tx: &Tx<'_>,
    owner: &str,
    operation: &str,
) -> Result<Option<Receipt>, StoreError> {
    let row = tx
        .query_opt(
            "SELECT work_id,version,fingerprint FROM catalog_metadata_publication_events WHERE owner_id=?1 AND operation_id=?2",
            &[&owner, &operation],
        )
        .await?;
    row.map(|row| {
        Ok(Receipt {
            work_id: get(&row, "work_id")?,
            version: positive(&row, "version")?,
            fingerprint: get(&row, "fingerprint")?,
        })
    })
    .transpose()
}

pub(crate) async fn generation(
    tx: &Tx<'_>,
    work: &str,
    number: GenerationNumber,
) -> Result<Option<Generation>, StoreError> {
    let number_value = checked_number(number.get())?;
    let Some(row) = tx
        .query_opt(
            "SELECT work_revision,work_digest,entry_count FROM catalog_metadata_generations WHERE work_id=?1 AND generation=?2",
            &[&work, &number_value],
        )
        .await?
    else {
        return Ok(None);
    };
    let rows = tx
        .query(
            "SELECT position,unit_id,unit_revision,unit_digest FROM catalog_metadata_generation_entries WHERE work_id=?1 AND generation=?2 ORDER BY position",
            &[&work, &number_value],
        )
        .await?;
    let count: i64 = get(&row, "entry_count")?;
    if usize::try_from(count).ok() != Some(rows.len()) {
        return Err(StoreError::CorruptRevision);
    }
    let mut audio = associations(tx, work, number_value).await?;
    let mut entries = Vec::with_capacity(rows.len());
    for (index, entry) in rows.iter().enumerate() {
        let position: i64 = get(entry, "position")?;
        if usize::try_from(position).ok() != Some(index + 1) {
            return Err(StoreError::CorruptRevision);
        }
        let raw: String = get(entry, "unit_id")?;
        let unit = ContentUnitId::parse(&raw).map_err(|_| StoreError::CorruptRevision)?;
        let association = audio.remove(&raw);
        entries.push((
            unit,
            pin(entry, "unit_revision", "unit_digest")?,
            association,
        ));
    }
    // Every pin names an entry of its own generation (also a foreign key).
    if !audio.is_empty() {
        return Err(StoreError::CorruptRevision);
    }
    Generation::restore(number, pin(&row, "work_revision", "work_digest")?, entries)
        .map(Some)
        .map_err(|_| StoreError::CorruptRevision)
}

async fn associations(
    tx: &Tx<'_>,
    work: &str,
    generation: i64,
) -> Result<BTreeMap<String, AudioAssociation>, StoreError> {
    let rows = tx
        .query(
            "SELECT unit_id,release_id,release_digest,basis FROM catalog_metadata_generation_audio WHERE work_id=?1 AND generation=?2",
            &[&work, &generation],
        )
        .await?;
    rows.iter()
        .map(|row| {
            let release = AudioRef::new(
                &get::<String>(row, "release_id")?,
                &get::<String>(row, "release_digest")?,
            )
            .map_err(|_| StoreError::CorruptRevision)?;
            let basis = AudioBasis::parse(&get::<String>(row, "basis")?)
                .map_err(|_| StoreError::CorruptRevision)?;
            Ok((
                get(row, "unit_id")?,
                AudioAssociation::restore(release, basis),
            ))
        })
        .collect()
}

/// The pointer as the domain sees it; a missing pointer is the only `NeverPublished`.
pub(crate) async fn state(tx: &Tx<'_>, work: &str) -> Result<PublicationState, StoreError> {
    let Some(row) = tx
        .query_opt(
            "SELECT version,lifecycle,generation,first_visible_at_ms FROM catalog_metadata_publication_pointers WHERE work_id=?1",
            &[&work],
        )
        .await?
    else {
        return Ok(PublicationState::NeverPublished);
    };
    settled(
        tx,
        work,
        positive(&row, "version")?,
        &get::<String>(&row, "lifecycle")?,
        positive(&row, "generation")?,
        get(&row, "first_visible_at_ms")?,
    )
    .await
    .map(PublicationState::Published)
}

/// The pointer exactly as one event settled it, for an operation replay.
pub(crate) async fn at_version(
    tx: &Tx<'_>,
    work: &str,
    version: u64,
) -> Result<Published, StoreError> {
    let row = tx
        .query_one(
            "SELECT e.lifecycle,e.generation,p.first_visible_at_ms FROM catalog_metadata_publication_events e JOIN catalog_metadata_publication_pointers p ON p.work_id=e.work_id WHERE e.work_id=?1 AND e.version=?2",
            &[&work, &checked_number(version)?],
        )
        .await?;
    settled(
        tx,
        work,
        version,
        &get::<String>(&row, "lifecycle")?,
        positive(&row, "generation")?,
        get(&row, "first_visible_at_ms")?,
    )
    .await
}

async fn settled(
    tx: &Tx<'_>,
    work: &str,
    version: u64,
    lifecycle: &str,
    generation: u64,
    first_visible_at: i64,
) -> Result<Published, StoreError> {
    let number = GenerationNumber::new(generation).map_err(|_| StoreError::CorruptRevision)?;
    let generation = self::generation(tx, work, number)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    Published::restore(
        PublicationVersion::new(version).map_err(|_| StoreError::CorruptRevision)?,
        Live::parse(lifecycle).map_err(|_| StoreError::CorruptRevision)?,
        generation,
        first_visible_at,
    )
    .map_err(|_| StoreError::CorruptRevision)
}

pub(crate) async fn recorded_at(
    tx: &Tx<'_>,
    work: &str,
    number: GenerationNumber,
) -> Result<i64, StoreError> {
    let row = tx
        .query_one(
            "SELECT recorded_at_ms FROM catalog_metadata_generations WHERE work_id=?1 AND generation=?2",
            &[&work, &checked_number(number.get())?],
        )
        .await?;
    get(&row, "recorded_at_ms")
}

pub(crate) async fn work_revision_digest(
    tx: &Tx<'_>,
    work: &str,
    revision: Revision,
) -> Result<Option<String>, StoreError> {
    tx.query_opt(
        "SELECT snapshot_digest FROM catalog_work_revisions WHERE work_id=?1 AND revision=?2",
        &[&work, &checked_number(revision.get())?],
    )
    .await?
    .map(|row| get(&row, "snapshot_digest"))
    .transpose()
}

pub(crate) async fn unit_revision_digest(
    tx: &Tx<'_>,
    unit: &str,
    revision: Revision,
) -> Result<Option<String>, StoreError> {
    tx.query_opt(
        "SELECT snapshot_digest FROM catalog_unit_revisions WHERE unit_id=?1 AND revision=?2",
        &[&unit, &checked_number(revision.get())?],
    )
    .await?
    .map(|row| get(&row, "snapshot_digest"))
    .transpose()
}

/// The Work snapshot a pin names, checked against both its stored and its pinned digest.
pub(crate) async fn pinned_work(tx: &Tx<'_>, work: &str, pin: &Pin) -> Result<Work, StoreError> {
    let row = tx
        .query_one(
            "SELECT snapshot,snapshot_digest FROM catalog_work_revisions WHERE work_id=?1 AND revision=?2",
            &[&work, &checked_number(pin.revision().get())?],
        )
        .await?;
    let bytes: Vec<u8> = get(&row, "snapshot")?;
    if get::<String>(&row, "snapshot_digest")? != pin.digest() || digest(&bytes) != pin.digest() {
        return Err(StoreError::CorruptRevision);
    }
    let decoded = decode_work(&bytes)?;
    if decoded.id().as_str() != work || decoded.revision() != pin.revision().get() {
        return Err(StoreError::CorruptRevision);
    }
    Ok(decoded)
}

/// Write a new generation and its entries. The caller then writes the sealing event.
pub(crate) async fn insert_generation(
    tx: &Tx<'_>,
    pinned: &Work,
    generation: &Generation,
    search_text: &str,
    now: i64,
) -> Result<(), StoreError> {
    let work = pinned.id().as_str();
    let number = checked_number(generation.number().get())?;
    let count =
        i64::try_from(generation.entries().len()).map_err(|_| StoreError::InvalidRequest)?;
    let language = pinned.details().language.to_ascii_lowercase();
    tx.execute(
        "INSERT INTO catalog_metadata_generations(work_id,generation,owner_id,kind,work_revision,work_digest,entry_count,language,search_text,recorded_by,recorded_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?3,?10)",
        &[
            &work,
            &number,
            &pinned.owner_id().as_str(),
            &pinned.kind().as_str(),
            &checked_number(generation.work().revision().get())?,
            &generation.work().digest(),
            &count,
            &language,
            &search_text,
            &now,
        ],
    )
    .await?;
    for (index, entry) in generation.entries().iter().enumerate() {
        let position = i64::try_from(index + 1).map_err(|_| StoreError::InvalidRequest)?;
        tx.execute(
            "INSERT INTO catalog_metadata_generation_entries(work_id,generation,position,unit_id,unit_revision,unit_digest) VALUES(?1,?2,?3,?4,?5,?6)",
            &[
                &work,
                &number,
                &position,
                &entry.unit().as_str(),
                &checked_number(entry.pin().revision().get())?,
                &entry.pin().digest(),
            ],
        )
        .await?;
        if let Some(audio) = entry.audio() {
            tx.execute(
                "INSERT INTO catalog_metadata_generation_audio(work_id,generation,unit_id,release_id,release_digest,basis) VALUES(?1,?2,?3,?4,?5,?6)",
                &[
                    &work,
                    &number,
                    &entry.unit().as_str(),
                    &audio.release().release(),
                    &audio.release().digest(),
                    &audio.basis().as_str(),
                ],
            )
            .await?;
        }
    }
    Ok(())
}

/// One settled version: the event (also the receipt), then the pointer that names it.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn settle(
    tx: &Tx<'_>,
    work: &str,
    owner: &str,
    action: &str,
    published: &Published,
    operation: &str,
    fingerprint: &str,
    now: i64,
) -> Result<(), StoreError> {
    let version = checked_number(published.version().get())?;
    let generation = checked_number(published.generation().number().get())?;
    let lifecycle = published.lifecycle().as_str();
    tx.execute(
        "INSERT INTO catalog_metadata_publication_events(work_id,version,owner_id,action,lifecycle,generation,operation_id,fingerprint,recorded_by,recorded_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?3,?9)",
        &[&work, &version, &owner, &action, &lifecycle, &generation, &operation, &fingerprint, &now],
    )
    .await?;
    let changed = if published.version().get() == 1 {
        tx.execute(
            "INSERT INTO catalog_metadata_publication_pointers(work_id,owner_id,version,lifecycle,generation,first_visible_at_ms) VALUES(?1,?2,?3,?4,?5,?6)",
            &[&work, &owner, &version, &lifecycle, &generation, &published.first_visible_at()],
        )
        .await?
    } else {
        tx.execute(
            "UPDATE catalog_metadata_publication_pointers SET version=?2,lifecycle=?3,generation=?4 WHERE work_id=?1 AND version=?2-1",
            &[&work, &version, &lifecycle, &generation],
        )
        .await?
    };
    if changed != 1 {
        return Err(StoreError::CorruptRevision);
    }
    Ok(())
}
