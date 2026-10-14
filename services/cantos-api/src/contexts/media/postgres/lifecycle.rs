//! Media-owned row admission. Corrupt snapshots never masquerade as proof values.
use super::super::domain::lifecycle::*;
use super::super::domain::object_storage::{
    ObjectIntegrity, ObjectKey, PrivateObjectRef, StorageTargetId,
};
use super::super::storage::registry::RegistryStoredAudio;
use crate::platform::postgres::{StoreError, Tx};
use cantos_contracts as dto;
use serde::{de::DeserializeOwned, Serialize};
use tokio_postgres::Row;

fn corrupt<T>(result: Result<T, AudioError>) -> Result<T, StoreError> {
    result.map_err(|_| StoreError::CorruptRevision)
}
fn payload<T: DeserializeOwned + Serialize>(row: &Row) -> Result<T, StoreError> {
    let bytes: Vec<u8> = row
        .try_get("snapshot")
        .map_err(|_| StoreError::CorruptRevision)?;
    if bytes.is_empty() || bytes.len() > 32768 {
        return Err(StoreError::CorruptRevision);
    }
    let value: T = serde_json::from_slice(&bytes).map_err(|_| StoreError::CorruptRevision)?;
    if serde_json::to_vec(&value).map_err(|_| StoreError::CorruptRevision)? != bytes {
        return Err(StoreError::CorruptRevision);
    }
    Ok(value)
}
fn matches(row: &Row, key: &str, value: &str) -> Result<(), StoreError> {
    if row
        .try_get::<_, String>(key)
        .map_err(|_| StoreError::CorruptRevision)?
        != value
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(())
}
pub(in crate::contexts::media) struct SubmissionRow {
    pub value: AudioSubmission,
    pub token_hash: Vec<u8>,
    pub request_digest: String,
}
fn submission(row: &Row) -> Result<SubmissionRow, StoreError> {
    let value = corrupt(AudioSubmission::restore(payload(row)?))?;
    let d = value.document();
    for (key, value) in [
        ("id", d.id.as_str()),
        ("owner_id", &d.owner_id),
        ("unit_id", &d.binding.unit_id),
        ("brief_id", &d.binding.brief_id),
        ("brief_digest", &d.binding.brief_digest),
        ("operation_id", &d.operation_id),
        ("digest", &d.digest),
    ] {
        matches(row, key, value)?;
    }
    let token_hash: Vec<u8> = row
        .try_get("token_hash")
        .map_err(|_| StoreError::CorruptRevision)?;
    if token_hash.len() != 32 {
        return Err(StoreError::CorruptRevision);
    }
    Ok(SubmissionRow {
        value,
        token_hash,
        request_digest: row.get("request_digest"),
    })
}
pub(in crate::contexts::media) async fn load(
    tx: &Tx<'_>,
    id: &str,
) -> Result<Option<SubmissionRow>, StoreError> {
    tx.query_opt("SELECT * FROM audio_submissions WHERE id=$1", &[&id])
        .await?
        .as_ref()
        .map(submission)
        .transpose()
}
pub(in crate::contexts::media) async fn prior(
    tx: &Tx<'_>,
    owner: &str,
    operation: &str,
) -> Result<Option<SubmissionRow>, StoreError> {
    tx.query_opt(
        "SELECT * FROM audio_submissions WHERE owner_id=$1 AND operation_id=$2",
        &[&owner, &operation],
    )
    .await?
    .as_ref()
    .map(submission)
    .transpose()
}
pub(in crate::contexts::media) async fn token_exists(
    tx: &Tx<'_>,
    hash: &[u8],
) -> Result<bool, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT id FROM audio_submissions WHERE token_hash=$1",
            &[&hash],
        )
        .await?
        .is_some())
}
pub(in crate::contexts::media) async fn insert(
    tx: &Tx<'_>,
    value: &AudioSubmission,
    hash: &[u8],
    request_digest: &str,
) -> Result<(), StoreError> {
    let d = value.document();
    let bytes = serde_json::to_vec(d).map_err(|_| StoreError::CorruptRevision)?;
    tx.execute("INSERT INTO audio_submissions(id,unit_id,owner_id,brief_id,brief_digest,operation_id,request_digest,token_hash,snapshot,digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",&[&d.id,&d.binding.unit_id,&d.owner_id,&d.binding.brief_id,&d.binding.brief_digest,&d.operation_id,&request_digest,&hash,&bytes,&d.digest]).await?;
    Ok(())
}
pub(in crate::contexts::media) async fn revoked(tx: &Tx<'_>, id: &str) -> Result<bool, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT submission_id FROM audio_submission_revocations WHERE submission_id=$1",
            &[&id],
        )
        .await?
        .is_some())
}
pub(in crate::contexts::media) async fn revoke(
    tx: &Tx<'_>,
    id: &str,
    owner: &str,
    now: i64,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO audio_submission_revocations VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
        &[&id, &owner, &now],
    )
    .await?;
    Ok(())
}
pub(in crate::contexts::media) async fn now(tx: &Tx<'_>) -> Result<i64, StoreError> {
    Ok(tx
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .await?
        .get(0))
}
pub(in crate::contexts::media) async fn rendition(
    tx: &Tx<'_>,
    s: &AudioSubmission,
) -> Result<Option<AudioRendition>, StoreError> {
    let Some(row) = tx
        .query_opt(
            "SELECT * FROM audio_renditions WHERE submission_id=$1",
            &[&s.document().id],
        )
        .await?
    else {
        return Ok(None);
    };
    let value = corrupt(AudioRendition::restore(payload(&row)?, s))?;
    let d = value.document();
    for (key, value) in [
        ("id", d.id.as_str()),
        ("submission_id", &d.submission_id),
        ("digest", &d.digest),
        ("object_key", &d.verified_media.sha256),
    ] {
        matches(&row, key, value)?;
    }
    Ok(Some(value))
}
/// Resolve the exact accepted rendition's persisted target, never the active write default.
pub(in crate::contexts::media) async fn location(
    tx: &Tx<'_>,
    value: &AudioRendition,
) -> Result<PrivateObjectRef, StoreError> {
    let d = value.document();
    let row = tx.query_opt(
        "SELECT object_key,storage_target_id FROM audio_renditions WHERE id=$1 AND submission_id=$2",
        &[&d.id, &d.submission_id],
    ).await?.ok_or(StoreError::CorruptRevision)?;
    let key: String = row
        .try_get("object_key")
        .map_err(|_| StoreError::CorruptRevision)?;
    let target: String = row
        .try_get("storage_target_id")
        .map_err(|_| StoreError::CorruptRevision)?;
    let key = ObjectKey::parse(&key).map_err(|_| StoreError::CorruptRevision)?;
    let target = StorageTargetId::parse(&target).map_err(|_| StoreError::CorruptRevision)?;
    let integrity = ObjectIntegrity::new(&d.verified_media.sha256, d.verified_media.byte_length)
        .map_err(|_| StoreError::CorruptRevision)?;
    PrivateObjectRef::new(target, key, integrity).map_err(|_| StoreError::CorruptRevision)
}

