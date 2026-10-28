//! Narrow stopped-database operator transaction. This module is not an HTTP authority.
use sha2::{Digest, Sha256};

use super::{sample_metadata_rights, CatalogStoreError};
use crate::contexts::{
    catalog::{
        domain::{
            metadata_disclosure::{
                authorize_publish, sample_ids_for_source, ApprovedSampleSource, MetadataEvidence,
                PublishFacts, FICTIONAL_SAMPLE_SOURCE_SHA256,
            },
            sample_seed_request::{SampleSeedAction, SampleSeedRequest},
            ContentUnitId, WorkId,
        },
        turso,
    },
    ingestion,
};
use crate::platform::{
    operator::{sample_seed_principal, FICTIONAL_SEED_ACTOR, SAMPLE_SEED_ACTOR},
    store::{Store, StoreError},
};

const SOURCE_ID: &str = "approved_sample_catalog_v1";

// The identifiers of the individual immutable decisions are stable across retries and distinct
// from the caller's batch operation. No client token or session is synthesized.
fn decision_operation(operation: &str, kind: &str, subject: &str) -> String {
    let hash = Sha256::digest(format!(
        "cantos-sample-seed-v1\0{operation}\0{kind}\0{subject}"
    ));
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

/// Publish the entire pinned sample once or withdraw it; same operation and exact bytes replay.
/// A new publication after withdrawal is deliberately unsupported by this narrow first version.
pub async fn seed_sample(
    store: &Store,
    request: &SampleSeedRequest,
    bytes: Option<&[u8]>,
) -> Result<(), CatalogStoreError> {
    request.validate().map_err(|_| StoreError::InvalidRequest)?;
    let source = match request.action {
        SampleSeedAction::Publish => Some(
            ApprovedSampleSource::parse(bytes.ok_or(StoreError::InvalidRequest)?)
                .map_err(|_| StoreError::EvidenceUnavailable)?,
        ),
        SampleSeedAction::Withdraw => None,
    };
    if source
        .as_ref()
        .is_some_and(|value| value.source_sha256() != request.source_sha256)
    {
        return Err(StoreError::EvidenceUnavailable.into());
    }
    let actor = if request.source_sha256 == FICTIONAL_SAMPLE_SOURCE_SHA256 {
        FICTIONAL_SEED_ACTOR
    } else {
        SAMPLE_SEED_ACTOR
    };
    let source_id = if actor == FICTIONAL_SEED_ACTOR {
        "fictional_sample_catalog_v2"
    } else {
        SOURCE_ID
    };
    let encoded = serde_json::to_vec(request).map_err(|_| StoreError::InvalidRequest)?;
    let fingerprint = turso::digest(&encoded);
    let mut conn = store.connection().await?;
    let tx = conn.transaction().await?;
    if let Some(row) =
        turso::sample_seed::operation(&tx, &request.operation_id, &request.source_sha256).await?
    {
        let prior: String = row.try_get("fingerprint")?;
        if prior != fingerprint {
            return Err(CatalogStoreError::OperationReused);
        }
        sample_seed_principal(&tx, false, &request.source_sha256, actor).await?;
        tx.commit().await?;
        return Ok(());
    }
    let prior_action = turso::sample_seed::latest(&tx, &request.source_sha256)
        .await?
        .map(|row| row.try_get::<_, String>("action"))
        .transpose()?;
    match request.action {
        SampleSeedAction::Publish if prior_action.is_none() => (),
        SampleSeedAction::Withdraw if prior_action.as_deref() == Some("publish") => (),
        _ => return Err(StoreError::InvalidRequest.into()),
    }
    let now = turso::now(&tx).await?;
    let created = sample_seed_principal(
        &tx,
        request.action == SampleSeedAction::Publish,
        &request.source_sha256,
        actor,
    )
    .await?;
    match request.action {
        SampleSeedAction::Publish => {
            if !created {
                return Err(StoreError::InvalidRequest.into());
            }
            let source = source.as_ref().ok_or(StoreError::InvalidRequest)?;
            let snapshots = source.sample_seed_snapshots(now)?;
            ingestion::record_pinned_sample_source(
                &tx,
                actor,
                source_id,
                &request.evidence_reference,
                bytes.ok_or(StoreError::InvalidRequest)?,
            )
            .await?;
            let rights = sample_metadata_rights::record_seed_attestation(
                &tx,
                actor,
                &request.operation_id,
                source_id,
                &request.evidence_reference,
                &request.rights_statement,
                &request.source_sha256,
            )
            .await?;
            if !sample_metadata_rights::bound(
                &tx,
                actor,
                &rights,
                source_id,
                &request.source_sha256,
            )
            .await?
            {
                return Err(StoreError::EvidenceUnavailable.into());
            }
            for (work, units) in &snapshots {
                if turso::load_work(&tx, work.id().as_str()).await?.is_some() {
                    return Err(CatalogStoreError::DuplicateId);
                }
                authorize_publish(PublishFacts {
                    actor: work.owner_id(),
                    owner_active: true,
                    work,
                    unit: None,
                    source,
                    evidence: MetadataEvidence::Confirmed,
                })
                .map_err(|_| StoreError::EvidenceUnavailable)?;
                turso::save_work(&tx, work, actor, true).await?;
                let work_id = work.id().as_str();
                let digest: String = turso::sample_metadata::work_digest(&tx, work_id, 1)
                    .await?
                    .try_get("snapshot_digest")?;
                let op = decision_operation(&request.operation_id, "work", work_id);
                turso::sample_metadata::insert_disclosed(
                    &tx,
                    work_id,
                    "work",
                    work_id,
                    1,
                    actor,
                    1,
                    &digest,
                    None,
                    &None,
                    source_id,
                    &rights,
                    now,
                    &op,
                    &fingerprint,
                )
                .await?;
                for unit in units {
                    authorize_publish(PublishFacts {
                        actor: work.owner_id(),
                        owner_active: true,
                        work,
                        unit: Some(unit),
                        source,
                        evidence: MetadataEvidence::Confirmed,
                    })
                    .map_err(|_| StoreError::EvidenceUnavailable)?;
                    if turso::load_unit(&tx, unit.id().as_str()).await?.is_some() {
                        return Err(CatalogStoreError::DuplicateId);
                    }
                    turso::save_unit(&tx, unit, actor, true).await?;
                    let id = unit.id().as_str();
                    let unit_digest = Some(
                        turso::sample_metadata::unit_digest(&tx, id, 1)
                            .await?
                            .try_get("snapshot_digest")?,
                    );
                    let op = decision_operation(&request.operation_id, "unit", id);
                    turso::sample_metadata::insert_disclosed(
                        &tx,
                        work_id,
                        "unit",
                        id,
                        1,
                        actor,
                        1,
                        &digest,
                        Some(1),
                        &unit_digest,
                        source_id,
                        &rights,
                        now,
                        &op,
                        &fingerprint,
                    )
                    .await?;
                }
            }
        }
        SampleSeedAction::Withdraw => {
            if created {
                return Err(StoreError::InvalidRequest.into());
            }
            for (work_id, units) in
                sample_ids_for_source(&request.source_sha256).ok_or(StoreError::InvalidRequest)?
            {
                let work = super::work(&tx, &WorkId::parse(work_id)?).await?;
                if work.owner_id().as_str() != actor {
                    return Err(StoreError::InvalidRequest.into());
                }
                for (kind, id) in
                    std::iter::once(("work", *work_id)).chain(units.iter().map(|id| ("unit", *id)))
                {
                    if kind == "unit"
                        && super::unit(&tx, &ContentUnitId::parse(id)?)
                            .await?
                            .work_id()
                            != work.id()
                    {
                        return Err(StoreError::InvalidRequest.into());
                    }
                    let latest = turso::sample_metadata::latest(&tx, kind, id)
                        .await?
                        .ok_or(StoreError::InvalidRequest)?;
                    let decision: String = latest.try_get("decision")?;
                    if decision != "disclosed" {
                        return Err(StoreError::InvalidRequest.into());
                    }
                    let previous: i64 = latest.try_get("sequence")?;
                    let sequence = previous.checked_add(1).ok_or(StoreError::CorruptRevision)?;
                    let op = decision_operation(&request.operation_id, kind, id);
                    turso::sample_metadata::insert_withdrawn(
                        &tx,
                        work_id,
                        kind,
                        id,
                        sequence,
                        actor,
                        now,
                        &op,
                        &fingerprint,
                    )
                    .await?;
                }
            }
        }
    }
    turso::bump_clock(&tx).await?;
    let action = match request.action {
        SampleSeedAction::Publish => "publish",
        SampleSeedAction::Withdraw => "withdraw",
    };
    turso::sample_seed::insert(&tx, actor, action, request, &fingerprint, now).await?;
    tx.commit().await?;
    Ok(())
}
