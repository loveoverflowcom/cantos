//! Authorized Turso listener reads. One bounded joined query, no per-item SQL/N+1. Every read
//! decides current eligibility at one trusted instant over one pinned snapshot; times here are
//! the domain's Unix milliseconds, converted once from the transaction's microsecond stamp.
use super::super::{
    domain::{
        lifecycle::{AudioRendition, AudioSubmission, ReleaseCandidate},
        listening::{normalize_search, DiscoveryQuery, ListenerFacts, ListenerPublication},
        publication::{AudioRelease, DistributionClearance, PublicationApproval},
    },
    listener_application::ListenerError,
    listener_cursor::Keyset,
    listener_projection::{audio_presentation, publication_manifest, work_presentation},
    turso::cover::{pinned_cover_facts, PinnedCoverFacts, PinnedWork},
};
use crate::platform::store::Row;
use crate::{
    contexts::catalog::{
        listener_metadata, restore_content, ListenerMetadata, ListenerMetadataRequest,
        LISTENER_CURRENT_FILTER,
    },
    platform::store::{checked_number, StoreError, Tx, RELEASE_OWNER_ACTIVE_FILTER},
};
use serde::{de::DeserializeOwned, Serialize};

pub(crate) async fn index_release(tx: &Tx<'_>, release: &AudioRelease) -> Result<(), StoreError> {
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
    // First committed index writer wins. The anchor never moves on replacement or withdrawal.
    // A pre-checked insert instead of `ON CONFLICT DO NOTHING`: the index tables refuse an
    // insert over an existing key before conflict resolution (see `REPLACE`), so the check
    // is explicit and the first committed index writer still wins.
    tx.execute(
        "INSERT INTO audio_listener_works(work_id,first_release_id,first_published_at) SELECT ?1,?2,?3 WHERE NOT EXISTS (SELECT 1 FROM audio_listener_works WHERE work_id=?1)",
        &[&m.work_id, &d.id, &d.published_at],
    )
    .await?;
    let position = i32::try_from(u.position()).map_err(|_| StoreError::CorruptRevision)?;
    tx.execute(
        "INSERT INTO audio_listener_units(unit_id,work_id,first_position) SELECT ?1,?2,?3 WHERE NOT EXISTS (SELECT 1 FROM audio_listener_units WHERE unit_id=?1)",
        &[&m.unit_id, &m.work_id, &position],
    )
    .await?;
    let text = normalize_search(&format!("{} {}", w.details().title, w.details().synopsis));
    tx.execute("INSERT INTO audio_listener_releases(release_id,work_id,unit_id,published_at,kind,language,search_text,rights_digest,approval_digest,rights_start,rights_end,work_revision,unit_revision,content_revision_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",&[&d.id,&m.work_id,&m.unit_id,&d.published_at,&w.kind().as_str(),&m.language.to_ascii_lowercase(),&text,&m.distribution_clearance.digest,&m.publication_approval.digest,&m.distribution_clearance.recorded_at,&m.distribution_clearance.request.valid_until,&checked_number(m.work_metadata_revision)?,&checked_number(m.unit_metadata_revision)?,&m.binding.content_revision_id]).await?;
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
    pub presentation: Option<PresentationStats>,
    /// Pinned cover facts, read only for a covered request (`presentation=2` or the cover route).
    pub cover: Option<PinnedCoverFacts>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::contexts::media) struct PresentationStats {
    pub unit_count: u64,
    pub ordinal: u64,
}
impl PresentationStats {
    fn new(unit_count: i64, ordinal: i64) -> Result<Self, ListenerError> {
        const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
        if !(1..=MAX_SAFE_INTEGER).contains(&unit_count) || !(1..=unit_count).contains(&ordinal) {
            return Err(ListenerError::Corrupt);
        }
        Ok(Self {
            unit_count: u64::try_from(unit_count).map_err(|_| ListenerError::Corrupt)?,
            ordinal: u64::try_from(ordinal).map_err(|_| ListenerError::Corrupt)?,
        })
    }
}
fn decode(
    row: &Row,
    metadata: ListenerMetadata,
    owner_active: bool,
    now: i64,
) -> Result<Option<ReadItem>, ListenerError> {
    decode_for(
        row,
        metadata,
        owner_active,
        now,
        ListenerPublication::resolve,
    )
}
fn decode_for(
    row: &Row,
    metadata: ListenerMetadata,
    owner_active: bool,
    now: i64,
    resolve: for<'a> fn(
        ListenerFacts<'a>,
    ) -> Result<
        Option<ListenerPublication>,
        super::super::domain::listening::ListenerDomainError,
    >,
) -> Result<Option<ReadItem>, ListenerError> {
    let s = admit(AudioSubmission::restore(raw(row, "submission", 32768)?))?;
    let r = admit(AudioRendition::restore(raw(row, "rendition", 32768)?, &s))?;
    let c = admit(ReleaseCandidate::restore(raw(row, "candidate", 32768)?, &r))?;
    let content = admit(restore_content(metadata.content))?;
    let release = admit(AudioRelease::restore(
        raw(row, "release_doc", 262144)?,
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
    let p = resolve(ListenerFacts {
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
        presentation: None,
        cover: None,
    }))
}
/// The eligible relation. `?1` is the read's instant in milliseconds. PostgreSQL's `JOIN LATERAL`
/// (the latest clearance/approval of the candidate) is the row whose `sequence` is the candidate's
/// maximum, which `UNIQUE (candidate_id, sequence)` makes a single row.
const FROM: &str = r#"FROM audio_listener_releases x
JOIN audio_listener_works anchor ON anchor.work_id=x.work_id
JOIN audio_listener_units ordering ON ordering.unit_id=x.unit_id AND ordering.work_id=x.work_id
JOIN audio_publication_pointers pointer ON pointer.unit_id=x.unit_id AND pointer.active_release_id=x.release_id
JOIN audio_releases rel ON rel.id=x.release_id AND rel.unit_id=x.unit_id AND rel.owner_id=pointer.owner_id
JOIN audio_release_candidates candidate ON candidate.id=rel.candidate_id AND candidate.owner_id=rel.owner_id
JOIN audio_renditions rendition ON rendition.id=rel.rendition_id AND rendition.id=candidate.rendition_id
JOIN audio_submissions submission ON submission.id=rendition.submission_id AND submission.owner_id=rel.owner_id AND submission.unit_id=x.unit_id
JOIN audio_distribution_clearances clearance ON clearance.candidate_id=candidate.id AND clearance.owner_id=rel.owner_id AND clearance.digest=x.rights_digest
  AND clearance.sequence=(SELECT max(c2.sequence) FROM audio_distribution_clearances c2 WHERE c2.candidate_id=candidate.id AND c2.owner_id=rel.owner_id)
JOIN audio_publication_approvals approval ON approval.candidate_id=candidate.id AND approval.owner_id=rel.owner_id AND approval.digest=x.approval_digest
  AND approval.sequence=(SELECT max(a2.sequence) FROM audio_publication_approvals a2 WHERE a2.candidate_id=candidate.id AND a2.owner_id=rel.owner_id)
WHERE {current_filter} AND {actor_filter}
AND x.rights_start<=?1 AND ?1<x.rights_end AND x.published_at<=?1
AND NOT EXISTS(SELECT 1 FROM audio_release_withdrawals w WHERE w.release_id=rel.id)
AND NOT EXISTS(SELECT 1 FROM audio_submission_revocations v WHERE v.submission_id=submission.id)
"#;
const COLUMNS:&str="x.*,rel.owner_id,anchor.first_published_at,ordering.first_position,rel.snapshot AS release_doc,submission.snapshot AS submission,rendition.snapshot AS rendition,candidate.snapshot AS candidate,clearance.snapshot AS clearance,approval.snapshot AS approval,
rel.id AS release_row_id,rel.unit_id AS release_row_unit,rel.owner_id AS release_row_owner,rel.candidate_id AS release_row_candidate,rel.rendition_id AS release_row_rendition,rel.manifest_digest AS release_row_manifest,rel.digest AS release_row_digest,
submission.id AS submission_row_id,submission.owner_id AS submission_row_owner,submission.unit_id AS submission_row_unit,submission.brief_id AS submission_row_brief,submission.brief_digest AS submission_row_brief_digest,submission.operation_id AS submission_row_operation,submission.digest AS submission_row_digest,
rendition.id AS rendition_row_id,rendition.submission_id AS rendition_row_submission,rendition.object_key AS rendition_row_object,rendition.digest AS rendition_row_digest,
candidate.id AS candidate_row_id,candidate.rendition_id AS candidate_row_rendition,candidate.owner_id AS candidate_row_owner,candidate.operation_id AS candidate_row_operation,candidate.digest AS candidate_row_digest,
clearance.id AS clearance_row_id,clearance.candidate_id AS clearance_row_candidate,clearance.owner_id AS clearance_row_owner,clearance.operation_id AS clearance_row_operation,clearance.sequence AS clearance_row_sequence,clearance.digest AS clearance_row_digest,
approval.id AS approval_row_id,approval.candidate_id AS approval_row_candidate,approval.owner_id AS approval_row_owner,approval.operation_id AS approval_row_operation,approval.sequence AS approval_row_sequence,approval.digest AS approval_row_digest";

fn relation() -> String {
    FROM.replace("{current_filter}", LISTENER_CURRENT_FILTER)
        .replace("{actor_filter}", RELEASE_OWNER_ACTIVE_FILTER)
}

/// The domain's Unix milliseconds of a trusted microsecond stamp.
fn millis(micros: i64) -> i64 {
    micros.div_euclid(1000)
}

/// Keeps the first row of each Work in the release order of Work detail: latest publication,
/// then the smallest release id (PostgreSQL `DISTINCT ON`).
const REPRESENTATIVE: &str =
    "row_number() OVER (PARTITION BY x.work_id ORDER BY x.published_at DESC,x.release_id ASC) AS pick";
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
                time: r.try_get("first_published_at")?,
                id: r.try_get("work_id")?,
            }
        } else {
            Keyset::Units {
                work: r.try_get("work_id")?,
                position: u32::try_from(r.try_get::<_, i32>("first_position")?)
                    .map_err(|_| ListenerError::Corrupt)?,
                id: r.try_get("unit_id")?,
            }
        });
    }
    let requests: Vec<_> = rows
        .iter()
        .map(|r| {
            Ok(ListenerMetadataRequest {
                release: r.try_get("release_id")?,
                work: r.try_get("work_id")?,
                unit: r.try_get("unit_id")?,
                work_revision: r.try_get("work_revision")?,
                unit_revision: r.try_get("unit_revision")?,
                content: r.try_get("content_revision_id")?,
            })
        })
        .collect::<Result<_, ListenerError>>()?;
    let mut metadata: std::collections::BTreeMap<_, _> = listener_metadata(tx, &requests)
        .await?
        .into_iter()
        .map(|m| (m.release.clone(), m))
        .collect();
    let owners: Vec<String> = rows
        .iter()
        .map(|r| r.try_get("owner_id"))
        .collect::<Result<_, _>>()?;
    let active = crate::platform::store::active_actors(tx, &owners).await?;
    let mut items = Vec::new();
    for r in rows {
        let m = metadata
            .remove(&r.try_get::<_, String>("release_id")?)
            .ok_or(ListenerError::Corrupt)?;
        if let Some(item) = decode(
            &r,
            m,
            active.contains(&r.try_get::<_, String>("owner_id")?),
            now,
        )? {
            items.push(item);
        }
    }
    Ok((items, next))
}

