//! Casting settings: per-character voice bindings checked against the catalog, pronunciation
//! overrides and the private planning budget. Structural admission precedes persistence.
use std::collections::BTreeSet;

use crate::contract::studio::{FieldIssue, ProductionPronunciation, ProductionSettings};

use super::catalog::production_catalog;
use super::estimate::MAX_MONEY_MINOR;
use super::{issue, valid_id, valid_registry_id, valid_text};

pub const MAX_SETTINGS_BYTES: usize = 256 * 1024;
const MAX_BINDINGS: usize = 1_000;
const MAX_PRONUNCIATIONS: usize = 100;

/// Structural admission precedes persistence; incomplete casting is explained on preview.
pub fn validate_settings(settings: &ProductionSettings) -> Result<(), Vec<FieldIssue>> {
    let mut issues = Vec::new();
    if serde_json::to_vec(settings).map_or(true, |bytes| bytes.len() > MAX_SETTINGS_BYTES) {
        issue(&mut issues, "settings", "document_too_large");
    }
    if settings.bindings.len() > MAX_BINDINGS {
        issue(&mut issues, "bindings", "array_length");
    }
    let mut seen = BTreeSet::new();
    let catalog = production_catalog();
    for (index, binding) in settings.bindings.iter().enumerate() {
        let path = format!("bindings[{index}]");
        let capability = catalog
            .capabilities
            .iter()
            .find(|capability| capability.model_id == binding.model_id)
            .unwrap_or(&catalog.capabilities[0]);
        if !valid_id(&binding.character_id) || !seen.insert(&binding.character_id) {
            issue(
                &mut issues,
                format!("{path}.character_id"),
                "invalid_or_duplicate_id",
            );
        }
        for (field, actual, supported) in [
            (
                "provider_id",
                binding.provider_id.as_str(),
                capability.provider_id.as_str(),
            ),
            (
                "model_id",
                binding.model_id.as_str(),
                capability.model_id.as_str(),
            ),
        ] {
            if actual != supported {
                issue(
                    &mut issues,
                    format!("{path}.{field}"),
                    "unsupported_control",
                );
            }
        }
        if !capability.voice_ids.contains(&binding.voice_id) {
            issue(
                &mut issues,
                format!("{path}.voice_id"),
                "unsupported_control",
            );
        }
        if !capability.languages.contains(&binding.language) {
            issue(
                &mut issues,
                format!("{path}.language"),
                "unsupported_control",
            );
        }
        if !valid_registry_id(&binding.voice_rights_record_id) {
            issue(
                &mut issues,
                format!("{path}.voice_rights_record_id"),
                "id_format",
            );
        }
        if !(capability.rate_min_permille..=capability.rate_max_permille)
            .contains(&binding.performance.rate_permille)
        {
            issue(
                &mut issues,
                format!("{path}.performance.rate_permille"),
                "unsupported_control",
            );
        }
        if !(capability.pitch_min_semitones..=capability.pitch_max_semitones)
            .contains(&binding.performance.pitch_semitones)
        {
            issue(
                &mut issues,
                format!("{path}.performance.pitch_semitones"),
                "unsupported_control",
            );
        }
        if binding
            .performance
            .emotion
            .as_ref()
            .is_some_and(|s| !capability.emotions.contains(s))
        {
            issue(
                &mut issues,
                format!("{path}.performance.emotion"),
                "unsupported_control",
            );
        }
        if binding
            .performance
            .intensity_permille
            .is_some_and(|v| v > 1_000 || (!capability.intensity_supported && v != 0))
        {
            issue(
                &mut issues,
                format!("{path}.performance.intensity_permille"),
                "unsupported_control",
            );
        }
        validate_pronunciation(
            &binding.pronunciation,
            &format!("{path}.pronunciation"),
            &mut issues,
        );
    }
    if settings.budget.limit_minor > MAX_MONEY_MINOR {
        issue(&mut issues, "budget.limit_minor", "money_out_of_range");
    }
    if !valid_text(&settings.budget.scope, 512) {
        issue(&mut issues, "budget.scope", "text_length_or_normalization");
    }
    if settings.budget.territory != "private-planning" {
        issue(
            &mut issues,
            "budget.territory",
            "unsupported_private_planning_territory",
        );
    }
    if let Some(rate) = &settings.budget.rate {
        if !valid_text(&rate.reference, 512) {
            issue(
                &mut issues,
                "budget.rate.reference",
                "text_length_or_normalization",
            );
        }
        if !valid_text(&rate.version, 128) {
            issue(
                &mut issues,
                "budget.rate.version",
                "text_length_or_normalization",
            );
        }
        if rate.units_per_charge == 0 || rate.units_per_charge > MAX_MONEY_MINOR {
            issue(
                &mut issues,
                "budget.rate.units_per_charge",
                "billing_unit_out_of_range",
            );
        }
        if rate.amount_minor > MAX_MONEY_MINOR {
            issue(
                &mut issues,
                "budget.rate.amount_minor",
                "money_out_of_range",
            );
        }
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

fn validate_pronunciation(
    values: &[ProductionPronunciation],
    path: &str,
    issues: &mut Vec<FieldIssue>,
) {
    if values.len() > MAX_PRONUNCIATIONS {
        issue(issues, path, "array_length");
    }
    let mut surfaces = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !valid_text(&value.surface, 512) || !surfaces.insert(&value.surface) {
            issue(
                issues,
                format!("{path}[{index}].surface"),
                "invalid_or_duplicate_surface",
            );
        }
        if !valid_text(&value.replacement, 512) {
            issue(
                issues,
                format!("{path}[{index}].replacement"),
                "text_length_or_normalization",
            );
        }
    }
}