pub(in crate::contexts::media) async fn insert_rendition(
    tx: &Tx<'_>,
    value: &AudioRendition,
    stored: &RegistryStoredAudio,
) -> Result<(), StoreError> {
    let d = value.document();
    if stored.object_key != d.verified_media.sha256 {
        return Err(StoreError::CorruptRevision);
    }
    let bytes = serde_json::to_vec(d).map_err(|_| StoreError::CorruptRevision)?;
    tx.execute(
        "INSERT INTO audio_renditions(id,submission_id,object_key,snapshot,digest,storage_target_id) VALUES($1,$2,$3,$4,$5,$6)",
        &[
            &d.id,
            &d.submission_id,
            &stored.object_key,
            &bytes,
            &d.digest,
            &stored.storage_target.as_str(),
        ],
    )
    .await?;
    Ok(())
}
pub(in crate::contexts::media) async fn reviews(
    tx: &Tx<'_>,
    r: &AudioRendition,
) -> Result<Vec<dto::AudioReviewDocument>, StoreError> {
    tx.query(
        "SELECT * FROM audio_reviews WHERE rendition_id=$1 ORDER BY sequence DESC LIMIT 1001",
        &[&r.document().id],
    )
    .await?
    .iter()
    .map(|row| {
        let d: dto::AudioReviewDocument = payload(row)?;
        if corrupt(AudioReview::restore(d.clone(), r))?.document() != &d {
            return Err(StoreError::CorruptRevision);
        }
        for (key, value) in [
            ("id", d.id.as_str()),
            ("rendition_id", &d.rendition_id),
            ("owner_id", &d.reviewer_id),
            ("operation_id", &d.operation_id),
            ("digest", &d.digest),
        ] {
            matches(row, key, value)?;
        }
        if row.get::<_, i64>("sequence") != d.sequence as i64 {
            return Err(StoreError::CorruptRevision);
        }
        Ok(d)
    })
    .collect()
}
pub(in crate::contexts::media) async fn review_prior(
    tx: &Tx<'_>,
    owner: &str,
    operation: &str,
) -> Result<Option<String>, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT rendition_id FROM audio_reviews WHERE owner_id=$1 AND operation_id=$2",
            &[&owner, &operation],
        )
        .await?
        .map(|r| r.get(0)))
}
pub(in crate::contexts::media) async fn insert_review(
    tx: &Tx<'_>,
    value: &AudioReview,
) -> Result<(), StoreError> {
    let d = value.document();
    let bytes = serde_json::to_vec(d).map_err(|_| StoreError::CorruptRevision)?;
    tx.execute(
        "INSERT INTO audio_reviews VALUES($1,$2,$3,$4,$5,$6,$7)",
        &[
            &d.id,
            &d.rendition_id,
            &d.reviewer_id,
            &d.operation_id,
            &(d.sequence as i64),
            &bytes,
            &d.digest,
        ],
    )
    .await?;
    Ok(())
}
fn candidate(row: &Row, r: &AudioRendition) -> Result<dto::ReleaseCandidateDocument, StoreError> {
    let d: dto::ReleaseCandidateDocument = payload(row)?;
    if corrupt(ReleaseCandidate::restore(d.clone(), r))?.document() != &d {
        return Err(StoreError::CorruptRevision);
    }
    for (key, value) in [
        ("id", d.id.as_str()),
        ("rendition_id", &d.rendition_id),
        ("owner_id", &d.created_by),
        ("operation_id", &d.operation_id),
        ("digest", &d.digest),
    ] {
        matches(row, key, value)?;
    }
    Ok(d)
}
pub(in crate::contexts::media) async fn candidate_prior(
    tx: &Tx<'_>,
    owner: &str,
    operation: &str,
    r: &AudioRendition,
) -> Result<Option<dto::ReleaseCandidateDocument>, StoreError> {
    let row = tx
        .query_opt(
            "SELECT * FROM audio_release_candidates WHERE owner_id=$1 AND operation_id=$2",
            &[&owner, &operation],
        )
        .await?;
    if row
        .as_ref()
        .is_some_and(|row| row.get::<_, String>("rendition_id") != r.document().id)
    {
        return Err(StoreError::OperationReused);
    }
    row.as_ref().map(|row| candidate(row, r)).transpose()
}
pub(in crate::contexts::media) async fn load_candidate(
    tx: &Tx<'_>,
    id: &str,
    r: &AudioRendition,
) -> Result<Option<dto::ReleaseCandidateDocument>, StoreError> {
    tx.query_opt(
        "SELECT * FROM audio_release_candidates WHERE id=$1 AND rendition_id=$2",
        &[&id, &r.document().id],
    )
    .await?
    .as_ref()
    .map(|row| candidate(row, r))
    .transpose()
}
pub(in crate::contexts::media) async fn insert_candidate(
    tx: &Tx<'_>,
    value: &ReleaseCandidate,
) -> Result<(), StoreError> {
    let d = value.document();
    let bytes = serde_json::to_vec(d).map_err(|_| StoreError::CorruptRevision)?;
    tx.execute(
        "INSERT INTO audio_release_candidates VALUES($1,$2,$3,$4,$5,$6)",
        &[
            &d.id,
            &d.rendition_id,
            &d.created_by,
            &d.operation_id,
            &bytes,
            &d.digest,
        ],
    )
    .await?;
    Ok(())
}

pub(in crate::contexts::media) async fn candidate_submission(
    tx: &Tx<'_>,
    id: &str,
) -> Result<Option<String>, StoreError> {
    Ok(tx.query_opt("SELECT r.submission_id FROM audio_release_candidates c JOIN audio_renditions r ON r.id=c.rendition_id WHERE c.id=$1",&[&id]).await?.map(|r|r.get(0)))
}
