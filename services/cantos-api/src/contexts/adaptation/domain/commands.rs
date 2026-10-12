//! Pure decisions for adaptation commands. The shell loads the facts under the run lock, asks
//! here, then writes exactly what the decision says.
use cantos_contracts::{
    AcceptAdaptationRequest, AdaptationContextRequest, AdaptationStatus, FieldIssue, ImportOutcome,
    ImportResponse,
};

fn issue(path: &str, rule: &str) -> FieldIssue {
    FieldIssue {
        path: path.into(),
        rule: rule.into(),
    }
}

/// Why a pinned source cannot seed a caller context.
#[derive(Debug, PartialEq, Eq)]
pub enum SourceRejection {
    /// The import has no parsed extraction to adapt.
    NotExtracted,
    /// The request names another source identity, or a required rights claim is absent.
    Issues(Vec<FieldIssue>),
}

/// The request must pin the exact source checksum and extractor, and the import must carry the
/// creator's rights holder, permission evidence and usage scope. Every problem is reported.
pub fn check_context_source(
    request: &AdaptationContextRequest,
    source: &ImportResponse,
) -> Result<(), SourceRejection> {
    let extractor = match &source.outcome {
        ImportOutcome::Parsed { extraction } => &extraction.extractor_version,
        ImportOutcome::Failed { .. } => return Err(SourceRejection::NotExtracted),
    };
    let mut issues = Vec::new();
    if request.source_sha256 != source.sha256 {
        issues.push(issue("/source_sha256", "source_identity_mismatch"));
    }
    if &request.extractor_version != extractor {
        issues.push(issue("/extractor_version", "extraction_identity_mismatch"));
    }
    for (value, path) in [
        (&source.metadata.rights_holder, "/source/rights_holder"),
        (
            &source.metadata.permission_evidence,
            "/source/permission_evidence",
        ),
        (&source.metadata.usage_scope, "/source/usage_scope"),
    ] {
        if value.as_deref().is_none_or(|value| value.trim().is_empty()) {
            issues.push(issue(path, "required_claim"));
        }
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(SourceRejection::Issues(issues))
    }
}

/// Why a submission is refused before its proposal is admitted.
#[derive(Debug, PartialEq, Eq)]
pub enum SubmissionRejection {
    Issues(Vec<FieldIssue>),
    AlreadySubmitted,
}

/// A submission must name the immutable context it answers and arrive while that context still
/// awaits a proposal. Generation declaration issues come from the caller-generation validator.
pub fn check_submission(
    context_digest: &str,
    submitted_digest: &str,
    generation_issues: Vec<FieldIssue>,
    status: AdaptationStatus,
) -> Result<(), SubmissionRejection> {
    if context_digest != submitted_digest {
        return Err(SubmissionRejection::Issues(vec![issue(
            "/context_digest",
            "immutable_context",
        )]));
    }
    if !generation_issues.is_empty() {
        return Err(SubmissionRejection::Issues(generation_issues));
    }
    if status != AdaptationStatus::AwaitingProposal {
        return Err(SubmissionRejection::AlreadySubmitted);
    }
    Ok(())
}

/// What an acceptance request does, given any acceptance already recorded for its operation.
#[derive(Debug, PartialEq, Eq)]
pub enum AcceptanceDecision {
    /// The same operation already accepted this run with this exact request.
    Replay,
    /// The operation identity was used for another run or another request.
    OperationReused,
    /// The run has no admitted proposal, or the request names another base revision.
    Refused,
    /// Save the edited proposal as the next revision and record the acceptance.
    Accept,
}

pub fn decide_acceptance(
    run: &str,
    request: &AcceptAdaptationRequest,
    prior: Option<(&str, &AcceptAdaptationRequest)>,
    status: AdaptationStatus,
    expected_revision: u64,
) -> AcceptanceDecision {
    if let Some((prior_run, prior_request)) = prior {
        return if prior_run != run || prior_request != request {
            AcceptanceDecision::OperationReused
        } else {
            AcceptanceDecision::Replay
        };
    }
    if status != AdaptationStatus::Succeeded || request.expected_revision != expected_revision {
        return AcceptanceDecision::Refused;
    }
    AcceptanceDecision::Accept
}

