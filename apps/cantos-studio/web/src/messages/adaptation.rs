use crate::adaptation::ReviewStatus;
use crate::contract::{
    AdaptationCost, AdaptationCurrency, AdaptationFindingCode, AdaptationStatus,
    AdaptationSubmissionStatus, AdaptationUsage,
};
use crate::localization::generated::adaptation as resources;

#[derive(Clone, Copy)]
pub struct AdaptationCopy {
    pub title: &'static str,
    pub help: &'static str,
    pub provider: &'static str,
    pub model: &'static str,
    pub prompt: &'static str,
    pub config: &'static str,
    pub source_id: &'static str,
    pub script_id: &'static str,
    pub base: &'static str,
    pub pending: &'static str,
    pub retry: &'static str,
    pub operation: &'static str,
    pub run_id: &'static str,
    pub open: &'static str,
    pub refresh: &'static str,
    pub new_run: &'static str,
    pub new_help: &'static str,
    pub provenance: &'static str,
    pub checksum: &'static str,
    pub extraction: &'static str,
    pub records: &'static str,
    pub cost: &'static str,
    pub usage: &'static str,
    pub unknown: &'static str,
    pub original: &'static str,
    pub source_missing: &'static str,
    pub original_binary: &'static str,
    pub download: &'static str,
    pub proposal: &'static str,
    pub proposal_help: &'static str,
    pub edit: &'static str,
    pub findings: &'static str,
    pub coverage: &'static str,
    pub coverage_help: &'static str,
    pub represented: &'static str,
    pub omitted: &'static str,
    pub block: &'static str,
    pub review: &'static str,
    pub accept: &'static str,
    pub accept_help: &'static str,
    pub accept_required: &'static str,
    pub accepted: &'static str,
    pub reopen: &'static str,
    pub problem: &'static str,
    pub no_findings: &'static str,
}

pub fn adaptation_copy(locale: crate::localization::Locale) -> AdaptationCopy {
    AdaptationCopy {
        title: resources::text(resources::Key::AdaptationCopyTitle, locale),
        help: resources::text(resources::Key::AdaptationCopyHelp, locale),
        provider: resources::text(resources::Key::AdaptationCopyProvider, locale),
        model: resources::text(resources::Key::AdaptationCopyModel, locale),
        prompt: resources::text(resources::Key::AdaptationCopyPrompt, locale),
        config: resources::text(resources::Key::AdaptationCopyConfig, locale),
        source_id: resources::text(resources::Key::AdaptationCopySourceId, locale),
        script_id: resources::text(resources::Key::AdaptationCopyScriptId, locale),
        base: resources::text(resources::Key::AdaptationCopyBase, locale),
        pending: resources::text(resources::Key::AdaptationCopyPending, locale),
        retry: resources::text(resources::Key::AdaptationCopyRetry, locale),
        operation: resources::text(resources::Key::AdaptationCopyOperation, locale),
        run_id: resources::text(resources::Key::AdaptationCopyRunId, locale),
        open: resources::text(resources::Key::AdaptationCopyOpen, locale),
        refresh: resources::text(resources::Key::AdaptationCopyRefresh, locale),
        new_run: resources::text(resources::Key::AdaptationCopyNewRun, locale),
        new_help: resources::text(resources::Key::AdaptationCopyNewHelp, locale),
        provenance: resources::text(resources::Key::AdaptationCopyProvenance, locale),
        checksum: resources::text(resources::Key::AdaptationCopyChecksum, locale),
        extraction: resources::text(resources::Key::AdaptationCopyExtraction, locale),
        records: resources::text(resources::Key::AdaptationCopyRecords, locale),
        cost: resources::text(resources::Key::AdaptationCopyCost, locale),
        usage: resources::text(resources::Key::AdaptationCopyUsage, locale),
        unknown: resources::text(resources::Key::AdaptationCopyUnknown, locale),
        original: resources::text(resources::Key::AdaptationCopyOriginal, locale),
        source_missing: resources::text(resources::Key::AdaptationCopySourceMissing, locale),
        original_binary: resources::text(resources::Key::AdaptationCopyOriginalBinary, locale),
        download: resources::text(resources::Key::AdaptationCopyDownload, locale),
        proposal: resources::text(resources::Key::AdaptationCopyProposal, locale),
        proposal_help: resources::text(resources::Key::AdaptationCopyProposalHelp, locale),
        edit: resources::text(resources::Key::AdaptationCopyEdit, locale),
        findings: resources::text(resources::Key::AdaptationCopyFindings, locale),
        coverage: resources::text(resources::Key::AdaptationCopyCoverage, locale),
        coverage_help: resources::text(resources::Key::AdaptationCopyCoverageHelp, locale),
        represented: resources::text(resources::Key::AdaptationCopyRepresented, locale),
        omitted: resources::text(resources::Key::AdaptationCopyOmitted, locale),
        block: resources::text(resources::Key::AdaptationCopyBlock, locale),
        review: resources::text(resources::Key::AdaptationCopyReview, locale),
        accept: resources::text(resources::Key::AdaptationCopyAccept, locale),
        accept_help: resources::text(resources::Key::AdaptationCopyAcceptHelp, locale),
        accept_required: resources::text(resources::Key::AdaptationCopyAcceptRequired, locale),
        accepted: resources::text(resources::Key::AdaptationCopyAccepted, locale),
        reopen: resources::text(resources::Key::AdaptationCopyReopen, locale),
        problem: resources::text(resources::Key::AdaptationCopyProblem, locale),
        no_findings: resources::text(resources::Key::AdaptationCopyNoFindings, locale),
    }
}

