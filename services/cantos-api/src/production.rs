//! Pure casting resolution, immutable input identity and fail-closed authorization planning.
//! This module has no HTTP, database, clock or provider dependency and sends no requests.
use std::collections::{BTreeMap, BTreeSet};

use cantos_contracts::{
    FieldIssue, FrozenProductionDocument, ProductionApprovalResponse, ProductionBudget,
    ProductionCapability, ProductionCatalogResponse, ProductionCurrency, ProductionEligibility,
    ProductionEstimate, ProductionFinding, ProductionFindingCode, ProductionPreviewResponse,
    ProductionPronunciation, ProductionRightsClaimResponse, ProductionRightsDeclaration,
    ProductionRightsStatus, ProductionRightsSubject, ProductionSettings,
    ProductionSettingsResponse, ResolvedPronunciation, ResolvedSynthesisInput, RevisionResponse,
    SynthesisPerformance,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::script_ir::{read_canonical_script, ScriptContent};
use crate::tts::{google_contract, local_contract, neural_contract};

pub const CATALOG_VERSION: &str = "production-reference-v1";
pub const ADAPTER_VERSION: &str = "reference-only-no-dispatch-v1";
pub const OUTPUT_CONTRACT: &str = "reference-only/no-audio";
pub const MAX_SETTINGS_BYTES: usize = 256 * 1024;
pub const MAX_MONEY_MINOR: u64 = i64::MAX as u64;
pub const MAX_EFFECTIVE_LINE_BYTES: usize = 64 * 1024;
pub const MAX_RESOLVED_BYTES: usize = 1024 * 1024;
pub const MAX_PRODUCTION_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_BINDINGS: usize = 1_000;
const MAX_PRONUNCIATIONS: usize = 100;
const LANGUAGES: [&str; 4] = ["vi", "vi-VN", "en", "en-US"];
const EMOTIONS: [&str; 6] = ["neutral", "calm", "hopeful", "warm", "sad", "angry"];

/// Explicit planning capabilities, not provider availability or real voice claims.
pub fn production_catalog() -> ProductionCatalogResponse {
    let first = ProductionCapability {
        provider_id: "reference-only".into(),
        model_id: "reference-v1".into(),
        voice_ids: vec!["synthetic-narrator".into(), "synthetic-character".into()],
        languages: LANGUAGES.iter().map(|s| (*s).into()).collect(),
        emotions: EMOTIONS.iter().map(|s| (*s).into()).collect(),
        rate_min_permille: 500,
        rate_max_permille: 2_000,
        pitch_min_semitones: -12,
        pitch_max_semitones: 12,
        intensity_supported: true,
        pronunciation_supported: true,
        reference_only: true,
        billable_dispatch_available: false,
    };
    let mut second = first.clone();
    second.model_id = "reference-v2".into();
    let mut capabilities = vec![
        first,
        second,
        local_contract::capability(),
        neural_contract::capability(),
    ];
    capabilities.extend(google_contract::capabilities());
    ProductionCatalogResponse {
        version: CATALOG_VERSION.into(),
        capabilities,
    }
}

fn issue(issues: &mut Vec<FieldIssue>, path: impl Into<String>, rule: &str) {
    issues.push(FieldIssue {
        path: path.into(),
        rule: rule.into(),
    });
}

fn valid_text(text: &str, max: usize) -> bool {
    !text.trim().is_empty()
        && text.len() <= max
        && !text.chars().any(char::is_control)
        && text.nfc().eq(text.chars())
}

fn valid_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 128
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}

fn valid_registry_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}

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

