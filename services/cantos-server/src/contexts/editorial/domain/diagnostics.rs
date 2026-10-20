//! Pure safe diagnostic projection shared by untrusted Script IR entry adapters.
use crate::script_ir::{ReadError, ShapeRule, ValidationIssue};

/// Safe correction facts; raw reader payloads never enter this projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafeDiagnostic {
    pub path: String,
    pub rule: &'static str,
}

pub(crate) fn validation_issues(error: ReadError) -> Vec<SafeDiagnostic> {
    match error {
        ReadError::Shape(issues) => issues
            .into_iter()
            .map(|issue| SafeDiagnostic {
                path: issue.path,
                rule: match issue.rule {
                    ShapeRule::IdFormat => "id_format",
                    ShapeRule::TextLength => "text_length",
                    ShapeRule::ArrayLength => "array_length",
                    ShapeRule::IntensityRange => "intensity_range",
                },
            })
            .collect(),
        ReadError::Semantic(issues) => issues
            .into_iter()
            .map(|issue| SafeDiagnostic {
                path: issue.path,
                // Only stable codes cross the boundary; payloads can contain manuscript text.
                rule: match issue.issue {
                    ValidationIssue::DuplicateId { .. } => "duplicate_id",
                    ValidationIssue::NarratorCount { .. } => "narrator_count",
                    ValidationIssue::UnknownSpeaker { .. } => "unknown_speaker",
                    ValidationIssue::CueAnchorUnresolved { .. } => "cue_anchor_unresolved",
                    ValidationIssue::UnknownProvenance { .. } => "unknown_provenance",
                    ValidationIssue::DuplicateProvenanceRef { .. } => "duplicate_provenance_ref",
                    ValidationIssue::EmptyText => "empty_text",
                    ValidationIssue::NormalizedTextTooLong { .. } => "normalized_text_too_long",
                    ValidationIssue::ForbiddenCharacter { .. } => "forbidden_character",
                    ValidationIssue::PronunciationTargetMissing { .. } => {
                        "pronunciation_target_missing"
                    }
                    ValidationIssue::PronunciationOverlap { .. } => "pronunciation_overlap",
                },
            })
            .collect(),
        ReadError::DocumentTooLarge { .. } => vec![field_issue("DocumentTooLarge")],
        ReadError::InvalidDocument { .. } => vec![field_issue("InvalidDocument")],
        ReadError::MissingSchemaVersion => vec![field_issue("MissingSchemaVersion")],
        ReadError::UnsupportedSchemaVersion { .. } => vec![field_issue("UnsupportedSchemaVersion")],
        ReadError::NonCanonicalDocument => vec![field_issue("NonCanonicalDocument")],
    }
}

fn field_issue(rule: &'static str) -> SafeDiagnostic {
    SafeDiagnostic {
        path: "$".into(),
        rule,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script_ir::{Diagnostic, ShapeDiagnostic};

    #[test]
    fn safe_classification_preserves_order_paths_and_codes_and_discards_payloads() {
        let cases = [
            (
                ValidationIssue::DuplicateId {
                    id: "private-id".into(),
                    first_seen: "private-first".into(),
                },
                "duplicate_id",
            ),
            (
                ValidationIssue::NarratorCount { found: 42 },
                "narrator_count",
            ),
            (
                ValidationIssue::UnknownSpeaker {
                    speaker_id: "private-speaker".into(),
                },
                "unknown_speaker",
            ),
            (
                ValidationIssue::CueAnchorUnresolved {
                    dialogue_id: "private-dialogue".into(),
                },
                "cue_anchor_unresolved",
            ),
            (
                ValidationIssue::UnknownProvenance {
                    source_id: "private-source".into(),
                },
                "unknown_provenance",
            ),
            (
                ValidationIssue::DuplicateProvenanceRef {
                    source_id: "private-source".into(),
                },
                "duplicate_provenance_ref",
            ),
            (ValidationIssue::EmptyText, "empty_text"),
            (
                ValidationIssue::NormalizedTextTooLong { max_chars: 42 },
                "normalized_text_too_long",
            ),
            (
                ValidationIssue::ForbiddenCharacter { code_point: 42 },
                "forbidden_character",
            ),
            (
                ValidationIssue::PronunciationTargetMissing {
                    surface: "private-surface".into(),
                },
                "pronunciation_target_missing",
            ),
            (
                ValidationIssue::PronunciationOverlap {
                    surface: "private-surface".into(),
                },
                "pronunciation_overlap",
            ),
        ];
        let expected = cases
            .iter()
            .enumerate()
            .map(|(index, (_, rule))| SafeDiagnostic {
                path: format!("episode/dialogue-{index}"),
                rule,
            })
            .collect::<Vec<_>>();
        let raw = cases
            .into_iter()
            .enumerate()
            .map(|(index, (issue, _))| Diagnostic {
                path: format!("episode/dialogue-{index}"),
                issue,
            })
            .collect();
        let safe = validation_issues(ReadError::Semantic(raw));
        assert_eq!(safe, expected);
        assert!(!format!("{safe:?}").contains("private-"));
        for (rule, expected) in [
            (ShapeRule::IdFormat, "id_format"),
            (ShapeRule::TextLength, "text_length"),
            (ShapeRule::ArrayLength, "array_length"),
            (ShapeRule::IntensityRange, "intensity_range"),
        ] {
            assert_eq!(
                validation_issues(ReadError::Shape(vec![ShapeDiagnostic {
                    path: "/work/id".into(),
                    rule,
                }])),
                vec![SafeDiagnostic {
                    path: "/work/id".into(),
                    rule: expected
                }]
            );
        }
    }

    #[test]
    fn root_diagnostics_keep_legacy_codes_without_reader_details() {
        for (error, rule) in [
            (
                ReadError::DocumentTooLarge { max_bytes: 42 },
                "DocumentTooLarge",
            ),
            (
                ReadError::InvalidDocument {
                    line: 4,
                    column: 2,
                    detail: "private-manuscript".into(),
                },
                "InvalidDocument",
            ),
            (ReadError::MissingSchemaVersion, "MissingSchemaVersion"),
            (
                ReadError::UnsupportedSchemaVersion {
                    found: "private-version".into(),
                    supported: "0.1.0",
                },
                "UnsupportedSchemaVersion",
            ),
            (ReadError::NonCanonicalDocument, "NonCanonicalDocument"),
        ] {
            assert_eq!(
                validation_issues(error),
                vec![SafeDiagnostic {
                    path: "$".into(),
                    rule
                }]
            );
        }
    }
}