pub fn adaptation_status(
    status: &ReviewStatus,
    locale: crate::localization::Locale,
) -> &'static str {
    match status {
        ReviewStatus::Idle => resources::text(resources::Key::AdaptationStatusIdle, locale),
        ReviewStatus::LoadingRun => {
            resources::text(resources::Key::AdaptationStatusLoadingRun, locale)
        }
        ReviewStatus::RunOpened => {
            resources::text(resources::Key::AdaptationStatusRunOpened, locale)
        }
        ReviewStatus::DraftChanged => {
            resources::text(resources::Key::AdaptationStatusDraftChanged, locale)
        }
        ReviewStatus::Accepting => {
            resources::text(resources::Key::AdaptationStatusAccepting, locale)
        }
        ReviewStatus::Accepted => resources::text(resources::Key::AdaptationStatusAccepted, locale),
        ReviewStatus::LoadingAccepted => {
            resources::text(resources::Key::AdaptationStatusLoadingAccepted, locale)
        }
        ReviewStatus::AcceptedOpened => {
            resources::text(resources::Key::AdaptationStatusAcceptedOpened, locale)
        }
        ReviewStatus::Error(code) => super::status(crate::editor::error_status(code), locale),
    }
}

pub fn adaptation_run_status(
    status: AdaptationStatus,
    locale: crate::localization::Locale,
) -> &'static str {
    match status {
        AdaptationStatus::AwaitingProposal => {
            resources::text(resources::Key::AdaptationRunStatusAwaitingProposal, locale)
        }
        AdaptationStatus::Succeeded => {
            resources::text(resources::Key::AdaptationRunStatusSucceeded, locale)
        }
        AdaptationStatus::Cancelled => {
            resources::text(resources::Key::AdaptationRunStatusCancelled, locale)
        }
        AdaptationStatus::Accepted => {
            resources::text(resources::Key::AdaptationRunStatusAccepted, locale)
        }
    }
}

pub fn adaptation_finding(
    code: AdaptationFindingCode,
    locale: crate::localization::Locale,
) -> &'static str {
    match code {
        AdaptationFindingCode::SourceWarning => {
            resources::text(resources::Key::AdaptationFindingSourceWarning, locale)
        }
        AdaptationFindingCode::UncoveredSource => {
            resources::text(resources::Key::AdaptationFindingUncoveredSource, locale)
        }
        AdaptationFindingCode::OmittedSource => {
            resources::text(resources::Key::AdaptationFindingOmittedSource, locale)
        }
        AdaptationFindingCode::TextChanged => {
            resources::text(resources::Key::AdaptationFindingTextChanged, locale)
        }
        AdaptationFindingCode::UnresolvedSpeaker => {
            resources::text(resources::Key::AdaptationFindingUnresolvedSpeaker, locale)
        }
        AdaptationFindingCode::NewSpeaker => {
            resources::text(resources::Key::AdaptationFindingNewSpeaker, locale)
        }
        AdaptationFindingCode::PossibleNarrationConfusion => resources::text(
            resources::Key::AdaptationFindingPossibleNarrationConfusion,
            locale,
        ),
        AdaptationFindingCode::CueLikeMarkup => {
            resources::text(resources::Key::AdaptationFindingCueLikeMarkup, locale)
        }
        AdaptationFindingCode::UnsupportedPerformanceControl => resources::text(
            resources::Key::AdaptationFindingUnsupportedPerformanceControl,
            locale,
        ),
        AdaptationFindingCode::ProviderReviewNote => {
            resources::text(resources::Key::AdaptationFindingProviderReviewNote, locale)
        }
        AdaptationFindingCode::SourceCoverageRequiresReview => resources::text(
            resources::Key::AdaptationFindingSourceCoverageRequiresReview,
            locale,
        ),
    }
}

