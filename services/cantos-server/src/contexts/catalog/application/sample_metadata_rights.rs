//! Owner attestation for only the pinned sample's metadata; not audio or legal clearance.
use super::CatalogStoreError;
use crate::{
    contexts::{
        catalog::domain::metadata_disclosure::{
            ApprovedSampleSource, APPROVED_SAMPLE_SOURCE_SHA256,
        },
        editorial::{self, EvidenceKind},
        ingestion,
    },
    contract::studio::{SampleMetadataRightsRequest, SampleMetadataRightsResponse},
    platform::store::{actor_active, authenticate, normalized_uuid, Store, StoreError, Tx},
};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Claim {
    version: String,
    operation_id: String,
    source_id: String,
    source_sha256: String,
    evidence_reference: String,
    rights_statement: String,
    confirm_metadata_rights: bool,
    confirm_source: bool,
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
}
fn rights_id(op: &str) -> String {
    format!("sample_metadata_{}", op.replace('-', ""))
}

/// A generic rights evidence row is insufficient; require this claim's exact source binding.
pub(super) async fn bound(
    tx: &Tx<'_>,
    owner: &str,
    id: &str,
    source: &str,
) -> Result<bool, StoreError> {
    let Some(text) = editorial::evidence_description(tx, owner, "rights", id).await? else {
        return Ok(false);
    };
    let Ok(claim) = serde_json::from_str::<Claim>(&text) else {
        return Ok(false);
    };
    Ok(claim.version == "sample-metadata-rights-v1"
        && normalized_uuid(&claim.operation_id).is_ok()
        && rights_id(&claim.operation_id) == id
        && claim.source_id == source
        && claim.source_sha256 == APPROVED_SAMPLE_SOURCE_SHA256
        && valid_text(&claim.evidence_reference)
        && valid_text(&claim.rights_statement)
        && claim.confirm_metadata_rights
        && claim.confirm_source)
}

impl Store {
    /// Record or replay an authenticated owner's source-bound attestation in one transaction.
    pub async fn record_sample_metadata_rights(
        &self,
        token: &str,
        request: SampleMetadataRightsRequest,
    ) -> Result<SampleMetadataRightsResponse, CatalogStoreError> {
        let operation = normalized_uuid(&request.operation_id)?;
        if !ingestion::domain::receipt::source_id_valid(&request.source_id)
            || !valid_text(&request.evidence_reference)
            || !valid_text(&request.rights_statement)
            || !request.confirm_metadata_rights
            || !request.confirm_source
        {
            return Err(StoreError::InvalidRequest.into());
        }
        let id = rights_id(&operation);
        let claim = Claim {
            version: "sample-metadata-rights-v1".into(),
            operation_id: operation,
            source_id: request.source_id.clone(),
            source_sha256: APPROVED_SAMPLE_SOURCE_SHA256.into(),
            evidence_reference: request.evidence_reference,
            rights_statement: request.rights_statement,
            confirm_metadata_rights: true,
            confirm_source: true,
        };
        let description = serde_json::to_string(&claim).map_err(|_| StoreError::CorruptRevision)?;
        let mut connection = self.connection().await?;
        let tx = connection.transaction().await?;
        let owner = authenticate(&tx, token).await?;
        if !actor_active(&tx, &owner).await? {
            return Err(StoreError::Unauthenticated.into());
        }
        let bytes = ingestion::source_bytes(&tx, &owner, &claim.source_id)
            .await?
            .ok_or(StoreError::EvidenceUnavailable)?;
        ApprovedSampleSource::parse(&bytes).map_err(|_| StoreError::EvidenceUnavailable)?;
        match editorial::evidence_description(&tx, &owner, "rights", &id).await? {
            Some(prior) if prior != description => return Err(CatalogStoreError::OperationReused),
            Some(_) => (),
            None => {
                editorial::register_evidence(&tx, &owner, EvidenceKind::Rights, &id, &description)
                    .await?
            }
        }
        if authenticate(&tx, token).await? != owner || !actor_active(&tx, &owner).await? {
            return Err(StoreError::Unauthenticated.into());
        }
        tx.commit().await?;
        Ok(SampleMetadataRightsResponse {
            rights_id: id,
            source_id: claim.source_id,
        })
    }
}