/// Rights are recorded declarations, not a legal verification witness.
pub fn validate_rights_declaration(
    claim: &ProductionRightsDeclaration,
) -> Result<(), Vec<FieldIssue>> {
    let mut issues = Vec::new();
    if !valid_registry_id(&claim.record_id) {
        issue(&mut issues, "claim.record_id", "id_format");
    }
    match &claim.subject {
        ProductionRightsSubject::Evidence { record_id } => {
            if record_id != &claim.record_id {
                issue(
                    &mut issues,
                    "claim.subject.record_id",
                    "rights_record_mismatch",
                );
            }
        }
        ProductionRightsSubject::Voice {
            provider_id,
            model_id,
            voice_id,
        } => {
            if !production_catalog().capabilities.iter().any(|capability| {
                capability.provider_id == *provider_id
                    && capability.model_id == *model_id
                    && capability.voice_ids.contains(voice_id)
            }) {
                issue(&mut issues, "claim.subject", "unsupported_reference_voice");
            }
        }
    }
    for (field, value, max) in [
        ("reference", claim.reference.as_str(), 512),
        ("rights_holder", claim.rights_holder.as_str(), 512),
        ("attribution", claim.attribution.as_str(), 2_048),
        ("restrictions", claim.restrictions.as_str(), 2_048),
        ("permitted_scope", claim.permitted_scope.as_str(), 512),
    ] {
        if !valid_text(value, max) {
            issue(
                &mut issues,
                format!("claim.{field}"),
                "text_length_or_normalization",
            );
        }
    }
    let mut languages = BTreeSet::new();
    if claim.languages.is_empty() || claim.languages.len() > 4 {
        issue(&mut issues, "claim.languages", "array_length");
    }
    for language in &claim.languages {
        if !(language == "all" || LANGUAGES.contains(&language.as_str()))
            || !languages.insert(language)
        {
            issue(
                &mut issues,
                "claim.languages",
                "unsupported_or_duplicate_language",
            );
        }
    }
    if languages.iter().any(|language| language.as_str() == "all") && languages.len() != 1 {
        issue(
            &mut issues,
            "claim.languages",
            "all_must_be_explicit_and_exclusive",
        );
    }
    if claim.territory != "private-planning" {
        issue(
            &mut issues,
            "claim.territory",
            "unsupported_private_planning_territory",
        );
    }
    if claim.valid_from_unix < 0
        || claim
            .valid_until_unix
            .is_some_and(|until| until <= claim.valid_from_unix)
    {
        issue(&mut issues, "claim.valid_until_unix", "invalid_term");
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

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

fn resolve_inputs(
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
        let local = binding.provider_id == local_contract::PROVIDER_ID;
        let neural = binding.provider_id == neural_contract::PROVIDER_ID;
        if local {
            if performance.emotion.as_deref() != Some("neutral")
                || performance.intensity_permille != Some(0)
            {
                issue(
                    &mut issues,
                    format!("dialogues.{}.performance", line.dialogue_id()),
                    "unsupported_control",
                );
            }
            if !local_contract::valid_text(&text) {
                issue(
                    &mut issues,
                    format!("dialogues.{}.text", line.dialogue_id()),
                    if text.len() > local_contract::MAX_TEXT_BYTES {
                        "local_text_too_large"
                    } else {
                        "invalid_local_text"
                    },
                );
            }
        }
        if neural {
            if performance.emotion.as_deref() != Some("neutral")
                || performance.intensity_permille != Some(0)
            {
                issue(
                    &mut issues,
                    format!("dialogues.{}.performance", line.dialogue_id()),
                    "unsupported_control",
                );
            }
            if !neural_contract::valid_text(&text) {
                issue(
                    &mut issues,
                    format!("dialogues.{}.text", line.dialogue_id()),
                    if text.len() > neural_contract::MAX_TEXT_BYTES {
                        "neural_text_too_large"
                    } else {
                        "invalid_neural_text"
                    },
                );
            }
        }
        // Google has no emotion or intensity control: only an explicit neutral delivery at
        // intensity 0, the voice's default, is castable. Nothing is mapped implicitly.
        let google = binding.provider_id == google_contract::PROVIDER_ID;
        if google {
            if performance.emotion.as_deref() != Some("neutral")
                || performance.intensity_permille != Some(0)
            {
                issue(
                    &mut issues,
                    format!("dialogues.{}.performance", line.dialogue_id()),
                    "unsupported_control",
                );
            }
            if !google_contract::valid_text(&text) {
                issue(
                    &mut issues,
                    format!("dialogues.{}.text", line.dialogue_id()),
                    if text.len() > google_contract::MAX_TEXT_BYTES {
                        "google_text_too_large"
                    } else {
                        "invalid_google_text"
                    },
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
            adapter_version: if local {
                local_contract::ADAPTER_VERSION
            } else if google {
                google_contract::ADAPTER_VERSION
            } else if neural {
                neural_contract::ADAPTER_VERSION
            } else {
                ADAPTER_VERSION
            }
            .into(),
            output_contract: if local {
                local_contract::OUTPUT_CONTRACT
            } else if google {
                google_contract::OUTPUT_CONTRACT
            } else if neural {
                neural_contract::OUTPUT_CONTRACT
            } else {
                OUTPUT_CONTRACT
            }
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

/// Exact rational per-dialogue planning estimate, rounded upward for this one dialogue.
pub fn estimate_cost(
    currency: ProductionCurrency,
    units: u64,
    budget: &ProductionBudget,
) -> ProductionEstimate {
    let Some(rate) = &budget.rate else {
        return ProductionEstimate::Unavailable {
            reason: "planning_rate_unrecorded".into(),
        };
    };
    if currency != budget.currency
        || units > MAX_MONEY_MINOR
        || rate.units_per_charge == 0
        || rate.units_per_charge > MAX_MONEY_MINOR
        || rate.amount_minor > MAX_MONEY_MINOR
    {
        return ProductionEstimate::Unavailable {
            reason: "invalid_or_out_of_range_rate".into(),
        };
    }
    let numerator = u128::from(units) * u128::from(rate.amount_minor);
    let denominator = u128::from(rate.units_per_charge);
    let rounded = numerator.div_ceil(denominator);
    if rounded > u128::from(MAX_MONEY_MINOR) {
        return ProductionEstimate::Unavailable {
            reason: "estimate_overflow".into(),
        };
    }
    ProductionEstimate::Known {
        currency,
        amount_minor: rounded as u64,
        units,
        rate_reference: rate.reference.clone(),
        rate_version: rate.version.clone(),
    }
}

type RequiredRights = BTreeMap<String, ProductionRightsSubject>;

fn required_rights(
    script: &ScriptContent,
    settings: &ProductionSettings,
) -> Result<RequiredRights, Vec<FieldIssue>> {
    let mut required = BTreeMap::new();
    for (kind, id) in script.evidence_refs() {
        if kind == "rights" {
            required.insert(
                id.into(),
                ProductionRightsSubject::Evidence {
                    record_id: id.into(),
                },
            );
        }
    }
    let mut issues = Vec::new();
    for binding in &settings.bindings {
        let subject = ProductionRightsSubject::Voice {
            provider_id: binding.provider_id.clone(),
            model_id: binding.model_id.clone(),
            voice_id: binding.voice_id.clone(),
        };
        if required
            .get(&binding.voice_rights_record_id)
            .is_some_and(|prior| prior != &subject)
        {
            issue(
                &mut issues,
                format!("bindings.{}.voice_rights_record_id", binding.character_id),
                "rights_subject_conflict",
            );
        }
        required.insert(binding.voice_rights_record_id.clone(), subject);
    }
    if issues.is_empty() {
        Ok(required)
    } else {
        Err(issues)
    }
}

fn finding(
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

fn rights_findings(
    required: &RequiredRights,
    rights: &[ProductionRightsClaimResponse],
    language: &str,
    budget: &ProductionBudget,
    now_unix: i64,
) -> Vec<ProductionFinding> {
    let mut findings = Vec::new();
    for (id, subject) in required {
        let path = format!("rights.{id}");
        let Some(response) = rights
            .iter()
            .find(|right| &right.claim.declaration.record_id == id)
        else {
            finding(
                &mut findings,
                ProductionFindingCode::RightsMissing,
                path,
                "No persisted production claim covers this required rights record.",
            );
            continue;
        };
        let claim = &response.claim.declaration;
        if &claim.subject != subject {
            finding(
                &mut findings,
                ProductionFindingCode::RightsSubjectMismatch,
                &path,
                "The claim covers another source or voice subject.",
            );
        }
        match claim.status {
            ProductionRightsStatus::Pending => finding(
                &mut findings,
                ProductionFindingCode::RightsPending,
                &path,
                "The recorded claim is pending.",
            ),
            ProductionRightsStatus::Revoked => finding(
                &mut findings,
                ProductionFindingCode::RightsRevoked,
                &path,
                "The recorded claim is revoked.",
            ),
            ProductionRightsStatus::Granted => {}
        }
        if now_unix < claim.valid_from_unix {
            finding(
                &mut findings,
                ProductionFindingCode::RightsNotYetValid,
                &path,
                "The recorded term has not begun.",
            );
        }
        if claim
            .valid_until_unix
            .is_some_and(|until| now_unix >= until)
        {
            finding(
                &mut findings,
                ProductionFindingCode::RightsExpired,
                &path,
                "The recorded term has expired.",
            );
        }
        if !claim
            .languages
            .iter()
            .any(|tag| tag == language || tag == "all")
        {
            finding(
                &mut findings,
                ProductionFindingCode::RightsLanguageMismatch,
                &path,
                "The recorded claim does not cover the exact script language.",
            );
        }
        if claim.territory != budget.territory {
            finding(
                &mut findings,
                ProductionFindingCode::RightsTerritoryMismatch,
                &path,
                "The recorded claim does not cover the planning territory.",
            );
        }
        if claim.permitted_scope != budget.scope {
            finding(
                &mut findings,
                ProductionFindingCode::RightsScopeMismatch,
                &path,
                "The recorded claim does not cover the exact production budget scope.",
            );
        }
        if !["none-declared", "private-planning-only"].contains(&claim.restrictions.as_str()) {
            finding(&mut findings, ProductionFindingCode::RightsRestrictionsRequireReview, &path, "This restriction is not supported by the bounded private planning policy; review is required.");
        }
    }
    findings
}

/// Sealed validated candidate: constructed only by deterministic admission and resolution.
pub struct PreparedProduction {
    document: FrozenProductionDocument,
    findings: Vec<ProductionFinding>,
}

impl PreparedProduction {
    pub fn document(&self) -> FrozenProductionDocument {
        self.document.clone()
    }

    pub fn preview(&self) -> ProductionPreviewResponse {
        ProductionPreviewResponse {
            input_digest: self.document.input_digest.clone(),
            script_revision: self.document.revision.revision,
            settings_version: self.document.settings.version,
            resolved: self.document.resolved.clone(),
            estimate: self.document.estimate.clone(),
            inputs_eligible: self.findings.is_empty(),
            findings: self.findings.clone(),
            billable_dispatch_available: false,
        }
    }
}

/// The shell supplies current persisted claims, exact editorial review and one clock instant.
pub fn prepare_inputs(
    owner_id: &str,
    revision: &RevisionResponse,
    settings: &ProductionSettingsResponse,
    rights: &[ProductionRightsClaimResponse],
    script_reviewed: bool,
    now_unix: i64,
) -> Result<PreparedProduction, Vec<FieldIssue>> {
    validate_settings(&settings.settings)?;
    let mut issues = Vec::new();
    if !valid_registry_id(owner_id) {
        issue(&mut issues, "owner_id", "id_format");
    }
    if revision.revision == 0
        || revision.revision > MAX_MONEY_MINOR
        || settings.version == 0
        || settings.version > MAX_MONEY_MINOR
        || revision.script_id != settings.script_id
        || revision.revision != settings.script_revision
    {
        issue(
            &mut issues,
            "settings.script_revision",
            "revision_scope_mismatch",
        );
    }
    let script = read_canonical_script(revision.script_json.as_bytes()).map_err(|_| {
        vec![FieldIssue {
            path: "revision.script_json".into(),
            rule: "invalid_canonical_export".into(),
        }]
    })?;
    let mut export_hash = Sha256::new();
    export_hash.update(b"cantos/script-export/e1\n");
    export_hash.update(revision.script_json.as_bytes());
    if script.content_digest().to_string() != revision.content_digest
        || format!("sir-e1:sha256:{:x}", export_hash.finalize()) != revision.export_digest
    {
        issue(&mut issues, "revision", "digest_mismatch");
    }
    let required = required_rights(&script, &settings.settings)?;
    let mut rights: Vec<_> = rights
        .iter()
        .filter(|right| required.contains_key(&right.claim.declaration.record_id))
        .cloned()
        .collect();
    rights.sort_by(|a, b| {
        a.claim
            .declaration
            .record_id
            .cmp(&b.claim.declaration.record_id)
    });
    let mut seen = BTreeSet::new();
    for right in &rights {
        validate_rights_declaration(&right.claim.declaration)?;
        if right.claim.version == 0
            || right.claim.version > MAX_MONEY_MINOR
            || !seen.insert(&right.claim.declaration.record_id)
        {
            issue(&mut issues, "rights", "invalid_or_duplicate_current_claim");
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    let resolved = resolve_inputs(&script, &settings.settings)?;
    let mut settings = settings.clone();
    settings
        .settings
        .bindings
        .sort_by(|a, b| a.character_id.cmp(&b.character_id));
    for binding in &mut settings.settings.bindings {
        binding
            .pronunciation
            .sort_by(|a, b| a.surface.cmp(&b.surface));
    }
    let language = resolved
        .first()
        .map(|line| line.language.as_str())
        .unwrap_or("vi");
    let mut findings = rights_findings(
        &required,
        &rights,
        language,
        &settings.settings.budget,
        now_unix,
    );
    if !script_reviewed {
        finding(
            &mut findings,
            ProductionFindingCode::ScriptUnreviewed,
            "revision",
            "The accepted script revision has no exact owner editorial review.",
        );
    }
    let estimate = estimate_dialogues(&resolved, &settings.settings.budget);
    match &estimate {
        ProductionEstimate::Unavailable { .. } => finding(
            &mut findings,
            ProductionFindingCode::EstimateUnavailable,
            "budget.rate",
            "No bounded planning estimate is available; absence is not zero.",
        ),
        ProductionEstimate::Known { amount_minor, .. }
            if *amount_minor > settings.settings.budget.limit_minor =>
        {
            finding(
                &mut findings,
                ProductionFindingCode::BudgetExceeded,
                "budget.limit_minor",
                "The upward-rounded planning estimate exceeds the recorded limit.",
            )
        }
        ProductionEstimate::Known { .. } => {}
    }
    let mut document = FrozenProductionDocument {
        owner_id: owner_id.into(),
        catalog_version: CATALOG_VERSION.into(),
        revision: revision.clone(),
        settings,
        rights,
        resolved,
        estimate,
        input_digest: String::new(),
    };
    document.input_digest = production_input_digest(&document)?;
    if serde_json::to_vec(&document)
        .map_or(true, |bytes| bytes.len() > MAX_PRODUCTION_DOCUMENT_BYTES)
    {
        return Err(vec![FieldIssue {
            path: "document".into(),
            rule: "production_document_too_large".into(),
        }]);
    }
    Ok(PreparedProduction { document, findings })
}

/// A dialogue is the independent future send unit. Round each line before summing.
pub fn estimate_dialogues(
    lines: &[ResolvedSynthesisInput],
    budget: &ProductionBudget,
) -> ProductionEstimate {
    let mut total_units = 0_u64;
    let mut total_minor = 0_u64;
    for line in lines {
        let units = line.text.chars().count() as u64;
        match estimate_cost(budget.currency, units, budget) {
            ProductionEstimate::Unavailable { reason } => {
                return ProductionEstimate::Unavailable { reason }
            }
            ProductionEstimate::Known { amount_minor, .. } => {
                let Some(sum_units) = total_units
                    .checked_add(units)
                    .filter(|value| *value <= MAX_MONEY_MINOR)
                else {
                    return ProductionEstimate::Unavailable {
                        reason: "usage_overflow".into(),
                    };
                };
                let Some(sum_minor) = total_minor
                    .checked_add(amount_minor)
                    .filter(|value| *value <= MAX_MONEY_MINOR)
                else {
                    return ProductionEstimate::Unavailable {
                        reason: "estimate_overflow".into(),
                    };
                };
                total_units = sum_units;
                total_minor = sum_minor;
            }
        }
    }
    let Some(rate) = &budget.rate else {
        return ProductionEstimate::Unavailable {
            reason: "planning_rate_unrecorded".into(),
        };
    };
    ProductionEstimate::Known {
        currency: budget.currency,
        amount_minor: total_minor,
        units: total_units,
        rate_reference: rate.reference.clone(),
        rate_version: rate.version.clone(),
    }
}

/// Recursive lexicographic compact UTF-8 JSON, excluding only the digest field itself.
pub fn production_input_digest(
    document: &FrozenProductionDocument,
) -> Result<String, Vec<FieldIssue>> {
    let mut value = serde_json::to_value(document).map_err(|_| {
        vec![FieldIssue {
            path: "document".into(),
            rule: "serialization_failed".into(),
        }]
    })?;
    if let Value::Object(map) = &mut value {
        map.remove("input_digest");
    }
    let mut bytes = Vec::new();
    canonical_json(&value, &mut bytes).map_err(|_| {
        vec![FieldIssue {
            path: "document".into(),
            rule: "serialization_failed".into(),
        }]
    })?;
    let mut hash = Sha256::new();
    hash.update(b"cantos/production-inputs/p1\n");
    hash.update(bytes);
    Ok(format!("production-p1:sha256:{:x}", hash.finalize()))
}

fn canonical_json(value: &Value, bytes: &mut Vec<u8>) -> Result<(), serde_json::Error> {
    match value {
        Value::Object(map) => {
            bytes.push(b'{');
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    bytes.push(b',');
                }
                bytes.extend(serde_json::to_vec(key)?);
                bytes.push(b':');
                canonical_json(&map[key], bytes)?;
            }
            bytes.push(b'}');
        }
        Value::Array(values) => {
            bytes.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    bytes.push(b',');
                }
                canonical_json(value, bytes)?;
            }
            bytes.push(b']');
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            bytes.extend(serde_json::to_vec(value)?)
        }
    }
    Ok(())
}

/// Staleness is derived from current facts; immutable snapshots/approval rows never mutate.
pub fn evaluate_snapshot(
    document: &FrozenProductionDocument,
    current_revision: u64,
    current_settings_version: u64,
    current_rights: &[ProductionRightsClaimResponse],
    script_reviewed: bool,
    approval: Option<&ProductionApprovalResponse>,
    now_unix: i64,
) -> ProductionEligibility {
    let prepared = prepare_inputs(
        &document.owner_id,
        &document.revision,
        &document.settings,
        current_rights,
        script_reviewed,
        now_unix,
    );
    let mut findings = match &prepared {
        Ok(candidate) => candidate.findings.clone(),
        Err(_) => vec![ProductionFinding {
            code: ProductionFindingCode::RightsChanged,
            path: "document".into(),
            detail: "Current input facts cannot be admitted under this frozen scope.".into(),
        }],
    };
    if current_revision != document.revision.revision {
        finding(
            &mut findings,
            ProductionFindingCode::RevisionChanged,
            "revision",
            "A newer accepted script revision requires a new candidate and approval.",
        );
    }
    if current_settings_version != document.settings.version {
        finding(
            &mut findings,
            ProductionFindingCode::SettingsChanged,
            "settings.version",
            "Casting or budget settings changed; approval must be reevaluated.",
        );
    }
    if prepared
        .as_ref()
        .is_ok_and(|candidate| candidate.document.rights != document.rights)
    {
        finding(
            &mut findings,
            ProductionFindingCode::RightsChanged,
            "rights",
            "A required rights claim revision changed; approval must be reevaluated.",
        );
    }
    let pinned = prepare_inputs(
        &document.owner_id,
        &document.revision,
        &document.settings,
        &document.rights,
        true,
        now_unix,
    );
    if pinned
        .as_ref()
        .map_or(true, |candidate| candidate.document != *document)
    {
        finding(
            &mut findings,
            ProductionFindingCode::ApprovalScopeMismatch,
            "document.input_digest",
            "The frozen document differs from the admitted exact input identity.",
        );
    }
    let inputs_eligible = findings.is_empty();
    let approval_matches = approval.is_some_and(|approval| {
        approval.input_digest == document.input_digest && approval.approved_by == document.owner_id
    });
    match approval {
        None => finding(
            &mut findings,
            ProductionFindingCode::ApprovalMissing,
            "approval",
            "No current owner input/cost approval is recorded for this snapshot.",
        ),
        Some(_) if !approval_matches => finding(
            &mut findings,
            ProductionFindingCode::ApprovalScopeMismatch,
            "approval.input_digest",
            "The approval does not cover this exact input scope and owner.",
        ),
        Some(_) => {}
    }
    ProductionEligibility {
        inputs_eligible,
        approval_current: inputs_eligible && approval_matches,
        billable_dispatch_available: false,
        findings,
    }
}

/// A sealed witness for current input authorization. No paid dispatcher consumes it yet.
pub struct AuthorizedProductionInputs {
    input_digest: String,
}

impl AuthorizedProductionInputs {
    pub fn input_digest(&self) -> &str {
        &self.input_digest
    }
}

pub fn authorize_inputs(
    document: &FrozenProductionDocument,
    current_revision: u64,
    current_settings_version: u64,
    current_rights: &[ProductionRightsClaimResponse],
    script_reviewed: bool,
    approval: Option<&ProductionApprovalResponse>,
    now_unix: i64,
) -> Result<AuthorizedProductionInputs, Vec<ProductionFinding>> {
    let eligibility = evaluate_snapshot(
        document,
        current_revision,
        current_settings_version,
        current_rights,
        script_reviewed,
        approval,
        now_unix,
    );
    if eligibility.approval_current {
        Ok(AuthorizedProductionInputs {
            input_digest: document.input_digest.clone(),
        })
    } else {
        Err(eligibility.findings)
    }
}
