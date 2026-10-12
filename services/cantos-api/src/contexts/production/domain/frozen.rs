//! Frozen production inputs: the sealed candidate built from exact facts, its tagged input
//! digest and the admission of immutable history against the rows it pinned.
use std::collections::BTreeSet;

use cantos_contracts::{
    FieldIssue, FrozenProductionDocument, ProductionEstimate, ProductionFinding,
    ProductionFindingCode, ProductionPreviewResponse, ProductionRightsClaimResponse,
    ProductionSettingsResponse, RevisionResponse,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::casting::resolve_inputs;
use super::catalog::{google as google_contract, CATALOG_VERSION};
use super::estimate::{estimate_dialogues, MAX_MONEY_MINOR};
use super::rights::{required_rights, rights_findings, validate_rights_declaration};
use super::settings::validate_settings;
use super::{finding, issue, valid_registry_id};
use crate::contexts::editorial::domain::export_digest;
use crate::script_ir::read_canonical_script;

pub const MAX_PRODUCTION_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;

/// Sealed validated candidate: constructed only by deterministic admission and resolution.
pub struct PreparedProduction {
    pub(super) document: FrozenProductionDocument,
    pub(super) findings: Vec<ProductionFinding>,
}

impl PreparedProduction {
    pub fn document(&self) -> FrozenProductionDocument {
        self.document.clone()
    }

    pub fn preview(&self) -> ProductionPreviewResponse {
        ProductionPreviewResponse {
            input_digest: self.document.input_digest.clone(),
            script_revision: self.document.revision.revision,
            settings_version: self.document.settings.version,
            resolved: self.document.resolved.clone(),
            estimate: self.document.estimate.clone(),
            inputs_eligible: self.findings.is_empty(),
            findings: self.findings.clone(),
            billable_dispatch_available: false,
        }
    }
}

/// The shell supplies current persisted claims, exact editorial review and one clock instant.
pub fn prepare_inputs(
    owner_id: &str,
    revision: &RevisionResponse,
    settings: &ProductionSettingsResponse,
    rights: &[ProductionRightsClaimResponse],
    script_reviewed: bool,
    now_unix: i64,
) -> Result<PreparedProduction, Vec<FieldIssue>> {
    validate_settings(&settings.settings)?;
    let mut issues = Vec::new();
    if !valid_registry_id(owner_id) {
        issue(&mut issues, "owner_id", "id_format");
    }
    if revision.revision == 0
        || revision.revision > MAX_MONEY_MINOR
        || settings.version == 0
        || settings.version > MAX_MONEY_MINOR
        || revision.script_id != settings.script_id
        || revision.revision != settings.script_revision
    {
        issue(
            &mut issues,
            "settings.script_revision",
            "revision_scope_mismatch",
        );
    }
    let script = read_canonical_script(revision.script_json.as_bytes()).map_err(|_| {
        vec![FieldIssue {
            path: "revision.script_json".into(),
            rule: "invalid_canonical_export".into(),
        }]
    })?;
    if script.content_digest().to_string() != revision.content_digest
        || export_digest(revision.script_json.as_bytes()) != revision.export_digest
    {
        issue(&mut issues, "revision", "digest_mismatch");
    }
    let required = required_rights(&script, &settings.settings)?;
    let mut rights: Vec<_> = rights
        .iter()
        .filter(|right| required.contains_key(&right.claim.declaration.record_id))
        .cloned()
        .collect();
    rights.sort_by(|a, b| {
        a.claim
            .declaration
            .record_id
            .cmp(&b.claim.declaration.record_id)
    });
    let mut seen = BTreeSet::new();
    for right in &rights {
        validate_rights_declaration(&right.claim.declaration)?;
        if right.claim.version == 0
            || right.claim.version > MAX_MONEY_MINOR
            || !seen.insert(&right.claim.declaration.record_id)
        {
            issue(&mut issues, "rights", "invalid_or_duplicate_current_claim");
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    let resolved = resolve_inputs(&script, &settings.settings)?;
    let mut settings = settings.clone();
    settings
        .settings
        .bindings
        .sort_by(|a, b| a.character_id.cmp(&b.character_id));
    for binding in &mut settings.settings.bindings {
        binding
            .pronunciation
            .sort_by(|a, b| a.surface.cmp(&b.surface));
    }
    let language = resolved
        .first()
        .map(|line| line.language.as_str())
        .unwrap_or("vi");
    let mut findings = rights_findings(
        &required,
        &rights,
        language,
        &settings.settings.budget,
        now_unix,
    );
    if !script_reviewed {
        finding(
            &mut findings,
            ProductionFindingCode::ScriptUnreviewed,
            "revision",
            "The accepted script revision has no exact owner editorial review.",
        );
    }
    let estimate = estimate_dialogues(&resolved, &settings.settings.budget);
    match &estimate {
        ProductionEstimate::Unavailable { .. } => finding(
            &mut findings,
            ProductionFindingCode::EstimateUnavailable,
            "budget.rate",
            "No bounded planning estimate is available; absence is not zero.",
        ),
        ProductionEstimate::Known { amount_minor, .. }
            if *amount_minor > settings.settings.budget.limit_minor =>
        {
            finding(
                &mut findings,
                ProductionFindingCode::BudgetExceeded,
                "budget.limit_minor",
                "The upward-rounded planning estimate exceeds the recorded limit.",
            )
        }
        ProductionEstimate::Known { .. } => {}
    }
    let mut document = FrozenProductionDocument {
        owner_id: owner_id.into(),
        catalog_version: CATALOG_VERSION.into(),
        revision: revision.clone(),
        settings,
        rights,
        resolved,
        estimate,
        input_digest: String::new(),
    };
    document.input_digest = production_input_digest(&document)?;
    if serde_json::to_vec(&document)
        .map_or(true, |bytes| bytes.len() > MAX_PRODUCTION_DOCUMENT_BYTES)
    {
        return Err(vec![FieldIssue {
            path: "document".into(),
            rule: "production_document_too_large".into(),
        }]);
    }
    Ok(PreparedProduction { document, findings })
}

/// Admit immutable history against exact pinned facts, without authorizing its current use.
/// Google v2 and v3 resolve the same effective inputs and output contract; v3 hardens the
/// execution boundary. Only the old adapter identity may differ from current reconstruction.
/// Recomputing and comparing the whole document refuses forged resolved fields and digests.
/// The caller may supply settled database rows instead of trusting the embedded receipts.
pub fn admit_frozen_document(
    document: &FrozenProductionDocument,
    pinned_revision: &RevisionResponse,
    pinned_settings: &ProductionSettingsResponse,
    pinned_rights: &[ProductionRightsClaimResponse],
) -> Result<(), Vec<FieldIssue>> {
    if serde_json::to_vec(document)
        .map_or(true, |bytes| bytes.len() > MAX_PRODUCTION_DOCUMENT_BYTES)
    {
        return Err(vec![FieldIssue {
            path: "document".into(),
            rule: "production_document_too_large".into(),
        }]);
    }
    let prepared = prepare_inputs(
        &document.owner_id,
        pinned_revision,
        pinned_settings,
        pinned_rights,
        true,
        0,
    )?;
    let mut reconstructed = prepared.document;
    let historical_google = document.resolved.iter().any(|line| {
        line.provider_id == google_contract::PROVIDER_ID
            && line.adapter_version == google_contract::HISTORICAL_ADAPTER_VERSION
    });
    if historical_google {
        // A freeze used one adapter version for every Google line. Mixed historical/current
        // identities fail the complete comparison below instead of being normalized.
        for line in &mut reconstructed.resolved {
            if line.provider_id == google_contract::PROVIDER_ID {
                line.adapter_version = google_contract::HISTORICAL_ADAPTER_VERSION.into();
            }
        }
        reconstructed.input_digest = production_input_digest(&reconstructed)?;
    }
    if reconstructed != *document {
        return Err(vec![FieldIssue {
            path: "document.input_digest".into(),
            rule: "frozen_document_identity_mismatch".into(),
        }]);
    }
    Ok(())
}

/// Recursive lexicographic compact UTF-8 JSON, excluding only the digest field itself.
pub fn production_input_digest(
    document: &FrozenProductionDocument,
) -> Result<String, Vec<FieldIssue>> {
    let mut value = serde_json::to_value(document).map_err(|_| {
        vec![FieldIssue {
            path: "document".into(),
            rule: "serialization_failed".into(),
        }]
    })?;
    if let Value::Object(map) = &mut value {
        map.remove("input_digest");
    }
    let mut bytes = Vec::new();
    canonical_json(&value, &mut bytes).map_err(|_| {
        vec![FieldIssue {
            path: "document".into(),
            rule: "serialization_failed".into(),
        }]
    })?;
    let mut hash = Sha256::new();
    hash.update(b"cantos/production-inputs/p1\n");
    hash.update(bytes);
    Ok(format!("production-p1:sha256:{:x}", hash.finalize()))
}

fn canonical_json(value: &Value, bytes: &mut Vec<u8>) -> Result<(), serde_json::Error> {
    match value {
        Value::Object(map) => {
            bytes.push(b'{');
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    bytes.push(b',');
                }
                bytes.extend(serde_json::to_vec(key)?);
                bytes.push(b':');
                canonical_json(&map[key], bytes)?;
            }
            bytes.push(b'}');
        }
        Value::Array(values) => {
            bytes.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    bytes.push(b',');
                }
                canonical_json(value, bytes)?;
            }
            bytes.push(b']');
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            bytes.extend(serde_json::to_vec(value)?)
        }
    }
    Ok(())
}