async fn presented_batch(
    tx: &Tx<'_>,
    rows: Vec<Row>,
    limit: usize,
    works: bool,
    now: i64,
) -> Result<(Vec<ReadItem>, Option<Keyset>), ListenerError> {
    let mut stats = std::collections::BTreeMap::new();
    for row in rows.iter().take(limit) {
        let (release, value) = stats_row(row)?;
        if stats.insert(release, value).is_some() {
            return Err(ListenerError::Corrupt);
        }
    }
    let (mut items, next) = batch(tx, rows, limit, works, now).await?;
    for item in &mut items {
        item.presentation = Some(
            stats
                .remove(&item.publication.release().document().id)
                .ok_or(ListenerError::Corrupt)?,
        );
    }
    Ok((items, next))
}

/// Rank/count the eligible relation before representative selection, cursor or page limit.
/// The opt-in queries alone pay for these window aggregates.
fn presented_relation() -> String {
    format!("SELECT {COLUMNS},count(*) OVER (PARTITION BY x.work_id) AS unit_count,row_number() OVER (PARTITION BY x.work_id ORDER BY ordering.first_position,x.unit_id) AS ordinal {}", relation())
}

fn stats_row(row: &Row) -> Result<(String, PresentationStats), ListenerError> {
    let release: String = row
        .try_get("release_id")
        .map_err(|_| ListenerError::Corrupt)?;
    let count = row
        .try_get("unit_count")
        .map_err(|_| ListenerError::Corrupt)?;
    let ordinal = row.try_get("ordinal").map_err(|_| ListenerError::Corrupt)?;
    Ok((release, PresentationStats::new(count, ordinal)?))
}

