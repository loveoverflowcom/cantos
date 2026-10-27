//! Opt-in union of release-gated Discovery and the separately disclosed sample metadata.
//! Every public fact is resolved inside one pinned database read; no Studio session is reused.
use super::{
    domain::listening::{after_unit, after_work, DiscoverableWork, DiscoveryQuery},
    listener_application::{clock, id, ListenerError, ListenerService},
    listener_cursor::Keyset,
    listener_projection::{presented_unit_summary, presented_work_summary, unit_kind, work_kind},
    turso::listener::{self, ReadItem},
};
use crate::{
    contexts::catalog::{
        domain::metadata_disclosure::approved_sample_ids, sample_disclosed_work,
        SampleDisclosedWork,
    },
    contract::theatre::discovery as dto,
    platform::store::{Store, Tx},
};
use std::collections::BTreeSet;

struct Sample {
    disclosure: SampleDisclosedWork,
    representative: Option<ReadItem>,
    units: Vec<ReadItem>,
}

async fn sample(
    tx: &Tx<'_>,
    id: &str,
    now: i64,
    covers: bool,
) -> Result<Option<Sample>, ListenerError> {
    let Some(disclosure) = sample_disclosed_work(tx, id).await? else {
        return Ok(None);
    };
    if disclosure.first_at > now {
        return Err(ListenerError::Corrupt);
    }
    let mut representative = listener::presented_work(tx, id, now).await?.0;
    if covers {
        representative = listener::with_covers(tx, (representative, None)).await?.0;
    }
    let ids: Vec<_> = disclosure
        .units
        .iter()
        .map(|u| u.id().as_str().to_owned())
        .collect();
    let mut units = Vec::new();
    for unit in ids {
        if let Some(row) = listener::presented_unit(tx, id, &unit, now).await? {
            units.push(row);
        }
    }
    Ok(Some(Sample {
        disclosure,
        representative: representative.into_iter().next(),
        units,
    }))
}

fn sample_summary(
    service: &ListenerService,
    s: &Sample,
    now: i64,
) -> Result<dto::MetadataWorkSummary, ListenerError> {
    let eligible = s
        .representative
        .as_ref()
        .filter(|r| r.publication.valid_at(now));
    let unit_releases: BTreeSet<_> = s
        .units
        .iter()
        .filter(|r| r.publication.valid_at(now))
        .map(|r| r.publication.unit().id().as_str())
        .collect();
    let extra = s
        .disclosure
        .units
        .iter()
        .filter(|u| !unit_releases.contains(u.id().as_str()))
        .count() as u64;
    if let Some(row) = eligible {
        let stats = row.presentation.ok_or(ListenerError::Corrupt)?;
        let summary = presented_work_summary(&row.publication, row.first_at, stats.unit_count);
        Ok(dto::MetadataWorkSummary {
            work_id: summary.work_id,
            kind: summary.kind,
            title: summary.title,
            description: summary.description,
            language: summary.language,
            first_visible_at_ms: row.first_at.min(s.disclosure.first_at),
            unit_count: safe_count(summary.unit_count.checked_add(extra))?,
            cover: service.shown(row, now),
        })
    } else {
        let w = &s.disclosure.work;
        Ok(dto::MetadataWorkSummary {
            work_id: w.id().as_str().into(),
            kind: work_kind(w.kind()),
            title: w.details().title.clone(),
            description: w.details().synopsis.clone(),
            language: w.details().language.clone(),
            first_visible_at_ms: s.disclosure.first_at,
            unit_count: s.disclosure.units.len() as u64,
            cover: None,
        })
    }
}

fn safe_count(value: Option<u64>) -> Result<u64, ListenerError> {
    value
        .filter(|v| *v <= 9_007_199_254_740_991)
        .ok_or(ListenerError::Corrupt)
}

