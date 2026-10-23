//! Rights claims: recorded declarations for source evidence and voices, the records a script
//! and its casting require, and the findings that block planning until each claim covers them.
use std::collections::{BTreeMap, BTreeSet};

use crate::contract::studio::{
    FieldIssue, ProductionBudget, ProductionFinding, ProductionFindingCode,
    ProductionRightsClaimResponse, ProductionRightsDeclaration, ProductionRightsStatus,
    ProductionRightsSubject, ProductionSettings,
};

use super::catalog::{production_catalog, LANGUAGES};
use super::{finding, issue, valid_registry_id, valid_text};
use crate::script_ir::ScriptContent;

/// Rights are recorded declarations, not a legal verification witness.
pub fn validate_rights_declaration(
    claim: &ProductionRightsDeclaration,
) -> Result<(), Vec<FieldIssue>> {
    let mut issues = Vec::new();
    if !valid_registry_id(&claim.record_id) {
        issue(&mut issues, "claim.record_id", "id_format");
    }
    match &claim.subject {
        ProductionRightsSubject::Evidence { record_id } => {
            if record_id != &claim.record_id {
                issue(
                    &mut issues,
                    "claim.subject.record_id",
                    "rights_record_mismatch",
                );
            }
        }
        ProductionRightsSubject::Voice {
            provider_id,
            model_id,
            voice_id,
        } => {
            if !production_catalog().capabilities.iter().any(|capability| {
                capability.provider_id == *provider_id
                    && capability.model_id == *model_id
                    && capability.voice_ids.contains(voice_id)
            }) {
                issue(&mut issues, "claim.subject", "unsupported_reference_voice");
            }
        }
    }
    for (field, value, max) in [
        ("reference", claim.reference.as_str(), 512),
        ("rights_holder", claim.rights_holder.as_str(), 512),
        ("attribution", claim.attribution.as_str(), 2_048),
        ("restrictions", claim.restrictions.as_str(), 2_048),
        ("permitted_scope", claim.permitted_scope.as_str(), 512),
    ] {
        if !valid_text(value, max) {
            issue(
                &mut issues,
                format!("claim.{field}"),
                "text_length_or_normalization",
            );
        }
    }
    let mut languages = BTreeSet::new();
    if claim.languages.is_empty() || claim.languages.len() > 4 {
        issue(&mut issues, "claim.languages", "array_length");
    }
    for language in &claim.languages {
        if !(language == "all" || LANGUAGES.contains(&language.as_str()))
            || !languages.insert(language)
        {
            issue(
                &mut issues,
                "claim.languages",
                "unsupported_or_duplicate_language",
            );
        }
    }
    if languages.iter().any(|language| language.as_str() == "all") && languages.len() != 1 {
        issue(
            &mut issues,
            "claim.languages",
            "all_must_be_explicit_and_exclusive",
        );
    }
    if claim.territory != "private-planning" {
        issue(
            &mut issues,
            "claim.territory",
            "unsupported_private_planning_territory",
        );
    }
    if claim.valid_from_unix < 0
        || claim
            .valid_until_unix
            .is_some_and(|until| until <= claim.valid_from_unix)
    {
        issue(&mut issues, "claim.valid_until_unix", "invalid_term");
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

pub(super) type RequiredRights = BTreeMap<String, ProductionRightsSubject>;

pub(super) fn required_rights(
    script: &ScriptContent,
    settings: &ProductionSettings,
) -> Result<RequiredRights, Vec<FieldIssue>> {
    let mut required = BTreeMap::new();
    for (kind, id) in script.evidence_refs() {
        if kind == "rights" {
            required.insert(
                id.into(),
                ProductionRightsSubject::Evidence {
                    record_id: id.into(),
                },
            );
        }
    }
    let mut issues = Vec::new();
    for binding in &settings.bindings {
        let subject = ProductionRightsSubject::Voice {
            provider_id: binding.provider_id.clone(),
            model_id: binding.model_id.clone(),
            voice_id: binding.voice_id.clone(),
        };
        if required
            .get(&binding.voice_rights_record_id)
            .is_some_and(|prior| prior != &subject)
        {
            issue(
                &mut issues,
                format!("bindings.{}.voice_rights_record_id", binding.character_id),
                "rights_subject_conflict",
            );
        }
        required.insert(binding.voice_rights_record_id.clone(), subject);
    }
    if issues.is_empty() {
        Ok(required)
    } else {
        Err(issues)
    }
}

pub(super) fn rights_findings(
    required: &RequiredRights,
    rights: &[ProductionRightsClaimResponse],
    language: &str,
    budget: &ProductionBudget,
    now_unix: i64,
) -> Vec<ProductionFinding> {
    let mut findings = Vec::new();
    for (id, subject) in required {
        let path = format!("rights.{id}");
        let Some(response) = rights
            .iter()
            .find(|right| &right.claim.declaration.record_id == id)
        else {
            finding(
                &mut findings,
                ProductionFindingCode::RightsMissing,
                path,
                "No persisted production claim covers this required rights record.",
            );
            continue;
        };
        let claim = &response.claim.declaration;
        if &claim.subject != subject {
            finding(
                &mut findings,
                ProductionFindingCode::RightsSubjectMismatch,
                &path,
                "The claim covers another source or voice subject.",
            );
        }
        match claim.status {
            ProductionRightsStatus::Pending => finding(
                &mut findings,
                ProductionFindingCode::RightsPending,
                &path,
                "The recorded claim is pending.",
            ),
            ProductionRightsStatus::Revoked => finding(
                &mut findings,
                ProductionFindingCode::RightsRevoked,
                &path,
                "The recorded claim is revoked.",
            ),
            ProductionRightsStatus::Granted => {}
        }
        if now_unix < claim.valid_from_unix {
            finding(
                &mut findings,
                ProductionFindingCode::RightsNotYetValid,
                &path,
                "The recorded term has not begun.",
            );
        }
        if claim
            .valid_until_unix
            .is_some_and(|until| now_unix >= until)
        {
            finding(
                &mut findings,
                ProductionFindingCode::RightsExpired,
                &path,
                "The recorded term has expired.",
            );
        }
        if !claim
            .languages
            .iter()
            .any(|tag| tag == language || tag == "all")
        {
            finding(
                &mut findings,
                ProductionFindingCode::RightsLanguageMismatch,
                &path,
                "The recorded claim does not cover the exact script language.",
            );
        }
        if claim.territory != budget.territory {
            finding(
                &mut findings,
                ProductionFindingCode::RightsTerritoryMismatch,
                &path,
                "The recorded claim does not cover the planning territory.",
            );
        }
        if claim.permitted_scope != budget.scope {
            finding(
                &mut findings,
                ProductionFindingCode::RightsScopeMismatch,
                &path,
                "The recorded claim does not cover the exact production budget scope.",
            );
        }
        if !["none-declared", "private-planning-only"].contains(&claim.restrictions.as_str()) {
            finding(&mut findings, ProductionFindingCode::RightsRestrictionsRequireReview, &path, "This restriction is not supported by the bounded private planning policy; review is required.");
        }
    }
    findings
}
