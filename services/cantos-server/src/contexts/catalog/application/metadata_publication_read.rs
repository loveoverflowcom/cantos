//! The public metadata projection of active generations, for the only v1 audience (PUBLIC,
//! D05a). The caller owns one read transaction, so list, count, detail and Units share one
//! snapshot; eligibility filters before count, ordinal and pagination. No draft head, Catalog
//! position or `visibility` is read. S05 composes this into the single Discovery representation together with every current
//! client (ADR 0054 D07a).
use crate::contexts::catalog::{
    domain::{
        metadata_publication::{
            public_search_text, visible_entries, AudioAssociation, GenerationNumber,
        },
        ContentUnit, Work, WorkId, WorkKind,
    },
    turso::{metadata_publication, public_metadata as store},
};
use crate::platform::store::{StoreError, Tx};

/// One visible Work: its pinned metadata revision, the generation it was read from, the
/// immutable recency anchor and the number of currently visible Units.
#[derive(Clone, Debug)]
pub(crate) struct PublicWork {
    pub work: Work,
    pub generation: GenerationNumber,
    pub first_visible_at: i64,
    pub unit_count: u32,
}

/// One visible Unit: its pinned metadata revision, consecutive public rank, published position
/// (the keyset) and its exact audio association, if any. The association is a pin, not a
/// right: Media's live gate (`media::playable_releases`) decides playback at each read.
#[derive(Clone, Debug)]
pub(crate) struct PublicUnit {
    pub unit: ContentUnit,
    pub ordinal: u32,
    pub position: u32,
    pub audio: Option<AudioAssociation>,
}

#[derive(Clone, Debug)]
pub(crate) struct PublicUnitPage {
    pub generation: GenerationNumber,
    pub items: Vec<PublicUnit>,
    /// The published position to continue after, bound to `generation`.
    pub next_after: Option<u32>,
}

#[derive(Debug)]
pub(crate) enum PublicReadError {
    Storage(StoreError),
    /// The walk's generation is no longer active; restart from the first page.
    StaleGeneration,
}

impl From<StoreError> for PublicReadError {
    fn from(error: StoreError) -> Self {
        Self::Storage(error)
    }
}

pub(crate) struct PublicWorksQuery<'a> {
    pub search: &'a str,
    pub kind: Option<WorkKind>,
    pub language: Option<&'a str>,
    pub limit: u8,
    /// The last returned `(first_visible_at, work_id)`.
    pub after: Option<(i64, &'a WorkId)>,
}

fn public(row: store::PublicWorkRow) -> PublicWork {
    PublicWork {
        work: row.work,
        generation: row.generation,
        first_visible_at: row.first_visible_at,
        unit_count: row.unit_count,
    }
}

/// Newest first by the immutable anchor, then ascending Work ID.
pub(crate) async fn public_works(
    tx: &Tx<'_>,
    query: PublicWorksQuery<'_>,
) -> Result<Vec<PublicWork>, StoreError> {
    if !(1..=50).contains(&query.limit) || query.search.len() > 512 {
        return Err(StoreError::InvalidRequest);
    }
    let search = public_search_text(query.search);
    let language = query.language.map(str::to_ascii_lowercase);
    let rows = store::public_works(
        tx,
        query.after.map(|(at, id)| (at, id.as_str())),
        query.kind,
        language.as_deref(),
        &search,
        i64::from(query.limit),
    )
    .await?;
    Ok(rows.into_iter().map(public).collect())
}

/// A direct ID read applies exactly the list's gates; absent and hidden are indistinguishable.
pub(crate) async fn public_work(
    tx: &Tx<'_>,
    id: &WorkId,
) -> Result<Option<PublicWork>, StoreError> {
    Ok(store::public_work(tx, id.as_str()).await?.map(public))
}

/// Visible Units of a visible Work in published order. `walk` binds a continuation to the
/// generation it started on; a newer generation restarts the walk instead of mixing two.
pub(crate) async fn public_units(
    tx: &Tx<'_>,
    id: &WorkId,
    walk: Option<(GenerationNumber, u32)>,
    limit: u8,
) -> Result<Option<PublicUnitPage>, PublicReadError> {
    if !(1..=50).contains(&limit) {
        return Err(StoreError::InvalidRequest.into());
    }
    let Some(work) = store::public_work(tx, id.as_str()).await? else {
        return Ok(None);
    };
    if walk.is_some_and(|(generation, _)| generation != work.generation) {
        return Err(PublicReadError::StaleGeneration);
    }
    let stored = metadata_publication::generation(tx, id.as_str(), work.generation)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    let mut archived = store::archived_units(tx, id.as_str(), work.generation).await?;
    archived
        .extend(super::super::turso::viewer_access::units::public_hidden(tx, id.as_str()).await?);
    let after = walk.map_or(0, |(_, after)| after);
    let mut page: Vec<_> = visible_entries(&stored, |unit| archived.contains(unit.as_str()))
        .into_iter()
        .filter(|visible| visible.position > after)
        .take(usize::from(limit) + 1)
        .collect();
    let next_after = if page.len() > usize::from(limit) {
        page.pop();
        page.last().map(|visible| visible.position)
    } else {
        None
    };
    let pins: Vec<_> = page
        .iter()
        .map(|visible| (visible.entry.unit(), visible.entry.pin()))
        .collect();
    let units = store::pinned_units(tx, id.as_str(), work.work.kind(), &pins).await?;
    let items = page
        .iter()
        .zip(units)
        .map(|(visible, unit)| PublicUnit {
            unit,
            ordinal: visible.ordinal,
            position: visible.position,
            audio: visible.entry.audio().cloned(),
        })
        .collect();
    Ok(Some(PublicUnitPage {
        generation: work.generation,
        items,
        next_after,
    }))
}

/// One exact visible entry for byte/manifest admission. No head or unrelated-release fallback.
pub(crate) async fn public_unit(
    tx: &Tx<'_>,
    id: &WorkId,
    unit: &super::super::domain::ContentUnitId,
) -> Result<Option<(PublicWork, PublicUnit)>, StoreError> {
    let Some(work) = public_work(tx, id).await? else {
        return Ok(None);
    };
    let stored = metadata_publication::generation(tx, id.as_str(), work.generation)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    let mut archived = store::archived_units(tx, id.as_str(), work.generation).await?;
    archived
        .extend(super::super::turso::viewer_access::units::public_hidden(tx, id.as_str()).await?);
    let visible = visible_entries(&stored, |unit| archived.contains(unit.as_str()));
    let Some(entry) = visible.into_iter().find(|entry| entry.entry.unit() == unit) else {
        return Ok(None);
    };
    let mut pinned = store::pinned_units(
        tx,
        id.as_str(),
        work.work.kind(),
        &[(entry.entry.unit(), entry.entry.pin())],
    )
    .await?;
    let unit = pinned.pop().ok_or(StoreError::CorruptRevision)?;
    Ok(Some((
        work,
        PublicUnit {
            unit,
            ordinal: entry.ordinal,
            position: entry.position,
            audio: entry.entry.audio().cloned(),
        },
    )))
}
