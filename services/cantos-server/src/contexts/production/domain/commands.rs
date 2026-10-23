//! Pure decisions behind each production command. The shell reads the facts under the script
//! lock, asks here, and writes only what the decision allows.
use crate::contract::studio::{
    FieldIssue, ProductionEligibility, ProductionFinding, ProductionRightsSubject,
    ProductionSettingsResponse,
};

use super::estimate::MAX_MONEY_MINOR;

/// Current rights claims one script may hold.
pub const MAX_RIGHTS: usize = 128;

#[derive(Debug, PartialEq, Eq)]
pub enum CommandRejection {
    /// The request was prepared against another accepted script head.
    StaleRevision(u64),
    /// Settings, a rights claim or the frozen input identity moved since the request was made.
    StaleInputs,
    InvalidRequest,
    /// The inputs are not eligible; each issue names one blocking finding.
    Blocked(Vec<FieldIssue>),
}

/// Settings are versioned per script and recorded against the head they were chosen for.
/// Returns the next settings version.
pub fn decide_settings_version(
    head: u64,
    current_version: u64,
    expected_revision: u64,
    expected_settings_version: u64,
) -> Result<u64, CommandRejection> {
    if head != expected_revision {
        return Err(CommandRejection::StaleRevision(head));
    }
    if current_version != expected_settings_version {
        return Err(CommandRejection::StaleInputs);
    }
    current_version
        .checked_add(1)
        .filter(|version| *version <= MAX_MONEY_MINOR)
        .ok_or(CommandRejection::InvalidRequest)
}

/// A claim revises one record: it names the version it replaces (zero for a new record), a
/// script holds at most [`MAX_RIGHTS`] records, and a record identity is never rebound to
/// another subject. Returns the next claim version.
pub fn decide_rights_version(
    prior: Option<(u64, &ProductionRightsSubject)>,
    current_records: usize,
    subject: &ProductionRightsSubject,
    expected_version: u64,
) -> Result<u64, CommandRejection> {
    if prior.map_or(0, |(version, _)| version) != expected_version {
        return Err(CommandRejection::StaleInputs);
    }
    if prior.is_none() && current_records >= MAX_RIGHTS {
        return Err(CommandRejection::InvalidRequest);
    }
    if prior.is_some_and(|(_, prior)| prior != subject) {
        return Err(CommandRejection::InvalidRequest);
    }
    expected_version
        .checked_add(1)
        .filter(|version| *version <= i64::MAX as u64)
        .ok_or(CommandRejection::InvalidRequest)
}

/// Freezing preserves exactly the head and settings version the creator previewed.
pub fn check_freeze(
    head: u64,
    settings: Option<&ProductionSettingsResponse>,
    expected_revision: u64,
    settings_version: u64,
) -> Result<&ProductionSettingsResponse, CommandRejection> {
    if head != expected_revision {
        return Err(CommandRejection::StaleRevision(head));
    }
    let settings = settings.ok_or(CommandRejection::StaleInputs)?;
    if settings.version != settings_version || settings.script_revision != head {
        return Err(CommandRejection::StaleInputs);
    }
    Ok(settings)
}

/// The resolved inputs must still have the identity the creator previewed.
pub fn check_input_identity(resolved: &str, previewed: &str) -> Result<(), CommandRejection> {
    if resolved != previewed {
        return Err(CommandRejection::StaleInputs);
    }
    Ok(())
}

/// An approval names the exact frozen identity, needs eligible inputs and is recorded once.
pub fn check_approval(
    frozen_digest: &str,
    eligibility: &ProductionEligibility,
    already_approved: bool,
    approved_digest: &str,
) -> Result<(), CommandRejection> {
    check_input_identity(frozen_digest, approved_digest)?;
    if !eligibility.inputs_eligible {
        return Err(CommandRejection::Blocked(blocking_issues(
            &eligibility.findings,
        )));
    }
    if already_approved {
        return Err(CommandRejection::StaleInputs);
    }
    Ok(())
}

/// Eligibility can lapse while a command waits for locks; the approval it records, or the
/// dispatch it authorizes, must still be current when the transaction commits.
pub fn require_current_approval(
    eligibility: &ProductionEligibility,
) -> Result<(), CommandRejection> {
    if !eligibility.inputs_eligible || !eligibility.approval_current {
        return Err(CommandRejection::Blocked(blocking_issues(
            &eligibility.findings,
        )));
    }
    Ok(())
}