/// What a cancellation request does, given the run any prior use of its operation cancelled.
#[derive(Debug, PartialEq, Eq)]
pub enum CancellationDecision {
    /// The same operation already cancelled this run.
    Replay,
    /// The operation identity already cancelled another run.
    OperationReused,
    /// An accepted run cannot be cancelled.
    Refused,
    /// Record the cancellation; `transition` is false when the run is already cancelled.
    Cancel { transition: bool },
}

pub fn decide_cancellation(
    run: &str,
    prior_run: Option<&str>,
    status: AdaptationStatus,
) -> CancellationDecision {
    match prior_run {
        Some(prior) if prior != run => CancellationDecision::OperationReused,
        Some(_) => CancellationDecision::Replay,
        None if status == AdaptationStatus::Accepted => CancellationDecision::Refused,
        None => CancellationDecision::Cancel {
            transition: status != AdaptationStatus::Cancelled,
        },
    }
}

#[cfg(test)]
mod tests {
    use cantos_contracts::{
        AcceptAdaptationRequest, AdaptationStatus, Extraction, ImportFormat, ImportMetadata,
        ImportProblem,
    };

    use super::*;

    const ALL_STATUSES: [AdaptationStatus; 9] = [
        AdaptationStatus::AwaitingProposal,
        AdaptationStatus::Queued,
        AdaptationStatus::Running,
        AdaptationStatus::Succeeded,
        AdaptationStatus::InvalidOutput,
        AdaptationStatus::Failed,
        AdaptationStatus::Ambiguous,
        AdaptationStatus::Cancelled,
        AdaptationStatus::Accepted,
    ];

    fn accept(revision: u64) -> AcceptAdaptationRequest {
        AcceptAdaptationRequest {
            operation_id: "11111111-1111-4111-8111-111111111111".into(),
            expected_revision: revision,
            script_json: "{}".into(),
            reviewed_findings: true,
        }
    }

    #[test]
    fn a_recorded_acceptance_replays_only_for_the_same_run_and_request() {
        let request = accept(1);
        for status in ALL_STATUSES {
            assert_eq!(
                decide_acceptance("run-a", &request, Some(("run-a", &request)), status, 9),
                AcceptanceDecision::Replay
            );
            assert_eq!(
                decide_acceptance("run-a", &request, Some(("run-b", &request)), status, 1),
                AcceptanceDecision::OperationReused
            );
            assert_eq!(
                decide_acceptance("run-a", &request, Some(("run-a", &accept(2))), status, 1),
                AcceptanceDecision::OperationReused
            );
        }
    }

    #[test]
    fn a_new_acceptance_needs_an_admitted_proposal_on_the_pinned_base() {
        let request = accept(1);
        for status in ALL_STATUSES {
            let expected = if status == AdaptationStatus::Succeeded {
                AcceptanceDecision::Accept
            } else {
                AcceptanceDecision::Refused
            };
            assert_eq!(
                decide_acceptance("run", &request, None, status, 1),
                expected
            );
        }
        assert_eq!(
            decide_acceptance("run", &request, None, AdaptationStatus::Succeeded, 2),
            AcceptanceDecision::Refused
        );
    }

    #[test]
    fn cancellation_is_idempotent_per_operation_and_never_undoes_acceptance() {
        for status in ALL_STATUSES {
            assert_eq!(
                decide_cancellation("run", Some("run"), status),
                CancellationDecision::Replay
            );
            assert_eq!(
                decide_cancellation("run", Some("other"), status),
                CancellationDecision::OperationReused
            );
            let expected = match status {
                AdaptationStatus::Accepted => CancellationDecision::Refused,
                AdaptationStatus::Cancelled => CancellationDecision::Cancel { transition: false },
                _ => CancellationDecision::Cancel { transition: true },
            };
            assert_eq!(
                decide_cancellation("run", None, status),
                expected,
                "{status:?}"
            );
        }
    }

