//! Casting state: read settings, rights and snapshot history, record new settings or rights
//! claim versions, and preview the inputs current facts would freeze.
use cantos_contracts::{
    ProductionPreviewResponse, ProductionRightsClaim, ProductionRightsClaimResponse,
    ProductionRightsSubject, ProductionSettingsResponse, ProductionStateResponse,
    SaveProductionRightsRequest, SaveProductionSettingsRequest,
};

use super::super::domain::{
    decide_rights_version, decide_settings_version, prepare_inputs, validate_rights_declaration,
    validate_settings,
};
use super::super::postgres::{self, encode};
use super::{facts, production_actor};
use crate::contexts::editorial::{
    ensure_evidence, load_revision, rights_cited_by_script, Action, EvidenceKind,
};
use crate::platform::postgres::{checked_number, normalized_uuid, LockMode, Store, StoreError};

impl Store {
    pub async fn production_state(
        &self,
        token: &str,
        script: &str,
    ) -> Result<ProductionStateResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Read, LockMode::Share).await?;
        let snapshots = postgres::snapshot_history(&tx, &script).await?;
        let result = ProductionStateResponse {
            settings: facts.settings,
            rights: facts.rights,
            snapshots,
        };
        tx.commit().await?;
        Ok(result)
    }

    pub async fn save_production_settings(
        &self,
        token: &str,
        script: &str,
        request: SaveProductionSettingsRequest,
    ) -> Result<ProductionSettingsResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let operation = normalized_uuid(&request.operation_id)?;
        let bytes = encode(&request, 256 * 1024)?;
        let settings_bytes = encode(&request.settings, 256 * 1024)?;
        validate_settings(&request.settings).map_err(StoreError::InvalidRequestFields)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Write, LockMode::Update).await?;
        if let Some(prior) =
            postgres::settings_by_operation(&tx, &script, &actor, &operation).await?
        {
            if prior.request_bytes() != bytes {
                return Err(StoreError::OperationReused);
            }
            let result = prior.settings()?;
            tx.commit().await?;
            return Ok(result);
        }
        let version = checked_number(decide_settings_version(
            facts.head,
            facts.settings_version(),
            request.expected_revision,
            request.expected_settings_version,
        )?)?;
        postgres::insert_settings(
            &tx,
            postgres::NewSettings {
                script: &script,
                version,
                script_revision: checked_number(facts.head)?,
                operation: &operation,
                actor: &actor,
                request_bytes: &bytes,
                settings_bytes: &settings_bytes,
            },
        )
        .await?;
        let result = postgres::load_settings(&tx, &script, version).await?;
        production_actor(&tx, token).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// A script-scoped assertion, never a legal verdict or an owner-wide consent grant.
    pub async fn save_production_rights(
        &self,
        token: &str,
        script: &str,
        request: SaveProductionRightsRequest,
    ) -> Result<ProductionRightsClaimResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let operation = normalized_uuid(&request.operation_id)?;
        let bytes = encode(&request, 64 * 1024)?;
        validate_rights_declaration(&request.claim).map_err(StoreError::InvalidRequestFields)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Review, LockMode::Update).await?;
        if let Some(prior) = postgres::rights_by_operation(&tx, &script, &actor, &operation).await?
        {
            if prior.request_bytes() != bytes {
                return Err(StoreError::OperationReused);
            }
            let result = prior.rights()?;
            tx.commit().await?;
            return Ok(result);
        }
        let prior = facts
            .rights
            .iter()
            .find(|claim| claim.claim.declaration.record_id == request.claim.record_id)
            .map(|claim| (claim.claim.version, &claim.claim.declaration.subject));
        let version = decide_rights_version(
            prior,
            facts.rights.len(),
            &request.claim.subject,
            request.expected_version,
        )?;
        match &request.claim.subject {
            ProductionRightsSubject::Evidence { record_id } => {
                if !rights_cited_by_script(&tx, &facts.owner, record_id, &script).await? {
                    return Err(StoreError::EvidenceUnavailable);
                }
            }
            ProductionRightsSubject::Voice { .. } => {
                ensure_evidence(
                    &tx,
                    &facts.owner,
                    EvidenceKind::Rights,
                    &request.claim.record_id,
                    "Script-scoped recorded voice permission assertion; not legal verification or provider consent",
                )
                .await?;
            }
        }
        let claim = ProductionRightsClaim {
            version,
            declaration: request.claim,
        };
        let claim_bytes = encode(&claim, 64 * 1024)?;
        postgres::insert_rights(
            &tx,
            postgres::NewRights {
                script: &script,
                owner: &facts.owner,
                record_id: &claim.declaration.record_id,
                version: checked_number(version)?,
                operation: &operation,
                actor: &actor,
                request_bytes: &bytes,
                claim_bytes: &claim_bytes,
            },
        )
        .await?;
        let result = postgres::load_rights(
            &tx,
            &script,
            &claim.declaration.record_id,
            checked_number(version)?,
        )
        .await?;
        production_actor(&tx, token).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn production_preview(
        &self,
        token: &str,
        script: &str,
    ) -> Result<ProductionPreviewResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = production_actor(&tx, token).await?;
        let facts = facts(&tx, &actor, &script, Action::Read, LockMode::Share).await?;
        let settings = facts
            .settings
            .as_ref()
            .ok_or(StoreError::StaleProductionInputs)?;
        let revision = crate::contexts::editorial::document::revision(
            load_revision(&tx, &script, checked_number(facts.head)?).await?,
        );
        let prepared = prepare_inputs(
            &facts.owner,
            &revision,
            settings,
            &facts.rights,
            facts.reviewed,
            postgres::now(&tx).await?,
        )
        .map_err(StoreError::InvalidRequestFields)?;
        tx.commit().await?;
        Ok(prepared.preview())
    }
}