pub(in crate::contexts::media) async fn presented_works(
    tx: &Tx<'_>,
    q: &DiscoveryQuery,
    after: Option<&Keyset>,
    now: i64,
) -> Result<(Vec<ReadItem>, Option<Keyset>), ListenerError> {
    let (mut items, next) = works(tx, q, after, now).await?;
    if items.is_empty() {
        return Ok((items, next));
    }
    // Select at most one page of representatives first; aggregate only their Works.
    let works: Vec<_> = items
        .iter()
        .map(|r| r.publication.work().id().as_str().to_owned())
        .collect();
    let releases: Vec<_> = items
        .iter()
        .map(|r| r.publication.release().document().id.clone())
        .collect();
    let relation = presented_relation();
    let sql = format!("WITH eligible AS ({relation} AND x.work_id IN (SELECT value FROM json_each(?2))) SELECT release_id,unit_count,ordinal FROM eligible WHERE release_id IN (SELECT value FROM json_each(?3))");
    let rows = tx
        .query(&sql, &[&now, &works, &releases])
        .await
        .map_err(|_| ListenerError::Unavailable)?;
    let mut stats = std::collections::BTreeMap::new();
    for row in rows {
        let (release, value) = stats_row(&row)?;
        if stats.insert(release, value).is_some() {
            return Err(ListenerError::Corrupt);
        }
    }
    for item in &mut items {
        item.presentation = Some(
            stats
                .remove(&item.publication.release().document().id)
                .ok_or(ListenerError::Corrupt)?,
        );
    }
    Ok((items, next))
}

