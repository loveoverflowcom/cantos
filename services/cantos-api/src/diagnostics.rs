//! Pure safe diagnostic projection shared by untrusted Script IR entry adapters.
use cantos_contracts::FieldIssue;

use crate::script_ir::{ReadError, ShapeRule, ValidationIssue};

pub(crate) fn validation_issues(error: ReadError) -> Vec<FieldIssue> {
    match error {
        ReadError::Shape(issues) => issues
            .into_iter()
            .map(|issue| FieldIssue {
                path: issue.path,
                rule: match issue.rule {
                    ShapeRule::IdFormat => "id_format",
                    ShapeRule::TextLength => "text_length",
                    ShapeRule::ArrayLength => "array_length",
                    ShapeRule::IntensityRange => "intensity_range",
                }
                .into(),
            })
            .collect(),
        ReadError::Semantic(issues) => issues
            .into_iter()
            .map(|issue| FieldIssue {
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
                }
                .into(),
            })
            .collect(),
        ReadError::DocumentTooLarge { .. } => vec![field_issue("DocumentTooLarge")],
        ReadError::InvalidDocument { .. } => vec![field_issue("InvalidDocument")],
        ReadError::MissingSchemaVersion => vec![field_issue("MissingSchemaVersion")],
        ReadError::UnsupportedSchemaVersion { .. } => vec![field_issue("UnsupportedSchemaVersion")],
        ReadError::NonCanonicalDocument => vec![field_issue("NonCanonicalDocument")],
    }
}

fn field_issue(rule: &str) -> FieldIssue {
    FieldIssue {
        path: "$".into(),
        rule: rule.into(),
    }
}