fn released_summary(
    service: &ListenerService,
    row: &ReadItem,
    now: i64,
) -> Result<dto::MetadataWorkSummary, ListenerError> {
    let stats = row.presentation.ok_or(ListenerError::Corrupt)?;
    let w = presented_work_summary(&row.publication, row.first_at, stats.unit_count);
    Ok(dto::MetadataWorkSummary {
        work_id: w.work_id,
        kind: w.kind,
        title: w.title,
        description: w.description,
        language: w.language,
        first_visible_at_ms: w.first_published_at_ms,
        unit_count: w.unit_count,
        cover: service.shown(row, now),
    })
}

fn matches(
    query: &DiscoveryQuery,
    summary: &dto::MetadataWorkSummary,
) -> Result<bool, ListenerError> {
    use crate::contexts::catalog::domain::WorkKind;
    let kind = match summary.kind {
        dto::WorkKind::Novel => WorkKind::Novel,
        dto::WorkKind::Story => WorkKind::Story,
        dto::WorkKind::Podcast => WorkKind::Podcast,
    };
    Ok(query.matches(&DiscoverableWork {
        kind,
        language: &summary.language,
        title: &summary.title,
        description: &summary.description,
    }))
}

fn after_work_key(
    row: &dto::MetadataWorkSummary,
    after: Option<&Keyset>,
) -> Result<bool, ListenerError> {
    match after {
        None => Ok(true),
        Some(Keyset::Works { time, id }) => Ok(after_work(
            row.first_visible_at_ms,
            uuid::Uuid::parse_str(&row.work_id)
                .map_err(|_| ListenerError::Corrupt)?
                .as_u128(),
            *time,
            uuid::Uuid::parse_str(id)
                .map_err(|_| ListenerError::InvalidCursor)?
                .as_u128(),
        )),
        _ => Err(ListenerError::InvalidCursor),
    }
}
fn after_unit_key(
    row: &dto::MetadataUnitSummary,
    after: Option<&Keyset>,
    work: &str,
) -> Result<bool, ListenerError> {
    match after {
        None => Ok(true),
        Some(Keyset::Units {
            work: w,
            position,
            id,
        }) if w == work => Ok(after_unit(
            row.position,
            uuid::Uuid::parse_str(&row.unit_id)
                .map_err(|_| ListenerError::Corrupt)?
                .as_u128(),
            *position,
            uuid::Uuid::parse_str(id)
                .map_err(|_| ListenerError::InvalidCursor)?
                .as_u128(),
        )),
        _ => Err(ListenerError::InvalidCursor),
    }
}
fn read_clock(store: &Store, initial: i64) -> Result<i64, ListenerError> {
    Ok((store
        .database()
        .now()
        .map_err(|_| ListenerError::Unavailable)?
        .get()
        / 1000)
        .max(initial))
}