pub(in crate::contexts::media) async fn presented_work(
    tx: &Tx<'_>,
    work: &str,
    now: i64,
) -> Result<(Vec<ReadItem>, Option<Keyset>), ListenerError> {
    let relation = presented_relation();
    let sql = format!("WITH eligible AS ({relation} AND x.work_id=?2) SELECT * FROM eligible ORDER BY published_at DESC,release_id ASC LIMIT 1");
    let rows = tx
        .query(&sql, &[&now, &work])
        .await
        .map_err(|_| ListenerError::Unavailable)?;
    presented_batch(tx, rows, 1, false, now).await
}

pub(in crate::contexts::media) async fn presented_units(
    tx: &Tx<'_>,
    work: &str,
    after: Option<&Keyset>,
    limit: usize,
    now: i64,
) -> Result<(Vec<ReadItem>, Option<Keyset>), ListenerError> {
    let (position, id) = unit_after(after, work)?;
    let relation = presented_relation();
    let sql = format!("WITH eligible AS ({relation} AND x.work_id=?2) SELECT * FROM eligible WHERE (?3 IS NULL OR first_position>?3 OR (first_position=?3 AND unit_id>?4)) ORDER BY first_position,unit_id LIMIT ?5");
    let rows = tx
        .query(&sql, &[&now, &work, &position, &id, &((limit + 1) as i64)])
        .await
        .map_err(|_| ListenerError::Unavailable)?;
    presented_batch(tx, rows, limit, false, now).await
}

