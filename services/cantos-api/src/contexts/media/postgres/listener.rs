//! Authorized PostgreSQL listener reads. One bounded joined query, no per-item SQL/N+1.
use super::super::{
    domain::{
        lifecycle::{AudioRendition, AudioSubmission, ReleaseCandidate},
        listening::{normalize_search, DiscoveryQuery, ListenerFacts, ListenerPublication},
        publication::{AudioRelease, DistributionClearance, PublicationApproval},
    },
    listener_application::ListenerError,
    listener_cursor::Keyset,
};
use crate::{
    contexts::catalog::{
        listener_metadata, restore_content, ListenerMetadata, ListenerMetadataRequest,
        LISTENER_CURRENT_FILTER,
    },
    platform::postgres::{checked_number, StoreError, Tx, LISTENER_ACTOR_FILTER},
};
use serde::{de::DeserializeOwned, Serialize};
use tokio_postgres::Row;

pub(super) async fn index_release(tx: &Tx<'_>, release: &AudioRelease) -> Result<(), StoreError> {
    let d = release.document();
    let m = &d.manifest;
    let requests = [ListenerMetadataRequest {
        release: d.id.clone(),
        work: m.work_id.clone(),
        unit: m.unit_id.clone(),
        work_revision: checked_number(m.work_metadata_revision)?,
        unit_revision: checked_number(m.unit_metadata_revision)?,
        content: m.binding.content_revision_id.clone(),
    }];
    let metadata = listener_metadata(tx, &requests)
        .await?
        .into_iter()
        .next()
        .ok_or(StoreError::CorruptRevision)?;
    let w = metadata.work;
    let u = metadata.unit;
    if w.id().as_str() != m.work_id
        || u.id().as_str() != m.unit_id
        || u.work_id() != w.id()
        || w.owner_id().as_str() != d.published_by
        || w.details().title != m.work_title
        || u.details().title != m.unit_title
    {
        return Err(StoreError::CorruptRevision);
    }
    if w.details().visibility != crate::contexts::catalog::domain::Visibility::Public {
        return Ok(());
    }
    // First committed index writer wins. The anchor never moves on replacement or withdrawal.
    tx.execute(
        "INSERT INTO audio_listener_works VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
        &[&m.work_id, &d.id, &d.published_at],
    )
    .await?;
    let position = i32::try_from(u.position()).map_err(|_| StoreError::CorruptRevision)?;
    tx.execute(
        "INSERT INTO audio_listener_units VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
        &[&m.unit_id, &m.work_id, &position],
    )
    .await?;
    let text = normalize_search(&format!("{} {}", w.details().title, w.details().synopsis));
    tx.execute("INSERT INTO audio_listener_releases VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",&[&d.id,&m.work_id,&m.unit_id,&d.published_at,&w.kind().as_str(),&m.language.to_ascii_lowercase(),&text,&m.distribution_clearance.digest,&m.publication_approval.digest,&m.distribution_clearance.recorded_at,&m.distribution_clearance.request.valid_until,&checked_number(m.work_metadata_revision)?,&checked_number(m.unit_metadata_revision)?,&m.binding.content_revision_id]).await?;
    Ok(())
}
fn bytes(row: &Row, key: &str) -> Result<Vec<u8>, StoreError> {
    row.try_get(key).map_err(|_| StoreError::CorruptRevision)
}
fn raw<T: DeserializeOwned + Serialize>(row: &Row, key: &str, max: usize) -> Result<T, StoreError> {
    let b = bytes(row, key)?;
    if b.is_empty() || b.len() > max {
        return Err(StoreError::CorruptRevision);
    }
    let v: T = serde_json::from_slice(&b).map_err(|_| StoreError::CorruptRevision)?;
    if serde_json::to_vec(&v).map_err(|_| StoreError::CorruptRevision)? != b {
        return Err(StoreError::CorruptRevision);
    }
    Ok(v)
}
fn admit<T, E>(r: Result<T, E>) -> Result<T, StoreError> {
    r.map_err(|_| StoreError::CorruptRevision)
}
pub(in crate::contexts::media) struct ReadItem {
    pub publication: ListenerPublication,
    pub first_at: i64,
    pub position: u32,
}
fn decode(
    row: &Row,
    metadata: ListenerMetadata,
    owner_active: bool,
    now: i64,
) -> Result<Option<ReadItem>, ListenerError> {
    let s = admit(AudioSubmission::restore(raw(row, "submission", 32768)?))?;
    let r = admit(AudioRendition::restore(raw(row, "rendition", 32768)?, &s))?;
    let c = admit(ReleaseCandidate::restore(raw(row, "candidate", 32768)?, &r))?;
    let content = admit(restore_content(metadata.content))?;
    let release = admit(AudioRelease::restore(
        raw(row, "release", 262144)?,
        &c,
        &r,
        &content,
    ))?;
    let rights = admit(DistributionClearance::restore(
        raw(row, "clearance", 32768)?,
        &c,
    ))?;
    let approval = admit(PublicationApproval::restore(
        raw(row, "approval", 8192)?,
        &c,
    ))?;
    let sd = s.document();
    let rd = r.document();
    let cd = c.document();
    let ld = release.document();
    let md = &ld.manifest;
    let dc = rights.document();
    let pa = approval.document();
    let expected = [
        ("release_row_id", ld.id.as_str()),
        ("release_row_unit", md.unit_id.as_str()),
        ("release_row_owner", ld.published_by.as_str()),
        ("release_row_candidate", md.candidate_id.as_str()),
        ("release_row_rendition", md.rendition_id.as_str()),
        ("release_row_manifest", md.digest.as_str()),
        ("release_row_digest", ld.digest.as_str()),
        ("submission_row_id", sd.id.as_str()),
        ("submission_row_owner", sd.owner_id.as_str()),
        ("submission_row_unit", sd.binding.unit_id.as_str()),
        ("submission_row_brief", sd.binding.brief_id.as_str()),
        (
            "submission_row_brief_digest",
            sd.binding.brief_digest.as_str(),
        ),
        ("submission_row_operation", sd.operation_id.as_str()),
        ("submission_row_digest", sd.digest.as_str()),
        ("rendition_row_id", rd.id.as_str()),
        ("rendition_row_submission", rd.submission_id.as_str()),
        ("rendition_row_object", rd.verified_media.sha256.as_str()),
        ("rendition_row_digest", rd.digest.as_str()),
        ("candidate_row_id", cd.id.as_str()),
        ("candidate_row_rendition", cd.rendition_id.as_str()),
        ("candidate_row_owner", cd.created_by.as_str()),
        ("candidate_row_operation", cd.operation_id.as_str()),
        ("candidate_row_digest", cd.digest.as_str()),
        ("clearance_row_id", dc.id.as_str()),
        ("clearance_row_candidate", dc.candidate_id.as_str()),
        ("clearance_row_owner", dc.actor_id.as_str()),
        ("clearance_row_operation", dc.request.operation_id.as_str()),
        ("clearance_row_digest", dc.digest.as_str()),
        ("approval_row_id", pa.id.as_str()),
        ("approval_row_candidate", pa.candidate_id.as_str()),
        ("approval_row_owner", pa.actor_id.as_str()),
        ("approval_row_operation", pa.request.operation_id.as_str()),
        ("approval_row_digest", pa.digest.as_str()),
    ];
    for (key, value) in expected {
        if row
            .try_get::<_, String>(key)
            .map_err(|_| ListenerError::Corrupt)?
            != value
        {
            return Err(ListenerError::Corrupt);
        }
    }
    for (key, expected) in [
        ("clearance_row_sequence", dc.sequence),
        ("approval_row_sequence", pa.sequence),
    ] {
        if row
            .try_get::<_, i64>(key)
            .map_err(|_| ListenerError::Corrupt)?
            != i64::try_from(expected).map_err(|_| ListenerError::Corrupt)?
        {
            return Err(ListenerError::Corrupt);
        }
    }
    let w = metadata.work;
    let u = metadata.unit;
    let current_w = metadata.current_work;
    let current_u = metadata.current_unit;
    let get = |key| {
        row.try_get::<_, String>(key)
            .map_err(|_| StoreError::CorruptRevision)
    };
    let m = &release.document().manifest;
    if get("release_id")? != release.document().id
        || get("work_id")? != m.work_id
        || get("unit_id")? != m.unit_id
        || get("owner_id")? != release.document().published_by
        || get("kind")? != w.kind().as_str()
        || get("language")? != m.language.to_ascii_lowercase()
        || get("search_text")?
            != normalize_search(&format!("{} {}", w.details().title, w.details().synopsis))
        || get("rights_digest")? != m.distribution_clearance.digest
        || get("approval_digest")? != m.publication_approval.digest
        || content.id().as_str() != m.binding.content_revision_id
    {
        return Err(ListenerError::Corrupt);
    }
    for (key, expected) in [
        ("published_at", release.document().published_at),
        ("rights_start", m.distribution_clearance.recorded_at),
        ("rights_end", m.distribution_clearance.request.valid_until),
        (
            "work_revision",
            i64::try_from(m.work_metadata_revision).map_err(|_| ListenerError::Corrupt)?,
        ),
        (
            "unit_revision",
            i64::try_from(m.unit_metadata_revision).map_err(|_| ListenerError::Corrupt)?,
        ),
    ] {
        if row
            .try_get::<_, i64>(key)
            .map_err(|_| ListenerError::Corrupt)?
            != expected
        {
            return Err(ListenerError::Corrupt);
        }
    }
    let p = ListenerPublication::resolve(ListenerFacts {
        release: &release,
        pinned_work: &w,
        pinned_unit: &u,
        current_work: &current_w,
        current_unit: &current_u,
        rights: Some(&rights),
        approval: Some(&approval),
        active: true,
        owner_active,
        withdrawn: false,
        submission_revoked: false,
        now,
    })
    .map_err(|_| ListenerError::Corrupt)?;
    let first_at = row
        .try_get::<_, i64>("first_published_at")
        .map_err(|_| ListenerError::Corrupt)?;
    let position = u32::try_from(
        row.try_get::<_, i32>("first_position")
            .map_err(|_| ListenerError::Corrupt)?,
    )
    .map_err(|_| ListenerError::Corrupt)?;
    if first_at < 0 || first_at > now {
        return Err(ListenerError::Corrupt);
    }
    Ok(p.map(|publication| ReadItem {
        publication,
        first_at,
        position,
    }))
}
const FROM: &str = r#"FROM audio_listener_releases x
JOIN audio_listener_works anchor ON anchor.work_id=x.work_id
JOIN audio_listener_units ordering ON ordering.unit_id=x.unit_id AND ordering.work_id=x.work_id
JOIN audio_publication_pointers pointer ON pointer.unit_id=x.unit_id AND pointer.active_release_id=x.release_id
JOIN audio_releases release ON release.id=x.release_id AND release.unit_id=x.unit_id AND release.owner_id=pointer.owner_id
JOIN audio_release_candidates candidate ON candidate.id=release.candidate_id AND candidate.owner_id=release.owner_id
JOIN audio_renditions rendition ON rendition.id=release.rendition_id AND rendition.id=candidate.rendition_id
JOIN audio_submissions submission ON submission.id=rendition.submission_id AND submission.owner_id=release.owner_id AND submission.unit_id=x.unit_id
JOIN LATERAL (SELECT * FROM audio_distribution_clearances WHERE candidate_id=candidate.id AND owner_id=release.owner_id ORDER BY sequence DESC LIMIT 1) clearance ON clearance.digest=x.rights_digest
JOIN LATERAL (SELECT * FROM audio_publication_approvals WHERE candidate_id=candidate.id AND owner_id=release.owner_id ORDER BY sequence DESC LIMIT 1) approval ON approval.digest=x.approval_digest
WHERE {current_filter} AND {actor_filter}
AND x.rights_start<=$1 AND $1<x.rights_end AND x.published_at<=$1
AND NOT EXISTS(SELECT 1 FROM audio_release_withdrawals WHERE release_id=release.id)
AND NOT EXISTS(SELECT 1 FROM audio_submission_revocations WHERE submission_id=submission.id)
"#;
const COLUMNS:&str="x.*,release.owner_id,anchor.first_published_at,ordering.first_position,release.snapshot AS release,submission.snapshot AS submission,rendition.snapshot AS rendition,candidate.snapshot AS candidate,clearance.snapshot AS clearance,approval.snapshot AS approval,
release.id AS release_row_id,release.unit_id AS release_row_unit,release.owner_id AS release_row_owner,release.candidate_id AS release_row_candidate,release.rendition_id AS release_row_rendition,release.manifest_digest AS release_row_manifest,release.digest AS release_row_digest,
submission.id AS submission_row_id,submission.owner_id AS submission_row_owner,submission.unit_id AS submission_row_unit,submission.brief_id AS submission_row_brief,submission.brief_digest AS submission_row_brief_digest,submission.operation_id AS submission_row_operation,submission.digest AS submission_row_digest,
rendition.id AS rendition_row_id,rendition.submission_id AS rendition_row_submission,rendition.object_key AS rendition_row_object,rendition.digest AS rendition_row_digest,
candidate.id AS candidate_row_id,candidate.rendition_id AS candidate_row_rendition,candidate.owner_id AS candidate_row_owner,candidate.operation_id AS candidate_row_operation,candidate.digest AS candidate_row_digest,
clearance.id AS clearance_row_id,clearance.candidate_id AS clearance_row_candidate,clearance.owner_id AS clearance_row_owner,clearance.operation_id AS clearance_row_operation,clearance.sequence AS clearance_row_sequence,clearance.digest AS clearance_row_digest,
approval.id AS approval_row_id,approval.candidate_id AS approval_row_candidate,approval.owner_id AS approval_row_owner,approval.operation_id AS approval_row_operation,approval.sequence AS approval_row_sequence,approval.digest AS approval_row_digest";
async fn batch(
    tx: &Tx<'_>,
    rows: Vec<Row>,
    limit: usize,
    works: bool,
    now: i64,
) -> Result<(Vec<ReadItem>, Option<Keyset>), ListenerError> {
    let more = rows.len() > limit;
    let rows: Vec<_> = rows.into_iter().take(limit).collect();
    let mut next = None;
    if more {
        let r = rows.last().ok_or(ListenerError::Corrupt)?;
        next = Some(if works {
            Keyset::Works {
                time: r.get("first_published_at"),
                id: r.get("work_id"),
            }
        } else {
            Keyset::Units {
                work: r.get("work_id"),
                position: u32::try_from(r.get::<_, i32>("first_position"))
                    .map_err(|_| ListenerError::Corrupt)?,
                id: r.get("unit_id"),
            }
        });
    }
    let requests: Vec<_> = rows
        .iter()
        .map(|r| ListenerMetadataRequest {
            release: r.get("release_id"),
            work: r.get("work_id"),
            unit: r.get("unit_id"),
            work_revision: r.get("work_revision"),
            unit_revision: r.get("unit_revision"),
            content: r.get("content_revision_id"),
        })
        .collect();
    let mut metadata: std::collections::BTreeMap<_, _> = listener_metadata(tx, &requests)
        .await?
        .into_iter()
        .map(|m| (m.release.clone(), m))
        .collect();
    let owners: Vec<String> = rows.iter().map(|r| r.get("owner_id")).collect();
    let active = crate::platform::postgres::active_actors(tx, &owners).await?;
    let mut items = Vec::new();
    for r in rows {
        let m = metadata
            .remove(&r.get::<_, String>("release_id"))
            .ok_or(ListenerError::Corrupt)?;
        if let Some(item) = decode(&r, m, active.contains(&r.get::<_, String>("owner_id")), now)? {
            items.push(item);
        }
    }
    Ok((items, next))
}
pub(in crate::contexts::media) async fn works(
    tx: &Tx<'_>,
    q: &DiscoveryQuery,
    after: Option<&Keyset>,
    now: i64,
) -> Result<(Vec<ReadItem>, Option<Keyset>), ListenerError> {
    let (time, id) = match after {
        None => (None, None),
        Some(Keyset::Works { time, id }) => (Some(*time), Some(id.as_str())),
        Some(Keyset::Units { .. }) => return Err(ListenerError::InvalidCursor),
    };
    let from = FROM
        .replace("{current_filter}", LISTENER_CURRENT_FILTER)
        .replace("{actor_filter}", LISTENER_ACTOR_FILTER);
    let sql=format!("SELECT * FROM (SELECT DISTINCT ON (x.work_id COLLATE \"C\") {COLUMNS} {from} AND ($2::bigint IS NULL OR anchor.first_published_at<$2 OR (anchor.first_published_at=$2 AND x.work_id COLLATE \"C\">$3::text COLLATE \"C\")) ORDER BY x.work_id COLLATE \"C\",x.published_at DESC,x.release_id COLLATE \"C\" ASC) selected WHERE ($4::text IS NULL OR kind=$4) AND ($5::text IS NULL OR language=$5) AND position($6 in search_text)>0 ORDER BY first_published_at DESC,work_id COLLATE \"C\" ASC LIMIT $7");
    let limit = (q.limit() + 1) as i64;
    let rows = tx
        .query(
            &sql,
            &[
                &now,
                &time,
                &id,
                &q.kind(),
                &q.language(),
                &q.search(),
                &limit,
            ],
        )
        .await
        .map_err(|_| ListenerError::Unavailable)?;
    batch(tx, rows, q.limit(), true, now).await
}
pub(in crate::contexts::media) async fn work(
    tx: &Tx<'_>,
    id: &str,
    now: i64,
) -> Result<(Vec<ReadItem>, Option<Keyset>), ListenerError> {
    let from = FROM
        .replace("{current_filter}", LISTENER_CURRENT_FILTER)
        .replace("{actor_filter}", LISTENER_ACTOR_FILTER);
    let sql=format!("SELECT {COLUMNS} {from} AND x.work_id=$2 ORDER BY x.published_at DESC,x.release_id COLLATE \"C\" ASC LIMIT 1");
    let rows = tx
        .query(&sql, &[&now, &id])
        .await
        .map_err(|_| ListenerError::Unavailable)?;
    batch(tx, rows, 1, false, now).await
}
pub(in crate::contexts::media) async fn units(
    tx: &Tx<'_>,
    work: &str,
    after: Option<&Keyset>,
    limit: usize,
    now: i64,
) -> Result<(Vec<ReadItem>, Option<Keyset>), ListenerError> {
    let (position, id) = match after {
        None => (None, None),
        Some(Keyset::Units {
            work: w,
            position,
            id,
        }) if w == work => (Some(i64::from(*position)), Some(id.as_str())),
        Some(_) => return Err(ListenerError::InvalidCursor),
    };
    let from = FROM
        .replace("{current_filter}", LISTENER_CURRENT_FILTER)
        .replace("{actor_filter}", LISTENER_ACTOR_FILTER);
    let sql=format!("SELECT {COLUMNS} {from} AND x.work_id=$2 AND ($3::bigint IS NULL OR (ordering.first_position,x.unit_id COLLATE \"C\")>($3,$4::text COLLATE \"C\")) ORDER BY ordering.first_position,x.unit_id COLLATE \"C\" LIMIT $5");
    let rows = tx
        .query(&sql, &[&now, &work, &position, &id, &((limit + 1) as i64)])
        .await
        .map_err(|_| ListenerError::Unavailable)?;
    batch(tx, rows, limit, false, now).await
}
pub(in crate::contexts::media) async fn publication(
    tx: &Tx<'_>,
    id: &str,
    now: i64,
) -> Result<Option<ReadItem>, ListenerError> {
    let from = FROM
        .replace("{current_filter}", LISTENER_CURRENT_FILTER)
        .replace("{actor_filter}", LISTENER_ACTOR_FILTER);
    let sql = format!("SELECT {COLUMNS} {from} AND x.release_id=$2");
    let rows = tx
        .query(&sql, &[&now, &id])
        .await
        .map_err(|_| ListenerError::Unavailable)?;
    Ok(batch(tx, rows, 1, false, now).await?.0.into_iter().next())
}

