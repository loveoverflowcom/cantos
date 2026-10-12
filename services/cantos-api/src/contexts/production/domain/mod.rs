//! Pure production rules: casting resolution, rights claims, planning estimates, frozen input
//! identity, snapshot eligibility and the decisions behind each production command.
//! No HTTP, database, clock or provider dependency; nothing here sends a request.
use cantos_contracts::{FieldIssue, ProductionFinding, ProductionFindingCode};
use unicode_normalization::UnicodeNormalization;

mod casting;
pub mod catalog;
mod commands;
mod eligibility;
mod estimate;
mod frozen;
mod rights;
mod settings;

pub use casting::{MAX_EFFECTIVE_LINE_BYTES, MAX_RESOLVED_BYTES};
pub use catalog::{production_catalog, ADAPTER_VERSION, CATALOG_VERSION, OUTPUT_CONTRACT};
pub use commands::*;
pub use eligibility::{
    authorize_inputs, evaluate_for_owner, evaluate_snapshot, AuthorizedProductionInputs,
};
pub use estimate::{estimate_cost, estimate_dialogues, MAX_MONEY_MINOR};
pub use frozen::{
    admit_frozen_document, prepare_inputs, production_input_digest, PreparedProduction,
    MAX_PRODUCTION_DOCUMENT_BYTES,
};
pub use rights::validate_rights_declaration;
pub use settings::{validate_settings, MAX_SETTINGS_BYTES};

pub(super) fn issue(issues: &mut Vec<FieldIssue>, path: impl Into<String>, rule: &str) {
    issues.push(FieldIssue {
        path: path.into(),
        rule: rule.into(),
    });
}

pub(super) fn valid_text(text: &str, max: usize) -> bool {
    !text.trim().is_empty()
        && text.len() <= max
        && !text.chars().any(char::is_control)
        && text.nfc().eq(text.chars())
}

pub(super) fn valid_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 128
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}

pub(super) fn valid_registry_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}

pub(super) fn finding(
    findings: &mut Vec<ProductionFinding>,
    code: ProductionFindingCode,
    path: impl Into<String>,
    detail: &str,
) {
    findings.push(ProductionFinding {
        code,
        path: path.into(),
        detail: detail.into(),
    });
}
