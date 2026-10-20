//! Current eligibility of a frozen snapshot. Staleness is derived from current facts; the
//! immutable snapshot and approval rows never change.
use cantos_contracts::{
    FrozenProductionDocument, ProductionApprovalResponse, ProductionEligibility, ProductionFinding,
    ProductionFindingCode, ProductionRightsClaimResponse,
};

use super::catalog::google as google_contract;
use super::finding;
use super::frozen::{admit_frozen_document, prepare_inputs};

/// Staleness is derived from current facts; immutable snapshots/approval rows never mutate.
pub fn evaluate_snapshot(
    document: &FrozenProductionDocument,
    current_revision: u64,
    current_settings_version: u64,
    current_rights: &[ProductionRightsClaimResponse],
    script_reviewed: bool,
    approval: Option<&ProductionApprovalResponse>,
    now_unix: i64,
) -> ProductionEligibility {
    let prepared = prepare_inputs(
        &document.owner_id,
        &document.revision,
        &document.settings,
        current_rights,
        script_reviewed,
        now_unix,
    );
    let mut findings = match &prepared {
        Ok(candidate) => candidate.findings.clone(),
        Err(_) => vec![ProductionFinding {
            code: ProductionFindingCode::RightsChanged,
            path: "document".into(),
            detail: "Current input facts cannot be admitted under this frozen scope.".into(),
        }],
    };
    if current_revision != document.revision.revision {
        finding(
            &mut findings,
            ProductionFindingCode::RevisionChanged,
            "revision",
            "A newer accepted script revision requires a new candidate and approval.",
        );
    }
    if current_settings_version != document.settings.version {
        finding(
            &mut findings,
            ProductionFindingCode::SettingsChanged,
            "settings.version",
            "Casting or budget settings changed; approval must be reevaluated.",
        );
    }
    if prepared
        .as_ref()
        .is_ok_and(|candidate| candidate.document.rights != document.rights)
    {
        finding(
            &mut findings,
            ProductionFindingCode::RightsChanged,
            "rights",
            "A required rights claim revision changed; approval must be reevaluated.",
        );
    }
    let pinned = admit_frozen_document(
        document,
        &document.revision,
        &document.settings,
        &document.rights,
    );
    if pinned.is_err() {
        finding(
            &mut findings,
            ProductionFindingCode::ApprovalScopeMismatch,
            "document.input_digest",
            "The frozen document differs from the admitted exact input identity.",
        );
    }
    if document.resolved.iter().any(|line| {
        line.provider_id == google_contract::PROVIDER_ID
            && line.adapter_version != google_contract::INSTRUCTION_VERSION
    }) {
        finding(
            &mut findings,
            ProductionFindingCode::ApprovalScopeMismatch,
            "resolved.adapter_version",
            "The frozen adapter is outdated; create and approve a new candidate for current execution.",
        );
    }
    let inputs_eligible = findings.is_empty();
    let approval_matches = approval.is_some_and(|approval| {
        approval.input_digest == document.input_digest && approval.approved_by == document.owner_id
    });
    match approval {
        None => finding(
            &mut findings,
            ProductionFindingCode::ApprovalMissing,
            "approval",
            "No current owner input/cost approval is recorded for this snapshot.",
        ),
        Some(_) if !approval_matches => finding(
            &mut findings,
            ProductionFindingCode::ApprovalScopeMismatch,
            "approval.input_digest",
            "The approval does not cover this exact input scope and owner.",
        ),
        Some(_) => {}
    }
    ProductionEligibility {
        inputs_eligible,
        approval_current: inputs_eligible && approval_matches,
        billable_dispatch_available: false,
        findings,
    }
}

/// Eligibility as the script's current owner sees it. A snapshot frozen for a previous owner,
/// or an approval recorded by anyone but the current owner, never authorizes use.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_for_owner(
    document: &FrozenProductionDocument,
    current_owner: &str,
    current_revision: u64,
    current_settings_version: u64,
    current_rights: &[ProductionRightsClaimResponse],
    script_reviewed: bool,
    approval: Option<&ProductionApprovalResponse>,
    now_unix: i64,
) -> ProductionEligibility {
    let approver_current = approval.is_some_and(|approval| approval.approved_by == current_owner);
    let mut eligibility = evaluate_snapshot(
        document,
        current_revision,
        current_settings_version,
        current_rights,
        script_reviewed,
        approval.filter(|_| approver_current),
        now_unix,
    );
    if document.owner_id != current_owner || (approval.is_some() && !approver_current) {
        eligibility.inputs_eligible = false;
        eligibility.approval_current = false;
        eligibility.findings.push(ProductionFinding {
            code: ProductionFindingCode::ApprovalScopeMismatch,
            path: "/approval".into(),
            detail: "The current owner no longer holds the recorded approval scope".into(),
        });
    }
    eligibility
}

/// A sealed decision for the supplied current-owner facts at one clock instant.
/// It is not a lasting capability: any future consumer must reload and recheck the owner,
/// rights, review, head, settings and time under its transaction fence. No dispatcher exists.
///
/// ```compile_fail
/// use cantos_server::contexts::production::domain::AuthorizedProductionInputs;
/// let forged = AuthorizedProductionInputs { input_digest: String::new() };
/// ```
/// ```compile_fail
/// use cantos_server::contexts::production::domain::AuthorizedProductionInputs;
/// fn deserialize<T: serde::de::DeserializeOwned>() {}
/// deserialize::<AuthorizedProductionInputs>();
/// ```
pub struct AuthorizedProductionInputs {
    input_digest: String,
}

impl AuthorizedProductionInputs {
    pub fn input_digest(&self) -> &str {
        &self.input_digest
    }
}

#[allow(clippy::too_many_arguments)]
pub fn authorize_inputs(
    document: &FrozenProductionDocument,
    current_owner: &str,
    current_revision: u64,
    current_settings_version: u64,
    current_rights: &[ProductionRightsClaimResponse],
    script_reviewed: bool,
    approval: Option<&ProductionApprovalResponse>,
    now_unix: i64,
) -> Result<AuthorizedProductionInputs, Vec<ProductionFinding>> {
    let eligibility = evaluate_for_owner(
        document,
        current_owner,
        current_revision,
        current_settings_version,
        current_rights,
        script_reviewed,
        approval,
        now_unix,
    );
    if eligibility.approval_current {
        Ok(AuthorizedProductionInputs {
            input_digest: document.input_digest.clone(),
        })
    } else {
        Err(eligibility.findings)
    }
}