/// The unit keyset of a cursor, bound to the Work it was issued for.
fn unit_after<'a>(
    after: Option<&'a Keyset>,
    work: &str,
) -> Result<(Option<i64>, Option<&'a str>), ListenerError> {
    match after {
        None => Ok((None, None)),
        Some(Keyset::Units {
            work: w,
            position,
            id,
        }) if w == work => Ok((Some(i64::from(*position)), Some(id.as_str()))),
        Some(_) => Err(ListenerError::InvalidCursor),
    }
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
    let from = relation();
    let sql=format!("SELECT * FROM (SELECT {COLUMNS},{REPRESENTATIVE} {from} AND (?2 IS NULL OR anchor.first_published_at<?2 OR (anchor.first_published_at=?2 AND x.work_id>?3))) selected WHERE pick=1 AND (?4 IS NULL OR kind=?4) AND (?5 IS NULL OR language=?5) AND (?6='' OR instr(search_text,?6)>0) ORDER BY first_published_at DESC,work_id ASC LIMIT ?7");
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
    let from = relation();
    let sql=format!("SELECT {COLUMNS} {from} AND x.work_id=?2 ORDER BY x.published_at DESC,x.release_id ASC LIMIT 1");
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
    let (position, id) = unit_after(after, work)?;
    let from = relation();
    let sql=format!("SELECT {COLUMNS} {from} AND x.work_id=?2 AND (?3 IS NULL OR ordering.first_position>?3 OR (ordering.first_position=?3 AND x.unit_id>?4)) ORDER BY ordering.first_position,x.unit_id LIMIT ?5");
    let rows = tx
        .query(&sql, &[&now, &work, &position, &id, &((limit + 1) as i64)])
        .await
        .map_err(|_| ListenerError::Unavailable)?;
    batch(tx, rows, limit, false, now).await
}
/// Attach each representative release's pinned cover facts in the same pinned snapshot: three
/// bounded statements for the whole page, never one per item.
pub(in crate::contexts::media) async fn with_covers(
    tx: &Tx<'_>,
    (mut items, next): (Vec<ReadItem>, Option<Keyset>),
) -> Result<(Vec<ReadItem>, Option<Keyset>), ListenerError> {
    let pinned: Vec<_> = items
        .iter()
        .map(|r| {
            let work = r.publication.work();
            PinnedWork {
                work_id: work.id().as_str().to_owned(),
                owner_id: work.owner_id().as_str().to_owned(),
                revision: work.revision(),
            }
        })
        .collect();
    let mut facts = pinned_cover_facts(tx, &pinned).await?;
    for item in &mut items {
        item.cover = Some(
            facts
                .remove(item.publication.work().id().as_str())
                .ok_or(ListenerError::Corrupt)?,
        );
    }
    Ok((items, next))
}