/// Each blocking finding as a stable rule code at its path. Finding details are not sent.
pub fn blocking_issues(findings: &[ProductionFinding]) -> Vec<FieldIssue> {
    findings
        .iter()
        .map(|finding| FieldIssue {
            path: finding.path.clone(),
            rule: serde_json::to_value(finding.code)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "production_blocked".into()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::contract::studio::{ProductionFindingCode, ProductionSettings};

    use super::*;

    fn eligibility(inputs: bool, approval: bool) -> ProductionEligibility {
        ProductionEligibility {
            inputs_eligible: inputs,
            approval_current: approval,
            billable_dispatch_available: false,
            findings: vec![ProductionFinding {
                code: ProductionFindingCode::RightsMissing,
                path: "rights.voice-claim".into(),
                detail: "No persisted production claim covers this required rights record.".into(),
            }],
        }
    }

    fn blocked() -> CommandRejection {
        CommandRejection::Blocked(vec![FieldIssue {
            path: "rights.voice-claim".into(),
            rule: "rights_missing".into(),
        }])
    }

    #[test]
    fn settings_advance_one_version_from_the_expected_head_and_version() {
        assert_eq!(decide_settings_version(3, 0, 3, 0), Ok(1));
        assert_eq!(decide_settings_version(3, 4, 3, 4), Ok(5));
        assert_eq!(
            decide_settings_version(3, 4, 2, 4),
            Err(CommandRejection::StaleRevision(3))
        );
        assert_eq!(
            decide_settings_version(3, 4, 3, 3),
            Err(CommandRejection::StaleInputs)
        );
        assert_eq!(
            decide_settings_version(3, MAX_MONEY_MINOR, 3, MAX_MONEY_MINOR),
            Err(CommandRejection::InvalidRequest)
        );
    }

    #[test]
    fn a_rights_record_is_revised_in_order_bounded_and_never_rebound() {
        let voice = ProductionRightsSubject::Voice {
            provider_id: "espeak-ng-local".into(),
            model_id: "espeak-ng-1.52.0".into(),
            voice_id: "vi".into(),
        };
        let evidence = ProductionRightsSubject::Evidence {
            record_id: "voice-claim".into(),
        };
        assert_eq!(decide_rights_version(None, 0, &voice, 0), Ok(1));
        assert_eq!(
            decide_rights_version(Some((2, &voice)), 9, &voice, 2),
            Ok(3)
        );
        assert_eq!(
            decide_rights_version(Some((2, &voice)), 9, &voice, 1),
            Err(CommandRejection::StaleInputs)
        );
        assert_eq!(
            decide_rights_version(None, 0, &voice, 1),
            Err(CommandRejection::StaleInputs)
        );
        assert_eq!(
            decide_rights_version(None, MAX_RIGHTS, &voice, 0),
            Err(CommandRejection::InvalidRequest)
        );
        assert_eq!(
            decide_rights_version(Some((1, &voice)), MAX_RIGHTS, &voice, 1),
            Ok(2)
        );
        assert_eq!(
            decide_rights_version(Some((1, &evidence)), 1, &voice, 1),
            Err(CommandRejection::InvalidRequest)
        );
    }

    #[test]
    fn rights_successor_never_exceeds_the_postgres_storage_domain() {
        let subject = ProductionRightsSubject::Evidence {
            record_id: "voice-claim".into(),
        };
        for version in [i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX] {
            assert_eq!(
                decide_rights_version(Some((version, &subject)), 1, &subject, version),
                Err(CommandRejection::InvalidRequest)
            );
        }
        assert_eq!(
            decide_rights_version(
                Some((i64::MAX as u64 - 1, &subject)),
                1,
                &subject,
                i64::MAX as u64 - 1,
            ),
            Ok(i64::MAX as u64)
        );
    }

    #[test]
    fn a_freeze_needs_the_previewed_head_and_settings() {
        let settings = ProductionSettingsResponse {
            script_id: "44444444-4444-4444-8444-444444444444".into(),
            version: 2,
            script_revision: 3,
            operation_id: "55555555-5555-4555-8555-555555555555".into(),
            recorded_by: "alice".into(),
            recorded_at: "2026-10-12T00:00:00.000000Z".into(),
            settings: ProductionSettings {
                bindings: vec![],
                budget: crate::contract::studio::ProductionBudget {
                    currency: crate::contract::studio::ProductionCurrency::VND,
                    limit_minor: 0,
                    scope: "private".into(),
                    territory: "private-planning".into(),
                    rate: None,
                },
            },
        };
        assert_eq!(check_freeze(3, Some(&settings), 3, 2), Ok(&settings));
        assert_eq!(
            check_freeze(3, Some(&settings), 2, 2),
            Err(CommandRejection::StaleRevision(3))
        );
        assert_eq!(
            check_freeze(3, None, 3, 2),
            Err(CommandRejection::StaleInputs)
        );
        assert_eq!(
            check_freeze(3, Some(&settings), 3, 1),
            Err(CommandRejection::StaleInputs)
        );
        assert_eq!(
            check_freeze(4, Some(&settings), 4, 2),
            Err(CommandRejection::StaleInputs)
        );
    }

    #[test]
    fn an_approval_checks_identity_then_eligibility_then_uniqueness() {
        assert_eq!(
            check_approval("a", &eligibility(true, false), false, "a"),
            Ok(())
        );
        assert_eq!(
            check_approval("a", &eligibility(false, false), true, "b"),
            Err(CommandRejection::StaleInputs)
        );
        assert_eq!(
            check_approval("a", &eligibility(false, false), true, "a"),
            Err(blocked())
        );
        assert_eq!(
            check_approval("a", &eligibility(true, false), true, "a"),
            Err(CommandRejection::StaleInputs)
        );
    }

    #[test]
    fn only_eligible_and_current_approval_authorizes() {
        assert_eq!(require_current_approval(&eligibility(true, true)), Ok(()));
        for (inputs, approval) in [(true, false), (false, true), (false, false)] {
            assert_eq!(
                require_current_approval(&eligibility(inputs, approval)),
                Err(blocked())
            );
        }
    }
}
