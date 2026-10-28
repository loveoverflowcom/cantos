//! Owner-only, source-bound sample metadata decisions. No listener route is exposed here.
use super::{
    owner_work::{owner_work, owner_work_now, recheck_owner_work},
    CatalogStoreError,
};
use crate::contexts::{
    catalog::{
        domain::{
            metadata_disclosure::{
                authorize_publish, sample_membership, ApprovedSampleSource, MetadataEvidence,
                PublishFacts, SampleMembership, APPROVED_SAMPLE_SOURCE_SHA256,
            },
            ContentUnitId, WorkId,
        },
        turso,
    },
    ingestion,
};
use crate::contract::studio::{
    SampleMetadataDisclosureRequest as Request, SampleMetadataDisclosureResponse as Response,
};
use crate::platform::store::{normalized_uuid, Store, StoreError};

impl Store {
    /// Record one Work or unit decision with an authenticated current owner and operation ID.
    /// A withdrawal remains possible after rights have been revoked or metadata archived.
    pub async fn sample_metadata_decide(
        &self,
        token: &str,
        work: &str,
        unit: Option<&str>,
        request: Request,
    ) -> Result<Response, CatalogStoreError> {
        let work_id = WorkId::parse(work).map_err(|_| StoreError::NotFound)?;
        let unit_id = unit
            .map(ContentUnitId::parse)
            .transpose()
            .map_err(|_| StoreError::NotFound)?;
        let (kind, subject) = match &unit_id {
            Some(id) => ("unit", id.as_str()),
            None => ("work", work_id.as_str()),
        };
        if sample_membership(
            APPROVED_SAMPLE_SOURCE_SHA256,
            work_id.as_str(),
            unit_id.as_ref().map(ContentUnitId::as_str),
        ) != SampleMembership::Approved
        {
            return Err(StoreError::NotFound.into());
        }
        let operation = normalized_uuid(request.operation_id())?;
        let encoded = serde_json::to_vec(&(work_id.as_str(), kind, subject, &request))
            .map_err(|_| StoreError::InvalidRequest)?;
        if encoded.len() > 4096 {
            return Err(CatalogStoreError::Limit);
        }
        let fingerprint = turso::digest(&encoded);
        let mut connection = self.connection().await?;
        let tx = connection.transaction().await.map_err(StoreError::from)?;
        let owner = owner_work(&tx, token, &work_id).await?;
        let actor = owner.actor.as_str();
        let unit_value = if let Some(id) = &unit_id {
            let found = super::unit(&tx, id).await?;
            if found.work_id() != &work_id {
                return Err(StoreError::NotFound.into());
            }
            Some(found)
        } else {
            None
        };
        if let Some(row) = turso::sample_metadata::operation(&tx, actor, &operation).await? {
            let prior: String = row.try_get("fingerprint")?;
            if prior != fingerprint {
                return Err(CatalogStoreError::OperationReused);
            }
            let sequence: i64 = row.try_get("sequence")?;
            let reply = Response {
                work_id: row.try_get("work_id")?,
                subject_kind: row.try_get("subject_kind")?,
                subject_id: row.try_get("subject_id")?,
                sequence: u64::try_from(sequence).map_err(|_| StoreError::CorruptRevision)?,
                decision: row.try_get("decision")?,
                recorded_at_ms: row.try_get("recorded_at_ms")?,
            };
            if reply.work_id != work
                || reply.subject_kind != kind
                || reply.subject_id != subject
                || reply.sequence == 0
            {
                return Err(StoreError::CorruptRevision.into());
            }
            recheck_owner_work(&tx, token, &owner).await?;
            tx.commit().await.map_err(StoreError::from)?;
            return Ok(reply);
        }
        let row = turso::sample_metadata::max_sequence(&tx, kind, subject).await?;
        let prior: i64 = row.try_get("sequence")?;
        let sequence = prior.checked_add(1).ok_or(StoreError::CorruptRevision)?;
        let now = owner_work_now(&tx).await?;
        let decision = match &request {
            Request::Disclose {
                expected_work_revision,
                expected_unit_revision,
                source_id,
                rights_id,
                confirm_metadata_rights,
                confirm_source,
                ..
            } => {
                if !confirm_metadata_rights || !confirm_source {
                    return Err(StoreError::InvalidRequest.into());
                }
                if !ingestion::domain::receipt::source_id_valid(source_id) {
                    return Err(StoreError::InvalidRequest.into());
                }
                if *expected_work_revision != owner.work.revision() {
                    return Err(super::super::domain::CatalogError::RevisionConflict {
                        current_revision: owner.work.revision(),
                        expected_revision: *expected_work_revision,
                    }
                    .into());
                }
                match (&unit_value, expected_unit_revision) {
                    (None, None) => {}
                    (Some(value), Some(expected)) if value.revision() == *expected => {}
                    (Some(value), Some(expected)) => {
                        return Err(super::super::domain::CatalogError::RevisionConflict {
                            current_revision: value.revision(),
                            expected_revision: *expected,
                        }
                        .into())
                    }
                    _ => return Err(StoreError::InvalidRequest.into()),
                }
                if rights_id.is_empty()
                    || rights_id.len() > 64
                    || !rights_id
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
                {
                    return Err(StoreError::InvalidRequest.into());
                }
                if !super::sample_metadata_rights::bound(
                    &tx,
                    actor,
                    rights_id,
                    source_id,
                    APPROVED_SAMPLE_SOURCE_SHA256,
                )
                .await?
                {
                    return Err(StoreError::EvidenceUnavailable.into());
                }
                let bytes = ingestion::source_bytes(&tx, actor, source_id)
                    .await?
                    .ok_or(StoreError::EvidenceUnavailable)?;
                let source = ApprovedSampleSource::parse(&bytes)
                    .map_err(|_| StoreError::EvidenceUnavailable)?;
                authorize_publish(PublishFacts {
                    actor: &owner.actor,
                    owner_active: true,
                    work: &owner.work,
                    unit: unit_value.as_ref(),
                    source: &source,
                    evidence: MetadataEvidence::Confirmed,
                })
                .map_err(|_| StoreError::EvidenceUnavailable)?;
                let work_rev =
                    i64::try_from(owner.work.revision()).map_err(|_| StoreError::InvalidRequest)?;
                let wr = turso::sample_metadata::work_digest(&tx, work, work_rev).await?;
                let work_digest: String = wr.try_get("snapshot_digest")?;
                let unit_rev = unit_value
                    .as_ref()
                    .map(|value| {
                        i64::try_from(value.revision()).map_err(|_| StoreError::InvalidRequest)
                    })
                    .transpose()?;
                let unit_digest: Option<String> = if let Some(rev) = unit_rev {
                    Some(
                        turso::sample_metadata::unit_digest(&tx, subject, rev)
                            .await?
                            .try_get("snapshot_digest")?,
                    )
                } else {
                    None
                };
                turso::sample_metadata::insert_disclosed(
                    &tx,
                    work,
                    kind,
                    subject,
                    sequence,
                    actor,
                    work_rev,
                    &work_digest,
                    unit_rev,
                    &unit_digest,
                    source_id,
                    rights_id,
                    now,
                    &operation,
                    &fingerprint,
                )
                .await?;
                "disclosed"
            }
            Request::Withdraw { .. } => {
                if prior == 0 {
                    return Err(StoreError::NotFound.into());
                }
                turso::sample_metadata::insert_withdrawn(
                    &tx,
                    work,
                    kind,
                    subject,
                    sequence,
                    actor,
                    now,
                    &operation,
                    &fingerprint,
                )
                .await?;
                "withdrawn"
            }
        };
        turso::bump_clock(&tx).await?;
        recheck_owner_work(&tx, token, &owner).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(Response {
            work_id: work.to_owned(),
            subject_kind: kind.to_owned(),
            subject_id: subject.to_owned(),
            sequence: u64::try_from(sequence).map_err(|_| StoreError::CorruptRevision)?,
            decision: decision.to_owned(),
            recorded_at_ms: now,
        })
    }
}
