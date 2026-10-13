//! Media-owned publication persistence. Transaction boundaries stay in the application.
use super::super::domain::{
    lifecycle::{AudioRendition, ReleaseCandidate},
    publication::{AudioRelease, DistributionClearance, PublicationApproval},
};
use crate::{
    contexts::catalog::domain::content::ContentRevision,
    platform::postgres::{checked_number, StoreError, Tx},
};
use serde::{de::DeserializeOwned, Serialize};
use tokio_postgres::Row;
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    serde_json::to_vec(value).map_err(|_| StoreError::CorruptRevision)
}
fn payload<T: DeserializeOwned + Serialize>(row: &Row, max: usize) -> Result<T, StoreError> {
    let bytes: Vec<u8> = row
        .try_get("snapshot")
        .map_err(|_| StoreError::CorruptRevision)?;
    if bytes.is_empty() || bytes.len() > max {
        return Err(StoreError::CorruptRevision);
    }
    let value: T = serde_json::from_slice(&bytes).map_err(|_| StoreError::CorruptRevision)?;
    if encode(&value)? != bytes {
        return Err(StoreError::CorruptRevision);
    }
    Ok(value)
}
fn column(row: &Row, key: &str, expected: &str) -> Result<(), StoreError> {
    if row
        .try_get::<_, String>(key)
        .map_err(|_| StoreError::CorruptRevision)?
        != expected
    {
        Err(StoreError::CorruptRevision)
    } else {
        Ok(())
    }
}
fn sequence(row: &Row, expected: u64) -> Result<(), StoreError> {
    if row
        .try_get::<_, i64>("sequence")
        .map_err(|_| StoreError::CorruptRevision)?
        != checked_number(expected)?
    {
        Err(StoreError::CorruptRevision)
    } else {
        Ok(())
    }
}
fn clearance(row: &Row, c: &ReleaseCandidate) -> Result<DistributionClearance, StoreError> {
    let value = DistributionClearance::restore(payload(row, 32768)?, c)
        .map_err(|_| StoreError::CorruptRevision)?;
    let d = value.document();
    for (key, v) in [
        ("id", &d.id),
        ("candidate_id", &d.candidate_id),
        ("owner_id", &d.actor_id),
        ("operation_id", &d.request.operation_id),
        ("digest", &d.digest),
    ] {
        column(row, key, v)?;
    }
    sequence(row, d.sequence)?;
    Ok(value)
}
fn approval(row: &Row, c: &ReleaseCandidate) -> Result<PublicationApproval, StoreError> {
    let value = PublicationApproval::restore(payload(row, 8192)?, c)
        .map_err(|_| StoreError::CorruptRevision)?;
    let d = value.document();
    for (key, v) in [
        ("id", &d.id),
        ("candidate_id", &d.candidate_id),
        ("owner_id", &d.actor_id),
        ("operation_id", &d.request.operation_id),
        ("digest", &d.digest),
    ] {
        column(row, key, v)?;
    }
    sequence(row, d.sequence)?;
    Ok(value)
}
pub(in crate::contexts::media) async fn latest_clearance(
    tx: &Tx<'_>,
    c: &ReleaseCandidate,
) -> Result<Option<DistributionClearance>, StoreError> {
    tx.query_opt("SELECT * FROM audio_distribution_clearances WHERE candidate_id=$1 ORDER BY sequence DESC LIMIT 1",&[&c.document().id]).await?.as_ref().map(|r|clearance(r,c)).transpose()
}
pub(in crate::contexts::media) async fn latest_approval(
    tx: &Tx<'_>,
    c: &ReleaseCandidate,
) -> Result<Option<PublicationApproval>, StoreError> {
    tx.query_opt("SELECT * FROM audio_publication_approvals WHERE candidate_id=$1 ORDER BY sequence DESC LIMIT 1",&[&c.document().id]).await?.as_ref().map(|r|approval(r,c)).transpose()
}
pub(in crate::contexts::media) async fn prior_clearance(
    tx: &Tx<'_>,
    owner: &str,
    op: &str,
    c: &ReleaseCandidate,
) -> Result<Option<DistributionClearance>, StoreError> {
    tx.query_opt(
        "SELECT * FROM audio_distribution_clearances WHERE owner_id=$1 AND operation_id=$2",
        &[&owner, &op],
    )
    .await?
    .as_ref()
    .map(|r| {
        if r.get::<_, String>("candidate_id") != c.document().id {
            return Err(StoreError::OperationReused);
        }
        clearance(r, c)
    })
    .transpose()
}
pub(in crate::contexts::media) async fn prior_approval(
    tx: &Tx<'_>,
    owner: &str,
    op: &str,
    c: &ReleaseCandidate,
) -> Result<Option<PublicationApproval>, StoreError> {
    tx.query_opt(
        "SELECT * FROM audio_publication_approvals WHERE owner_id=$1 AND operation_id=$2",
        &[&owner, &op],
    )
    .await?
    .as_ref()
    .map(|r| {
        if r.get::<_, String>("candidate_id") != c.document().id {
            return Err(StoreError::OperationReused);
        }
        approval(r, c)
    })
    .transpose()
}
pub(in crate::contexts::media) async fn insert_clearance(
    tx: &Tx<'_>,
    value: &DistributionClearance,
) -> Result<(), StoreError> {
    let d = value.document();
    tx.execute("INSERT INTO audio_distribution_clearances(id,candidate_id,owner_id,operation_id,sequence,snapshot,digest) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&d.id,&d.candidate_id,&d.actor_id,&d.request.operation_id,&checked_number(d.sequence)?,&encode(d)?,&d.digest]).await?;
    Ok(())
}
pub(in crate::contexts::media) async fn insert_approval(
    tx: &Tx<'_>,
    value: &PublicationApproval,
) -> Result<(), StoreError> {
    let d = value.document();
    tx.execute("INSERT INTO audio_publication_approvals(id,candidate_id,owner_id,operation_id,sequence,snapshot,digest) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&d.id,&d.candidate_id,&d.actor_id,&d.request.operation_id,&checked_number(d.sequence)?,&encode(d)?,&d.digest]).await?;
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::contexts::media) struct Pointer {
    pub version: u64,
    pub active: Option<String>,
}
pub(in crate::contexts::media) async fn pointer(
    tx: &Tx<'_>,
    unit: &str,
    owner: &str,
    write: bool,
) -> Result<Pointer, StoreError> {
    if write {
        tx.execute("INSERT INTO audio_publication_pointers(unit_id,owner_id,active_release_id,version) VALUES($1,$2,NULL,0) ON CONFLICT DO NOTHING",&[&unit,&owner]).await?;
    }
    let query = if write {
        "SELECT * FROM audio_publication_pointers WHERE unit_id=$1 FOR UPDATE"
    } else {
        "SELECT * FROM audio_publication_pointers WHERE unit_id=$1"
    };
    let Some(r) = tx.query_opt(query, &[&unit]).await? else {
        return Ok(Pointer {
            version: 0,
            active: None,
        });
    };
    column(&r, "owner_id", owner)?;
    Ok(Pointer {
        version: u64::try_from(
            r.try_get::<_, i64>("version")
                .map_err(|_| StoreError::CorruptRevision)?,
        )
        .map_err(|_| StoreError::CorruptRevision)?,
        active: r
            .try_get("active_release_id")
            .map_err(|_| StoreError::CorruptRevision)?,
    })
}
pub(in crate::contexts::media) async fn release_candidate(
    tx: &Tx<'_>,
    release: &str,
    unit: &str,
    owner: &str,
) -> Result<Option<String>, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT candidate_id FROM audio_releases WHERE id=$1 AND unit_id=$2 AND owner_id=$3",
            &[&release, &unit, &owner],
        )
        .await?
        .map(|r| r.get(0)))
}
pub(in crate::contexts::media) async fn release(
    tx: &Tx<'_>,
    id: &str,
    c: &ReleaseCandidate,
    r: &AudioRendition,
    content: &ContentRevision,
) -> Result<AudioRelease, StoreError> {
    let row = tx
        .query_opt("SELECT * FROM audio_releases WHERE id=$1", &[&id])
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    let value = AudioRelease::restore(payload(&row, 262144)?, c, r, content)
        .map_err(|_| StoreError::CorruptRevision)?;
    let d = value.document();
    let m = &d.manifest;
    for (key, v) in [
        ("id", &d.id),
        ("unit_id", &m.unit_id),
        ("owner_id", &d.published_by),
        ("candidate_id", &m.candidate_id),
        ("rendition_id", &m.rendition_id),
        ("manifest_digest", &m.digest),
        ("digest", &d.digest),
    ] {
        column(&row, key, v)?;
    }
    Ok(value)
}
pub(in crate::contexts::media) async fn insert_release(
    tx: &Tx<'_>,
    release: &AudioRelease,
) -> Result<(), StoreError> {
    let d = release.document();
    let m = &d.manifest;
    tx.execute("INSERT INTO audio_releases(id,unit_id,owner_id,candidate_id,rendition_id,snapshot,manifest_digest,digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",&[&d.id,&m.unit_id,&d.published_by,&m.candidate_id,&m.rendition_id,&encode(d)?,&m.digest,&d.digest]).await?;
    Ok(())
}
pub(in crate::contexts::media) async fn set_pointer(
    tx: &Tx<'_>,
    unit: &str,
    owner: &str,
    expected: &Pointer,
    active: Option<&str>,
    next: u64,
) -> Result<(), StoreError> {
    let count=tx.execute("UPDATE audio_publication_pointers SET active_release_id=$1,version=$2 WHERE unit_id=$3 AND owner_id=$4 AND version=$5 AND active_release_id IS NOT DISTINCT FROM $6",&[&active,&checked_number(next)?,&unit,&owner,&checked_number(expected.version)?,&expected.active]).await?;
    if count != 1 {
        return Err(StoreError::StaleRevision(expected.version));
    }
    Ok(())
}
pub(in crate::contexts::media) struct Operation {
    pub request_digest: String,
    pub unit: String,
    pub release: String,
    pub version: u64,
}
pub(in crate::contexts::media) async fn prior(
    tx: &Tx<'_>,
    owner: &str,
    op: &str,
    kind: &str,
) -> Result<Option<Operation>, StoreError> {
    tx.query_opt("SELECT * FROM audio_publication_operations WHERE owner_id=$1 AND operation_id=$2 AND kind=$3",&[&owner,&op,&kind]).await?.map(|r|Ok(Operation{request_digest:r.get("request_digest"),unit:r.get("unit_id"),release:r.get("release_id"),version:u64::try_from(r.get::<_,i64>("pointer_version")).map_err(|_|StoreError::CorruptRevision)?})).transpose()
}
pub(in crate::contexts::media) async fn record(
    tx: &Tx<'_>,
    owner: &str,
    op: &str,
    kind: &str,
    value: &Operation,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO audio_publication_operations VALUES($1,$2,$3,$4,$5,$6,$7)",
        &[
            &owner,
            &op,
            &kind,
            &value.request_digest,
            &value.unit,
            &value.release,
            &checked_number(value.version)?,
        ],
    )
    .await?;
    Ok(())
}
pub(in crate::contexts::media) struct Event<'a> {
    pub id: &'a str,
    pub unit: &'a str,
    pub version: u64,
    pub kind: &'a str,
    pub release: &'a str,
    pub owner: &'a str,
    pub now: i64,
}
pub(in crate::contexts::media) async fn event(
    tx: &Tx<'_>,
    event: Event<'_>,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO audio_publication_events VALUES($1,$2,$3,$4,$5,$6,$7)",
        &[
            &event.id,
            &event.unit,
            &checked_number(event.version)?,
            &event.kind,
            &event.release,
            &event.owner,
            &event.now,
        ],
    )
    .await?;
    Ok(())
}
pub(in crate::contexts::media) async fn withdrawn(
    tx: &Tx<'_>,
    release: &str,
) -> Result<bool, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT release_id FROM audio_release_withdrawals WHERE release_id=$1",
            &[&release],
        )
        .await?
        .is_some())
}
pub(in crate::contexts::media) async fn withdraw(
    tx: &Tx<'_>,
    release: &str,
    owner: &str,
    now: i64,
    reason: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO audio_release_withdrawals VALUES($1,$2,$3,$4)",
        &[&release, &owner, &now, &reason],
    )
    .await?;
    Ok(())
}