    #[test]
    fn a_submission_checks_context_then_generation_then_status() {
        let generation = vec![issue("/generation/model", "required")];
        assert_eq!(
            check_submission("a", "b", generation.clone(), AdaptationStatus::Succeeded),
            Err(SubmissionRejection::Issues(vec![issue(
                "/context_digest",
                "immutable_context"
            )]))
        );
        assert_eq!(
            check_submission("a", "a", generation.clone(), AdaptationStatus::Succeeded),
            Err(SubmissionRejection::Issues(generation))
        );
        for status in ALL_STATUSES {
            let expected = if status == AdaptationStatus::AwaitingProposal {
                Ok(())
            } else {
                Err(SubmissionRejection::AlreadySubmitted)
            };
            assert_eq!(check_submission("a", "a", vec![], status), expected);
        }
    }

    fn source(outcome: ImportOutcome, claims: Option<&str>) -> ImportResponse {
        ImportResponse {
            id: "src_synthetic".into(),
            imported_by: "alice".into(),
            recorded_at: "2026-10-10T00:00:00Z".into(),
            sha256: "a".repeat(64),
            byte_len: 10,
            metadata: ImportMetadata {
                operation_id: "22222222-2222-4222-8222-222222222222".into(),
                file_name: "chuong-mot.txt".into(),
                format: ImportFormat::Txt,
                reference: "Chương một".into(),
                rights_holder: claims.map(str::to_owned),
                permission_evidence: claims.map(str::to_owned),
                usage_scope: claims.map(str::to_owned),
            },
            outcome,
            original_text: None,
        }
    }

    fn parsed() -> ImportOutcome {
        ImportOutcome::Parsed {
            extraction: Extraction {
                extractor_version: "cantos-import-1".into(),
                blocks: vec![],
                warnings: vec![],
                script_json: None,
            },
        }
    }

    fn request(sha256: &str, extractor: &str) -> AdaptationContextRequest {
        AdaptationContextRequest {
            operation_id: "33333333-3333-4333-8333-333333333333".into(),
            source_id: "src_synthetic".into(),
            source_sha256: sha256.into(),
            extractor_version: extractor.into(),
            script_id: "44444444-4444-4444-8444-444444444444".into(),
            expected_revision: 0,
            rights_authorization: true,
        }
    }

    #[test]
    fn a_context_source_must_be_parsed_pinned_and_rights_declared() {
        let pinned = request(&"a".repeat(64), "cantos-import-1");
        assert_eq!(
            check_context_source(&pinned, &source(parsed(), Some("Tác giả"))),
            Ok(())
        );
        let failed = ImportOutcome::Failed {
            error: ImportProblem {
                code: "invalid_utf8".into(),
                offset: Some(0),
            },
        };
        assert_eq!(
            check_context_source(&pinned, &source(failed, Some("Tác giả"))),
            Err(SourceRejection::NotExtracted)
        );
        assert_eq!(
            check_context_source(&request("b", "other"), &source(parsed(), Some(" "))),
            Err(SourceRejection::Issues(vec![
                issue("/source_sha256", "source_identity_mismatch"),
                issue("/extractor_version", "extraction_identity_mismatch"),
                issue("/source/rights_holder", "required_claim"),
                issue("/source/permission_evidence", "required_claim"),
                issue("/source/usage_scope", "required_claim"),
            ]))
        );
        assert_eq!(
            check_context_source(&pinned, &source(parsed(), None)),
            Err(SourceRejection::Issues(vec![
                issue("/source/rights_holder", "required_claim"),
                issue("/source/permission_evidence", "required_claim"),
                issue("/source/usage_scope", "required_claim"),
            ]))
        );
    }
}