pub fn adaptation_problem(code: &str, locale: crate::localization::Locale) -> &'static str {
    match code {
        "output_too_large" | "output_findings_too_large" => resources::text(
            resources::Key::AdaptationProblemOutputTooLargeOutputFindingsTooLarge,
            locale,
        ),
        "invalid_output" | "invalid_script_output" => resources::text(
            resources::Key::AdaptationProblemInvalidOutputInvalidScriptOutput,
            locale,
        ),
        _ => resources::text(resources::Key::AdaptationProblem, locale),
    }
}

pub fn adaptation_finding_explanation(
    code: AdaptationFindingCode,
    locale: crate::localization::Locale,
) -> &'static str {
    match code {
        AdaptationFindingCode::SourceWarning => resources::text(
            resources::Key::AdaptationFindingExplanationSourceWarning,
            locale,
        ),
        AdaptationFindingCode::UncoveredSource => resources::text(
            resources::Key::AdaptationFindingExplanationUncoveredSource,
            locale,
        ),
        AdaptationFindingCode::OmittedSource => resources::text(
            resources::Key::AdaptationFindingExplanationOmittedSource,
            locale,
        ),
        AdaptationFindingCode::TextChanged => resources::text(
            resources::Key::AdaptationFindingExplanationTextChanged,
            locale,
        ),
        AdaptationFindingCode::UnresolvedSpeaker => resources::text(
            resources::Key::AdaptationFindingExplanationUnresolvedSpeaker,
            locale,
        ),
        AdaptationFindingCode::NewSpeaker => resources::text(
            resources::Key::AdaptationFindingExplanationNewSpeaker,
            locale,
        ),
        AdaptationFindingCode::PossibleNarrationConfusion => resources::text(
            resources::Key::AdaptationFindingExplanationPossibleNarrationConfusion,
            locale,
        ),
        AdaptationFindingCode::CueLikeMarkup => resources::text(
            resources::Key::AdaptationFindingExplanationCueLikeMarkup,
            locale,
        ),
        AdaptationFindingCode::UnsupportedPerformanceControl => resources::text(
            resources::Key::AdaptationFindingExplanationUnsupportedPerformanceControl,
            locale,
        ),
        AdaptationFindingCode::ProviderReviewNote => resources::text(
            resources::Key::AdaptationFindingExplanationProviderReviewNote,
            locale,
        ),
        AdaptationFindingCode::SourceCoverageRequiresReview => resources::text(
            resources::Key::AdaptationFindingExplanationSourceCoverageRequiresReview,
            locale,
        ),
    }
}

pub fn adaptation_has_provider_detail(code: AdaptationFindingCode) -> bool {
    match code {
        AdaptationFindingCode::NewSpeaker
        | AdaptationFindingCode::OmittedSource
        | AdaptationFindingCode::UnsupportedPerformanceControl
        | AdaptationFindingCode::ProviderReviewNote => true,
        AdaptationFindingCode::SourceWarning
        | AdaptationFindingCode::UncoveredSource
        | AdaptationFindingCode::TextChanged
        | AdaptationFindingCode::UnresolvedSpeaker
        | AdaptationFindingCode::PossibleNarrationConfusion
        | AdaptationFindingCode::CueLikeMarkup
        | AdaptationFindingCode::SourceCoverageRequiresReview => false,
    }
}

pub fn adaptation_provider_detail(locale: crate::localization::Locale) -> &'static str {
    resources::text(resources::Key::AdaptationProviderDetail, locale)
}

