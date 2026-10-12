//! Pure adaptation rules: the exported request, caller generation declarations, the closed
//! proposal admission kernel and the trusted evidence bindings that acceptance preserves.
//! Durable I/O, identity minting and the clock belong to the application shell.
use cantos_contracts::{AdaptationProblem, Extraction, FieldIssue, ImportOutcome, ImportResponse};
use serde_json::Value;

use crate::contexts::editorial::domain::diagnostics::validation_issues;
use crate::script_ir::ReadError;

mod commands;
mod evidence;
mod frozen;
mod generation;
mod materialize;
mod proposal;
mod request;

pub use commands::{
    check_context_source, check_submission, decide_acceptance, decide_cancellation,
    AcceptanceDecision, CancellationDecision, SourceRejection, SubmissionRejection,
};
pub use evidence::{admit_edited_proposal, TrustedBinding};
pub use frozen::{
    generation_record_id, rights_record_id, typed_digest, FrozenAdaptation, FrozenContext,
    Observation, INPUT_A1_DIGEST, INPUT_C1_DIGEST, PROPOSAL_DIGEST, SUBMISSION_DIGEST,
};
pub use generation::{validate_caller_generation, MAX_CALLER_CONFIGURATION_BYTES};
pub use materialize::{admit_output_with_ids, admit_output_with_minter, MAX_FINDINGS};
pub use proposal::{MAX_NODES, MAX_OUTPUT_BYTES};
pub use request::{
    prepare_request, PreparedAdaptationContext, CONTRACT_VERSION, MAX_REQUEST_BYTES,
    MAX_SOURCE_CONTEXT_BYTES, OUTPUT_SCHEMA, PROMPT_VERSION,
};

pub(super) fn problem(code: &str, path: &str, rule: &str) -> AdaptationProblem {
    AdaptationProblem {
        code: code.into(),
        issues: vec![FieldIssue {
            path: path.into(),
            rule: rule.into(),
        }],
    }
}

pub(super) fn extracted(source: &ImportResponse) -> Result<&Extraction, AdaptationProblem> {
    match &source.outcome {
        ImportOutcome::Parsed { extraction } => Ok(extraction),
        ImportOutcome::Failed { .. } => {
            Err(problem("source_not_extracted", "/source_id", "parsed"))
        }
    }
}

pub(super) fn array(value: &Value) -> Result<&Vec<Value>, AdaptationProblem> {
    value
        .as_array()
        .ok_or_else(|| problem("invalid_base", "/", "array"))
}

pub(super) fn text(value: &Value) -> Result<&str, AdaptationProblem> {
    value
        .as_str()
        .ok_or_else(|| problem("invalid_output", "/", "string"))
}

pub(super) fn script_problem(error: ReadError) -> AdaptationProblem {
    AdaptationProblem {
        code: "invalid_script_output".into(),
        issues: validation_issues(error),
    }
}