use super::super::listener_application::{ListenerRead, ReadFuture, ReadRequest, ReadSnapshot};
use crate::platform::postgres::Store;
pub(in crate::contexts::media) struct PgRead(pub Store);
impl ListenerRead for PgRead {
    fn read(&self, request: ReadRequest) -> ReadFuture {
        let store = self.0.clone();
        Box::pin(async move {
            let mut c = store.connection().await?;
            let tx = c
                .build_transaction()
                .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(|_| ListenerError::Unavailable)?;
            let now = tx
                .query_one(
                    "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
                    &[],
                )
                .await
                .map_err(|_| ListenerError::Unavailable)?
                .get(0);
            let (mut items, next_key) = match request {
                ReadRequest::Work { work: id } => work(&tx, &id, now).await?,
                ReadRequest::Works { query, after } => {
                    works(&tx, &query, after.as_ref(), now).await?
                }
                ReadRequest::Units { work, after, limit } => {
                    units(&tx, &work, after.as_ref(), limit, now).await?
                }
                ReadRequest::Publication { id } => (
                    publication(&tx, &id, now).await?.into_iter().collect(),
                    None,
                ),
            };
            let now: i64 = tx
                .query_one(
                    "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
                    &[],
                )
                .await
                .map_err(|_| ListenerError::Unavailable)?
                .get(0);
            items.retain(|r| r.publication.valid_at(now));
            tx.commit().await.map_err(|_| ListenerError::Unavailable)?;
            Ok(ReadSnapshot {
                now,
                items,
                next_key,
            })
        })
    }
}