pub fn adaptation_cost(
    cost: Option<&AdaptationCost>,
    locale: crate::localization::Locale,
) -> String {
    let Some(cost) = cost else {
        return adaptation_copy(locale).unknown.into();
    };
    let currency = match cost.currency {
        AdaptationCurrency::USD => cantos_localization::Currency::Usd,
        AdaptationCurrency::VND => cantos_localization::Currency::Vnd,
    };
    crate::localization::present(
        cantos_localization::Formatter::try_new(locale.lang())
            .and_then(|formatter| formatter.currency(cost.amount_minor, currency)),
        locale,
    )
}

#[derive(Clone, Copy)]
pub struct HostCopy {
    pub context_digest: &'static str,
    pub context_version: &'static str,
    pub rights_claim: &'static str,
    pub authorized: &'static str,
    pub absent: &'static str,
    pub submission: &'static str,
    pub caller_warning: &'static str,
    pub host_tool: &'static str,
    pub recorded_prompt: &'static str,
    pub submitted_by: &'static str,
    pub submitted_at: &'static str,
    pub output_checksum: &'static str,
}

pub fn host_copy(locale: crate::localization::Locale) -> HostCopy {
    HostCopy {
        context_digest: resources::text(resources::Key::HostCopyContextDigest, locale),
        context_version: resources::text(resources::Key::HostCopyContextVersion, locale),
        rights_claim: resources::text(resources::Key::HostCopyRightsClaim, locale),
        authorized: resources::text(resources::Key::HostCopyAuthorized, locale),
        absent: resources::text(resources::Key::HostCopyAbsent, locale),
        submission: resources::text(resources::Key::HostCopySubmission, locale),
        caller_warning: resources::text(resources::Key::HostCopyCallerWarning, locale),
        host_tool: resources::text(resources::Key::HostCopyHostTool, locale),
        recorded_prompt: resources::text(resources::Key::HostCopyRecordedPrompt, locale),
        submitted_by: resources::text(resources::Key::HostCopySubmittedBy, locale),
        submitted_at: resources::text(resources::Key::HostCopySubmittedAt, locale),
        output_checksum: resources::text(resources::Key::HostCopyOutputChecksum, locale),
    }
}

pub fn adaptation_input_revision(locale: crate::localization::Locale) -> &'static str {
    resources::text(resources::Key::AdaptationInputRevision, locale)
}

pub fn adaptation_submission_status(
    status: AdaptationSubmissionStatus,
    locale: crate::localization::Locale,
) -> &'static str {
    match status {
        AdaptationSubmissionStatus::Valid => {
            resources::text(resources::Key::AdaptationSubmissionStatusValid, locale)
        }
        AdaptationSubmissionStatus::Invalid => {
            resources::text(resources::Key::AdaptationSubmissionStatusInvalid, locale)
        }
    }
}