use super::super::listener_application::{ListenerRead, ReadFuture, ReadRequest, ReadSnapshot};
use crate::platform::store::Store;
pub(in crate::contexts::media) struct TursoRead(pub Store);
impl ListenerRead for TursoRead {
    fn read(&self, request: ReadRequest) -> ReadFuture {
        let store = self.0.clone();
        Box::pin(async move {
            // One pinned reader: every statement below sees the same committed state and the
            // read never waits for the writer. No file or object I/O happens inside it.
            let c = store.connection().await?;
            let tx = c.read().await.map_err(|_| ListenerError::Unavailable)?;
            let now = millis(tx.now_micros());
            let (mut items, next_key) = match request {
                ReadRequest::Work { work: id } => work(&tx, &id, now).await?,
                ReadRequest::Works { query, after } => {
                    works(&tx, &query, after.as_ref(), now).await?
                }
                ReadRequest::Units { work, after, limit } => {
                    units(&tx, &work, after.as_ref(), limit, now).await?
                }
                ReadRequest::PresentedWork { work } => presented_work(&tx, &work, now).await?,
                ReadRequest::PresentedWorks { query, after } => {
                    presented_works(&tx, &query, after.as_ref(), now).await?
                }
                ReadRequest::PresentedUnits { work, after, limit } => {
                    presented_units(&tx, &work, after.as_ref(), limit, now).await?
                }
                ReadRequest::CoveredWork { work } => {
                    with_covers(&tx, presented_work(&tx, &work, now).await?).await?
                }
                ReadRequest::CoveredWorks { query, after } => {
                    with_covers(
                        &tx,
                        presented_works(&tx, &query, after.as_ref(), now).await?,
                    )
                    .await?
                }
                ReadRequest::Publication { id } => (
                    super::super::generation_audio::public_release(&tx, &id)
                        .await?
                        .into_iter()
                        .collect(),
                    None,
                ),
            };
            // The trusted clock again, after the reads: an access window that closed while the
            // page was assembled removes its item instead of granting from a stale instant.
            let now = millis(
                store
                    .database()
                    .now()
                    .map_err(|_| ListenerError::Unavailable)?
                    .get(),
            )
            .max(now);
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

/// Narrow Media-owned current-public batch for the Library adapter. It returns public manifests,
/// never private storage or the immutable Studio audit envelope. No per-item SQL round trips.
/// `now` is the caller's unit instant in milliseconds.
pub(crate) async fn library_publications(
    tx: &Tx<'_>,
    units: &[String],
    now: i64,
) -> Result<Vec<crate::contract::theatre::discovery::PublicationManifest>, ListenerError> {
    Ok(library_current_items(tx, units, now)
        .await?
        .into_iter()
        .map(|r| publication_manifest(&r.publication))
        .collect())
}

pub(crate) async fn library_presentations(
    tx: &Tx<'_>,
    units: &[String],
    now: i64,
) -> Result<
    Vec<(
        crate::contract::theatre::discovery::PublicationManifest,
        crate::contract::theatre::library::AudioPresentation,
    )>,
    ListenerError,
> {
    Ok(library_current_items(tx, units, now)
        .await?
        .into_iter()
        .map(|r| {
            (
                publication_manifest(&r.publication),
                audio_presentation(&r.publication),
            )
        })
        .collect())
}

pub(in crate::contexts::media) async fn library_current_items(
    tx: &Tx<'_>,
    units: &[String],
    now: i64,
) -> Result<Vec<ReadItem>, ListenerError> {
    if units.len() > 50 {
        return Err(ListenerError::InvalidRequest);
    }
    let from = relation();
    let sql = format!("SELECT {COLUMNS} {from} AND x.unit_id IN (SELECT value FROM json_each(?2))");
    let rows = tx
        .query(&sql, &[&now, &units])
        .await
        .map_err(|_| ListenerError::Unavailable)?;
    let mut items = batch(tx, rows, 50, false, now).await?.0;
    items.retain(|r| r.publication.valid_at(now));
    Ok(items)
}

/// One representative per requested Work, selected by the same release order as detail.
pub(crate) async fn library_work_presentations(
    tx: &Tx<'_>,
    works: &[String],
    now: i64,
) -> Result<Vec<crate::contract::theatre::library::WorkPresentation>, ListenerError> {
    if works.len() > 50 {
        return Err(ListenerError::InvalidRequest);
    }
    let from = relation();
    let sql = format!("SELECT * FROM (SELECT {COLUMNS},{REPRESENTATIVE} {from} AND x.work_id IN (SELECT value FROM json_each(?2))) selected WHERE pick=1 ORDER BY work_id");
    let rows = tx
        .query(&sql, &[&now, &works])
        .await
        .map_err(|_| ListenerError::Unavailable)?;
    let mut items = batch(tx, rows, 50, true, now).await?.0;
    items.retain(|r| r.publication.valid_at(now));
    Ok(items
        .into_iter()
        .map(|r| work_presentation(&r.publication))
        .collect())
}
pub(crate) async fn library_public_work(
    tx: &Tx<'_>,
    work_id: &str,
    now: i64,
) -> Result<bool, ListenerError> {
    Ok(!work(tx, work_id, now).await?.0.is_empty())
}

/// The domain's Unix milliseconds of a transaction's trusted microsecond stamp.
pub(crate) fn unit_millis(tx: &Tx<'_>) -> i64 {
    millis(tx.now_micros())
}

#[cfg(test)]
mod presentation_tests {
    use super::*;

    #[test]
    fn presentation_stats_reject_zero_negative_unsafe_and_out_of_count_rank() {
        for (count, ordinal) in [
            (0, 1),
            (-1, 1),
            (1, 0),
            (1, -1),
            (1, 2),
            (9_007_199_254_740_992, 1),
        ] {
            assert_eq!(
                PresentationStats::new(count, ordinal),
                Err(ListenerError::Corrupt)
            );
        }
        assert_eq!(
            PresentationStats::new(9_007_199_254_740_991, 9_007_199_254_740_991),
            Ok(PresentationStats {
                unit_count: 9_007_199_254_740_991,
                ordinal: 9_007_199_254_740_991
            })
        );
    }
}

#[cfg(test)]
mod corrupt_row_tests {
    //! The read model decodes untrusted stored rows: a mistyped, NULL or missing column is
    //! `Corrupt`, never a panic.
    use super::*;
    use crate::test_support::ServerFixture;

    #[tokio::test]
    async fn mistyped_or_missing_keyset_and_request_columns_are_corrupt() {
        let fixture = ServerFixture::new().await;
        let mut cases = Vec::new();
        for (sql, limit, works) in [
            // The next-page Works keyset reads a text publication time.
            (
                "SELECT 'late' AS first_published_at,'w' AS work_id UNION ALL SELECT 1,'w'",
                1,
                true,
            ),
            // The next-page Units keyset reads a NULL position.
            (
                "SELECT 'w' AS work_id,NULL AS first_position,'u' AS unit_id UNION ALL SELECT 'w',1,'u'",
                1,
                false,
            ),
            // A metadata request reads an integer release id and has no revision columns.
            ("SELECT 7 AS release_id,'w' AS work_id,'u' AS unit_id", 5, true),
        ] {
            cases.push((fixture.rows(sql, &[]).await.unwrap(), limit, works, sql));
        }
        let conn = fixture.store().connection().await.unwrap();
        let tx = conn.read().await.unwrap();
        for (rows, limit, works, sql) in cases {
            assert!(
                matches!(
                    batch(&tx, rows, limit, works, 0).await,
                    Err(ListenerError::Corrupt)
                ),
                "{sql}"
            );
        }
        tx.rollback().await.unwrap();
    }
}

/// Full release-chain integrity and live release gates for the generation lane, with no
/// editorial visibility condition. Only `generation_audio::public_release` composes it into authority.
pub(in crate::contexts::media) async fn verified_generation_release(
    tx: &Tx<'_>,
    id: &str,
    now: i64,
) -> Result<Option<ReadItem>, ListenerError> {
    let from = FROM
        .replace(
            "{current_filter}",
            crate::contexts::catalog::LISTENER_ACTIVE_FILTER,
        )
        .replace("{actor_filter}", RELEASE_OWNER_ACTIVE_FILTER);
    let query = format!("SELECT {COLUMNS} {from} AND x.release_id=?2");
    let Some(row) = tx.query_opt(&query, &[&now, &id]).await? else {
        return Ok(None);
    };
    let request = ListenerMetadataRequest {
        release: row.try_get("release_id")?,
        work: row.try_get("work_id")?,
        unit: row.try_get("unit_id")?,
        work_revision: row.try_get("work_revision")?,
        unit_revision: row.try_get("unit_revision")?,
        content: row.try_get("content_revision_id")?,
    };
    let metadata = listener_metadata(tx, &[request])
        .await?
        .into_iter()
        .next()
        .ok_or(ListenerError::Corrupt)?;
    decode_for(
        &row,
        metadata,
        true,
        now,
        ListenerPublication::resolve_generation,
    )
}
