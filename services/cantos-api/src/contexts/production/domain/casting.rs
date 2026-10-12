//! Casting resolution: every spoken line becomes one exact synthesis input with its voice,
//! effective text after pronunciation, performance and the adapter contract it is frozen for.
use std::collections::{BTreeMap, BTreeSet};

use cantos_contracts::{
    FieldIssue, ProductionSettings, ResolvedPronunciation, ResolvedSynthesisInput,
    SynthesisPerformance,
};
use serde::Deserialize;
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use super::catalog::{casting_contract, ADAPTER_VERSION, OUTPUT_CONTRACT};
use super::issue;
use crate::script_ir::ScriptContent;

pub const MAX_EFFECTIVE_LINE_BYTES: usize = 64 * 1024;
pub const MAX_RESOLVED_BYTES: usize = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpokenFacts {
    text: String,
    language: String,
    delivery: ScriptDelivery,
    #[serde(default)]
    pronunciation_overrides: Vec<ResolvedPronunciation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScriptDelivery {
    emotion: String,
    intensity_permille: u16,
}

pub(super) fn resolve_inputs(
    script: &ScriptContent,
    settings: &ProductionSettings,
) -> Result<Vec<ResolvedSynthesisInput>, Vec<FieldIssue>> {
    let mut issues = Vec::new();
    let bindings: BTreeMap<_, _> = settings
        .bindings
        .iter()
        .map(|b| (b.character_id.as_str(), b))
        .collect();
    let export: Value = serde_json::from_slice(&script.export_bytes()).map_err(|_| {
        vec![FieldIssue {
            path: "revision.script_json".into(),
            rule: "invalid_canonical_export".into(),
        }]
    })?;
    let characters: BTreeSet<_> = export["characters"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| value["id"].as_str())
        .collect();
    for binding in &settings.bindings {
        if !characters.contains(binding.character_id.as_str()) {
            issue(
                &mut issues,
                format!("bindings.{}", binding.character_id),
                "unknown_character",
            );
        }
    }
    let mut resolved = Vec::new();
    let mut resolved_bytes = 2_usize;
    for line in script.spoken_lines() {
        let Some(binding) = bindings.get(line.speaker_id()) else {
            issue(
                &mut issues,
                format!("dialogues.{}.speaker_id", line.dialogue_id()),
                "casting_missing",
            );
            continue;
        };
        let facts: SpokenFacts = serde_json::from_slice(&line.canonical_bytes()).map_err(|_| {
            vec![FieldIssue {
                path: "revision.script_json".into(),
                rule: "invalid_spoken_content".into(),
            }]
        })?;
        if facts.language != binding.language {
            issue(
                &mut issues,
                format!("dialogues.{}.language", line.dialogue_id()),
                "language_mismatch_no_translation",
            );
        }
        let mut pronunciations: BTreeMap<String, String> = binding
            .pronunciation
            .iter()
            .map(|p| (p.surface.clone(), p.replacement.clone()))
            .collect();
        for p in facts.pronunciation_overrides {
            if pronunciations
                .get(&p.surface)
                .is_some_and(|replacement| replacement != &p.replacement)
            {
                issue(
                    &mut issues,
                    format!("dialogues.{}.pronunciation", line.dialogue_id()),
                    "conflicting_pronunciation",
                );
            }
            pronunciations.insert(p.surface, p.replacement);
        }
        let pronunciation: Vec<_> = pronunciations
            .into_iter()
            .map(|(surface, replacement)| ResolvedPronunciation {
                surface,
                replacement,
            })
            .collect();
        let text = apply_pronunciation(&facts.text, &pronunciation).ok_or_else(|| {
            vec![FieldIssue {
                path: format!("dialogues.{}.text", line.dialogue_id()),
                rule: "effective_text_too_large".into(),
            }]
        })?;
        let performance = SynthesisPerformance {
            rate_permille: binding.performance.rate_permille,
            pitch_semitones: binding.performance.pitch_semitones,
            emotion: Some(
                binding
                    .performance
                    .emotion
                    .clone()
                    .unwrap_or(facts.delivery.emotion),
            ),
            intensity_permille: Some(
                binding
                    .performance
                    .intensity_permille
                    .unwrap_or(facts.delivery.intensity_permille),
            ),
        };
        let contract = casting_contract(&binding.provider_id);
        if let Some(contract) = &contract {
            if performance.emotion.as_deref() != Some("neutral")
                || performance.intensity_permille != Some(0)
            {
                issue(
                    &mut issues,
                    format!("dialogues.{}.performance", line.dialogue_id()),
                    "unsupported_control",
                );
            }
            if let Some(rule) = contract.text_issue(&text) {
                issue(
                    &mut issues,
                    format!("dialogues.{}.text", line.dialogue_id()),
                    rule,
                );
            }
        }
        let resolved_line = ResolvedSynthesisInput {
            dialogue_id: line.dialogue_id().into(),
            character_id: line.speaker_id().into(),
            text,
            language: binding.language.clone(),
            provider_id: binding.provider_id.clone(),
            model_id: binding.model_id.clone(),
            voice_id: binding.voice_id.clone(),
            voice_rights_record_id: binding.voice_rights_record_id.clone(),
            adapter_version: contract
                .map_or(ADAPTER_VERSION, |contract| contract.adapter_version)
                .into(),
            output_contract: contract
                .map_or(OUTPUT_CONTRACT, |contract| contract.output_contract)
                .into(),
            performance,
            pronunciation,
        };
        let bytes = serde_json::to_vec(&resolved_line).map_err(|_| {
            vec![FieldIssue {
                path: "resolved".into(),
                rule: "serialization_failed".into(),
            }]
        })?;
        resolved_bytes = resolved_bytes
            .checked_add(bytes.len() + 1)
            .filter(|sum| *sum <= MAX_RESOLVED_BYTES)
            .ok_or_else(|| {
                vec![FieldIssue {
                    path: "resolved".into(),
                    rule: "resolved_document_too_large".into(),
                }]
            })?;
        resolved.push(resolved_line);
    }
    if issues.is_empty() {
        Ok(resolved)
    } else {
        Err(issues)
    }
}

/// Match longest surface at each position; replacements are never scanned again.
fn apply_pronunciation(text: &str, values: &[ResolvedPronunciation]) -> Option<String> {
    let mut ordered: Vec<_> = values.iter().collect();
    ordered.sort_by(|a, b| {
        b.surface
            .len()
            .cmp(&a.surface.len())
            .then(a.surface.cmp(&b.surface))
    });
    let mut result = String::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        if let Some(value) = ordered
            .iter()
            .find(|value| remaining.starts_with(&value.surface))
        {
            if result.len().checked_add(value.replacement.len())? > MAX_EFFECTIVE_LINE_BYTES {
                return None;
            }
            result.push_str(&value.replacement);
            remaining = &remaining[value.surface.len()..];
        } else if let Some(ch) = remaining.chars().next() {
            if result.len().checked_add(ch.len_utf8())? > MAX_EFFECTIVE_LINE_BYTES {
                return None;
            }
            result.push(ch);
            remaining = &remaining[ch.len_utf8()..];
        }
    }
    let normalized: String = result.nfc().collect();
    (normalized.len() <= MAX_EFFECTIVE_LINE_BYTES).then_some(normalized)
}