impl ListenerService {
    pub async fn metadata_works(
        &self,
        search: &str,
        kind: Option<&str>,
        language: Option<&str>,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::MetadataWorkPage, ListenerError> {
        let query = DiscoveryQuery::new(search, kind, language, limit)?;
        let after = cursor
            .map(|v| self.metadata_cursors.read(v, &query, clock()?))
            .transpose()?;
        if after
            .as_ref()
            .is_some_and(|v| !matches!(v, Keyset::Works { .. }))
        {
            return Err(ListenerError::InvalidCursor);
        }
        let store = self
            .metadata_store
            .as_ref()
            .ok_or(ListenerError::Unavailable)?;
        let connection = store.connection().await?;
        let tx = connection.read().await?;
        let now = listener::unit_millis(&tx);
        let mut samples = Vec::new();
        for (id, _) in approved_sample_ids() {
            if let Some(value) = sample(&tx, id, now, self.covers.is_some()).await? {
                samples.push(value);
            }
        }
        let excluded: BTreeSet<_> = samples
            .iter()
            .map(|v| v.disclosure.work.id().as_str().to_owned())
            .collect();
        let mut released = Vec::new();
        let mut scan = after.clone();
        loop {
            let (rows, next) = listener::presented_works(&tx, &query, scan.as_ref(), now).await?;
            released.extend(
                rows.into_iter()
                    .filter(|v| !excluded.contains(v.publication.work().id().as_str())),
            );
            if released.len() > query.limit() || next.is_none() {
                break;
            }
            scan = next;
        }
        released.truncate(query.limit() + 1);
        if self.covers.is_some() {
            released = listener::with_covers(&tx, (released, None)).await?.0;
        }
        let now = read_clock(store, now)?;
        let mut items = Vec::new();
        for s in &samples {
            let value = sample_summary(self, s, now)?;
            if matches(&query, &value)? && after_work_key(&value, after.as_ref())? {
                items.push(value);
            }
        }
        for row in released.iter().filter(|r| r.publication.valid_at(now)) {
            items.push(released_summary(self, row, now)?);
        }
        items.sort_by(|a, b| {
            b.first_visible_at_ms
                .cmp(&a.first_visible_at_ms)
                .then(a.work_id.cmp(&b.work_id))
        });
        let more = items.len() > query.limit();
        items.truncate(query.limit());
        let next_cursor = if more {
            let v = items.last().ok_or(ListenerError::Corrupt)?;
            Some(self.metadata_cursors.issue(
                &query,
                Keyset::Works {
                    time: v.first_visible_at_ms,
                    id: v.work_id.clone(),
                },
                clock()?,
            )?)
        } else {
            None
        };
        tx.commit().await?;
        Ok(dto::MetadataWorkPage { items, next_cursor })
    }

    pub async fn metadata_work(
        &self,
        raw: &str,
    ) -> Result<dto::MetadataWorkSummary, ListenerError> {
        let work = id(raw)?;
        let store = self
            .metadata_store
            .as_ref()
            .ok_or(ListenerError::Unavailable)?;
        let connection = store.connection().await?;
        let tx = connection.read().await?;
        let now = listener::unit_millis(&tx);
        let sample = sample(&tx, &work, now, self.covers.is_some()).await?;
        let mut released = if sample.is_none() {
            listener::presented_work(&tx, &work, now).await?.0
        } else {
            vec![]
        };
        if self.covers.is_some() {
            released = listener::with_covers(&tx, (released, None)).await?.0;
        }
        let now = read_clock(store, now)?;
        let result = match sample {
            Some(ref s) => sample_summary(self, s, now)?,
            None => {
                let row = released
                    .first()
                    .filter(|r| r.publication.valid_at(now))
                    .ok_or(ListenerError::NotFound)?;
                released_summary(self, row, now)?
            }
        };
        tx.commit().await?;
        Ok(result)
    }

    pub async fn metadata_units(
        &self,
        raw: &str,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::MetadataUnitPage, ListenerError> {
        let work = id(raw)?;
        let query = DiscoveryQuery::new("", None, None, limit)?;
        let after = cursor
            .map(|v| self.metadata_cursors.read(v, &query, clock()?))
            .transpose()?;
        if after
            .as_ref()
            .is_some_and(|v| !matches!(v, Keyset::Units { work: w, .. } if w == &work))
        {
            return Err(ListenerError::InvalidCursor);
        }
        let store = self
            .metadata_store
            .as_ref()
            .ok_or(ListenerError::Unavailable)?;
        let connection = store.connection().await?;
        let tx = connection.read().await?;
        let now = listener::unit_millis(&tx);
        let sample = sample(&tx, &work, now, false).await?;
        if sample.is_none()
            && listener::presented_work(&tx, &work, now)
                .await?
                .0
                .is_empty()
        {
            return Err(ListenerError::NotFound);
        }
        let excluded: BTreeSet<_> = sample
            .as_ref()
            .into_iter()
            .flat_map(|v| &v.disclosure.units)
            .map(|v| v.id().as_str().to_owned())
            .collect();
        let mut released = Vec::new();
        let mut scan = after.clone();
        loop {
            let (rows, next) =
                listener::presented_units(&tx, &work, scan.as_ref(), query.limit(), now).await?;
            released.extend(
                rows.into_iter()
                    .filter(|v| !excluded.contains(v.publication.unit().id().as_str())),
            );
            if released.len() > query.limit() || next.is_none() {
                break;
            }
            scan = next;
        }
        let final_now = read_clock(store, now)?;
        let extra: Vec<_> = sample
            .as_ref()
            .into_iter()
            .flat_map(|s| &s.disclosure.units)
            .filter(|unit| {
                !sample.as_ref().is_some_and(|s| {
                    s.units.iter().any(|r| {
                        r.publication.unit().id() == unit.id() && r.publication.valid_at(final_now)
                    })
                })
            })
            .collect();
        let extra_before = |position: u32, id: &str| -> u64 {
            extra
                .iter()
                .filter(|u| (u.position(), u.id().as_str()) < (position, id))
                .count() as u64
        };
        let mut items = Vec::new();
        if let Some(sample) = &sample {
            for unit in &sample.disclosure.units {
                let row = sample.units.iter().find(|r| {
                    r.publication.unit().id() == unit.id() && r.publication.valid_at(final_now)
                });
                let value = if let Some(row) = row {
                    let stats = row.presentation.ok_or(ListenerError::Corrupt)?;
                    let mut value = released_unit(row, stats.ordinal)?;
                    value.ordinal = safe_count(
                        value
                            .ordinal
                            .checked_add(extra_before(value.position, &value.unit_id)),
                    )?;
                    value
                } else {
                    let before = listener::released_units_before(
                        &tx,
                        &work,
                        unit.position(),
                        unit.id().as_str(),
                        final_now,
                    )
                    .await?;
                    dto::MetadataUnitSummary {
                        unit_id: unit.id().as_str().into(),
                        kind: unit_kind(unit.kind()),
                        position: unit.position(),
                        title: unit.details().title.clone(),
                        description: unit.details().description.clone(),
                        ordinal: safe_count(
                            before
                                .checked_add(extra_before(unit.position(), unit.id().as_str()))
                                .and_then(|v| v.checked_add(1)),
                        )?,
                        playback: None,
                    }
                };
                if after_unit_key(&value, after.as_ref(), &work)? {
                    items.push(value);
                }
            }
        }
        for row in released
            .iter()
            .filter(|r| r.publication.valid_at(final_now))
        {
            let stats = row.presentation.ok_or(ListenerError::Corrupt)?;
            let mut value = released_unit(row, stats.ordinal)?;
            value.ordinal = safe_count(
                value
                    .ordinal
                    .checked_add(extra_before(value.position, &value.unit_id)),
            )?;
            items.push(value);
        }
        items.sort_by(|a, b| a.position.cmp(&b.position).then(a.unit_id.cmp(&b.unit_id)));
        let more = items.len() > query.limit();
        items.truncate(query.limit());
        let next_cursor = if more {
            let v = items.last().ok_or(ListenerError::Corrupt)?;
            Some(self.metadata_cursors.issue(
                &query,
                Keyset::Units {
                    work: work.clone(),
                    position: v.position,
                    id: v.unit_id.clone(),
                },
                clock()?,
            )?)
        } else {
            None
        };
        tx.commit().await?;
        Ok(dto::MetadataUnitPage {
            work_id: work,
            items,
            next_cursor,
        })
    }
}

fn released_unit(row: &ReadItem, ordinal: u64) -> Result<dto::MetadataUnitSummary, ListenerError> {
    let u = presented_unit_summary(&row.publication, row.position, ordinal);
    Ok(dto::MetadataUnitSummary {
        unit_id: u.unit_id,
        kind: u.kind,
        position: u.position,
        title: u.title,
        description: u.description,
        ordinal: u.ordinal,
        playback: Some(dto::UnitPlayback {
            publication_id: u.publication_id,
            published_at_ms: u.published_at_ms,
            duration_ms: u.duration_ms,
        }),
    })
}
