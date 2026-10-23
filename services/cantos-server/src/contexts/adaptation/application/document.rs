//! Freeze safe domain refusals into the unchanged versioned submission receipt contract.
use crate::contract::studio::AdaptationProblem;

use super::super::domain::AdaptationRejection;
use crate::contexts::editorial::document::issues;

pub(super) fn problem(rejection: AdaptationRejection) -> AdaptationProblem {
    AdaptationProblem {
        code: rejection.code.into(),
        issues: issues(rejection.issues),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::editorial::domain::diagnostics::validation_issues;
    use crate::script_ir::ReadError;

    #[test]
    fn receipt_problem_keeps_the_wire_envelope_after_domain_redaction() {
        let projected = problem(AdaptationRejection {
            code: "invalid_script_output",
            issues: validation_issues(ReadError::InvalidDocument {
                line: 2,
                column: 3,
                detail: "private-sentinel".into(),
            }),
        });
        assert_eq!(
            serde_json::to_string(&projected).unwrap(),
            r#"{"code":"invalid_script_output","issues":[{"path":"$","rule":"InvalidDocument"}]}"#
        );
    }
}