pub fn adaptation_usage(
    usage: Option<&AdaptationUsage>,
    locale: crate::localization::Locale,
) -> String {
    let unknown = adaptation_copy(locale).unknown;
    let number = |value: u64| {
        crate::localization::present(
            cantos_localization::Formatter::try_new(locale.lang())
                .and_then(|formatter| formatter.number(&value.to_string())),
            locale,
        )
    };
    let input = usage
        .and_then(|usage| usage.input_tokens)
        .map(number)
        .unwrap_or_else(|| unknown.into());
    let output = usage
        .and_then(|usage| usage.output_tokens)
        .map(number)
        .unwrap_or_else(|| unknown.into());
    crate::localization::present(
        crate::localization::generated::Message::StudioAdaptationUsage {
            arg_input: &input,
            arg_output: &output,
        }
        .render(locale),
        locale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_run_and_review_finding_has_both_locales_and_unknown_cost_is_not_zero() {
        for locale in crate::localization::Locale::supported() {
            for status in [
                AdaptationStatus::AwaitingProposal,
                AdaptationStatus::Succeeded,
                AdaptationStatus::Cancelled,
                AdaptationStatus::Accepted,
            ] {
                assert!(!adaptation_run_status(status, locale).is_empty());
            }
            for finding in [
                AdaptationFindingCode::SourceWarning,
                AdaptationFindingCode::UncoveredSource,
                AdaptationFindingCode::OmittedSource,
                AdaptationFindingCode::TextChanged,
                AdaptationFindingCode::UnresolvedSpeaker,
                AdaptationFindingCode::NewSpeaker,
                AdaptationFindingCode::PossibleNarrationConfusion,
                AdaptationFindingCode::CueLikeMarkup,
                AdaptationFindingCode::UnsupportedPerformanceControl,
                AdaptationFindingCode::ProviderReviewNote,
                AdaptationFindingCode::SourceCoverageRequiresReview,
            ] {
                assert!(!adaptation_finding(finding, locale).is_empty());
                assert!(!adaptation_finding_explanation(finding, locale).is_empty());
            }
            assert!(!adaptation_copy(locale).unknown.contains('0'));
            assert!(!host_copy(locale).caller_warning.is_empty());
            for status in [
                AdaptationSubmissionStatus::Valid,
                AdaptationSubmissionStatus::Invalid,
            ] {
                assert!(!adaptation_submission_status(status, locale).is_empty());
            }
        }
    }

    #[test]
    fn deterministic_findings_use_localized_explanations_and_only_content_notes_are_quoted() {
        for code in [
            AdaptationFindingCode::UncoveredSource,
            AdaptationFindingCode::TextChanged,
            AdaptationFindingCode::UnresolvedSpeaker,
            AdaptationFindingCode::PossibleNarrationConfusion,
            AdaptationFindingCode::CueLikeMarkup,
            AdaptationFindingCode::SourceCoverageRequiresReview,
        ] {
            assert!(!adaptation_has_provider_detail(code));
            assert_ne!(
                adaptation_finding_explanation(code, crate::localization::Locale::Vietnamese),
                adaptation_finding_explanation(code, crate::localization::Locale::English)
            );
        }
        for code in [
            AdaptationFindingCode::NewSpeaker,
            AdaptationFindingCode::OmittedSource,
            AdaptationFindingCode::UnsupportedPerformanceControl,
            AdaptationFindingCode::ProviderReviewNote,
        ] {
            assert!(adaptation_has_provider_detail(code));
        }
        assert!(!adaptation_has_provider_detail(
            AdaptationFindingCode::SourceWarning
        ));
    }

    #[test]
    fn cost_uses_recorded_currency_minor_units_and_missing_cost_stays_unknown() {
        use crate::contract::AdaptationCostBasis;
        let usd = AdaptationCost {
            currency: AdaptationCurrency::USD,
            amount_minor: 123,
            basis: AdaptationCostBasis::CallerDeclared,
        };
        let vnd = AdaptationCost {
            currency: AdaptationCurrency::VND,
            amount_minor: 123,
            basis: AdaptationCostBasis::CallerDeclared,
        };
        assert_eq!(
            adaptation_cost(Some(&usd), crate::localization::Locale::English),
            "USD\u{a0}1.23"
        );
        assert_eq!(
            adaptation_cost(Some(&vnd), crate::localization::Locale::Vietnamese),
            "123\u{a0}VND"
        );
        for locale in crate::localization::Locale::supported() {
            assert_eq!(
                adaptation_cost(None, locale),
                adaptation_copy(locale).unknown
            );
        }
    }

    #[test]
    fn host_workflow_copy_discloses_unverified_metadata_and_preserves_unknown_usage() {
        assert!(adaptation_copy(crate::localization::Locale::English)
            .help
            .contains("does not generate content or call AI"));
        assert!(adaptation_copy(crate::localization::Locale::Vietnamese)
            .help
            .contains("không tự sinh nội dung hay gọi AI"));
        assert!(host_copy(crate::localization::Locale::English)
            .caller_warning
            .contains("caller declarations"));
        assert!(host_copy(crate::localization::Locale::Vietnamese)
            .caller_warning
            .contains("người gửi khai báo"));
        for locale in crate::localization::Locale::supported() {
            let unknown = adaptation_copy(locale).unknown;
            assert_eq!(
                adaptation_usage(None, locale),
                format!("{unknown} / {unknown}")
            );
            let partial = AdaptationUsage {
                input_tokens: Some(0),
                output_tokens: None,
            };
            assert_eq!(
                adaptation_usage(Some(&partial), locale),
                format!("0 / {unknown}")
            );
        }
    }
}
