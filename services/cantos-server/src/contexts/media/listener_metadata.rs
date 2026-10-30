//! The single anonymous Discovery representation: exact published generations, never heads or samples.
use super::{
    domain::listening::DiscoveryQuery,
    generation_audio::playable_releases,
    listener_application::{clock, id, ListenerError, ListenerService},
    listener_cursor::Keyset,
    listener_projection::{unit_kind, work_kind},
    turso::cover::{pinned_cover_facts, PinnedCoverFacts, PinnedWork},
};
use crate::{
    contexts::catalog::{
        self,
        domain::{metadata_publication::GenerationNumber, WorkId, WorkKind},
    },
    contract::theatre::discovery as dto,
    platform::store::Tx,
};

fn summary(
    service: &ListenerService,
    row: &catalog::PublicWork,
    facts: Option<&PinnedCoverFacts>,
    now: i64,
) -> dto::MetadataWorkSummary {
    let work = &row.work;
    dto::MetadataWorkSummary {
        work_id: work.id().as_str().into(),
        kind: work_kind(work.kind()),
        title: work.details().title.clone(),
        description: work.details().synopsis.clone(),
        language: work.details().language.clone(),
        first_visible_at_ms: row.first_visible_at,
        unit_count: u64::from(row.unit_count),
        cover: service.shown_generation_cover(work, facts, now),
    }
}

async fn covers(
    service: &ListenerService,
    tx: &Tx<'_>,
    rows: &[catalog::PublicWork],
) -> Result<std::collections::BTreeMap<String, PinnedCoverFacts>, ListenerError> {
    if service.covers.is_none() {
        return Ok(Default::default());
    }
    let pins: Vec<_> = rows
        .iter()
        .map(|row| PinnedWork {
            work_id: row.work.id().as_str().into(),
            owner_id: row.work.owner_id().as_str().into(),
            revision: row.work.revision(),
        })
        .collect();
    Ok(pinned_cover_facts(tx, &pins).await?)
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
            .map(|token| self.metadata_cursors.read(token, &query, clock()?))
            .transpose()?;
        let after = match after {
            None => None,
            Some(Keyset::Works { time, id }) => Some((
                time,
                WorkId::parse(&id).map_err(|_| ListenerError::InvalidCursor)?,
            )),
            _ => return Err(ListenerError::InvalidCursor),
        };
        let kind = kind
            .map(WorkKind::parse)
            .transpose()
            .map_err(|_| ListenerError::InvalidRequest)?;
        let store = self
            .metadata_store
            .as_ref()
            .ok_or(ListenerError::Unavailable)?;
        let connection = store.connection().await?;
        let tx = connection.read().await?;
        let rows = catalog::public_works(
            &tx,
            catalog::PublicWorksQuery {
                search: query.search(),
                kind,
                language: query.language(),
                limit: limit as u8,
                after: after.as_ref().map(|(at, id)| (*at, id)),
            },
        )
        .await?;
        let cover_facts = covers(self, &tx, &rows).await?;
        let now = store
            .database()
            .now()
            .map_err(|_| ListenerError::Unavailable)?
            .get()
            / 1000;
        let items = rows
            .iter()
            .map(|row| summary(self, row, cover_facts.get(row.work.id().as_str()), now))
            .collect();
        // Probe in this same snapshot; a full terminal page must not advertise a phantom continuation.
        let next_cursor =
            if let Some(last) = rows.last().filter(|_| rows.len() == usize::from(limit)) {
                let more = catalog::public_works(
                    &tx,
                    catalog::PublicWorksQuery {
                        search: query.search(),
                        kind,
                        language: query.language(),
                        limit: 1,
                        after: Some((last.first_visible_at, last.work.id())),
                    },
                )
                .await?;
                if more.is_empty() {
                    None
                } else {
                    Some(self.metadata_cursors.issue(
                        &query,
                        Keyset::Works {
                            time: last.first_visible_at,
                            id: last.work.id().as_str().into(),
                        },
                        clock()?,
                    )?)
                }
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
        let id = WorkId::parse(&id(raw)?).map_err(|_| ListenerError::InvalidRequest)?;
        let store = self
            .metadata_store
            .as_ref()
            .ok_or(ListenerError::Unavailable)?;
        let connection = store.connection().await?;
        let tx = connection.read().await?;
        let work = catalog::public_work(&tx, &id)
            .await?
            .ok_or(ListenerError::NotFound)?;
        let cover_facts = covers(self, &tx, std::slice::from_ref(&work)).await?;
        let now = store
            .database()
            .now()
            .map_err(|_| ListenerError::Unavailable)?
            .get()
            / 1000;
        let result = summary(self, &work, cover_facts.get(work.work.id().as_str()), now);
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
            .map(|token| self.metadata_cursors.read(token, &query, clock()?))
            .transpose()?;
        // This vault belongs solely to generations; `id` is the generation number, never a Unit head.
        let walk = match after {
            None => None,
            Some(Keyset::Units {
                work: bound,
                position,
                id,
            }) if bound == work => Some((
                GenerationNumber::new(id.parse().map_err(|_| ListenerError::InvalidCursor)?)
                    .map_err(|_| ListenerError::InvalidCursor)?,
                position,
            )),
            _ => return Err(ListenerError::InvalidCursor),
        };
        let work_id = WorkId::parse(&work).map_err(|_| ListenerError::InvalidRequest)?;
        let store = self
            .metadata_store
            .as_ref()
            .ok_or(ListenerError::Unavailable)?;
        let connection = store.connection().await?;
        let tx = connection.read().await?;
        let page = catalog::public_units(&tx, &work_id, walk, limit as u8)
            .await
            .map_err(|error| match error {
                catalog::PublicReadError::Storage(error) => ListenerError::from(error),
                catalog::PublicReadError::StaleGeneration => ListenerError::InvalidCursor,
            })?
            .ok_or(ListenerError::NotFound)?;
        let asked: Vec<_> = page
            .items
            .iter()
            .map(|row| (row.unit.id(), row.audio.as_ref()))
            .collect();
        let releases = playable_releases(&tx, &asked).await?;
        let now = store
            .database()
            .now()
            .map_err(|_| ListenerError::Unavailable)?
            .get()
            / 1000;
        let items = page
            .items
            .iter()
            .zip(releases)
            .map(|(row, audio)| {
                let unit = &row.unit;
                dto::MetadataUnitSummary {
                    unit_id: unit.id().as_str().into(),
                    kind: unit_kind(unit.kind()),
                    position: row.position,
                    title: unit.details().title.clone(),
                    description: unit.details().description.clone(),
                    ordinal: u64::from(row.ordinal),
                    playback: audio
                        .filter(|release| {
                            let d = release.release().document();
                            super::domain::listening::access_window_valid(
                                d.published_at,
                                d.manifest.distribution_clearance.request.valid_until,
                                now,
                            )
                        })
                        .map(|release| {
                            let d = release.release().document();
                            dto::UnitPlayback {
                                publication_id: d.id.clone(),
                                published_at_ms: d.published_at,
                                duration_ms: d.manifest.media.duration_ms,
                            }
                        }),
                }
            })
            .collect();
        let next_cursor = page
            .next_after
            .map(|position| {
                self.metadata_cursors.issue(
                    &query,
                    Keyset::Units {
                        work: work.clone(),
                        position,
                        id: page.generation.get().to_string(),
                    },
                    clock()?,
                )
            })
            .transpose()?;
        tx.commit().await?;
        Ok(dto::MetadataUnitPage {
            work_id: work,
            items,
            next_cursor,
        })
    }
}
