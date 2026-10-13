//! Deterministic materialization of a decoded proposal into admitted Script IR with findings.
//! The caller supplies every opaque identity, so the same inputs always produce the same output.
use cantos_contracts::{
    AdaptationCoverage, AdaptationCoverageDisposition, AdaptationFinding, AdaptationFindingCode,
    AdaptationProblem, AdaptationProposal, ImportBlockKind, ImportResponse,
};

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use super::evidence::{check_binding, BoundSource};
use super::proposal::{decode, RawProposal};
use super::{array, extracted, problem, script_problem, text, TrustedBinding};
use crate::script_ir::read_script;

pub const MAX_FINDINGS: usize = 2000;

fn finding(code: AdaptationFindingCode, block: Option<u32>, detail: &str) -> AdaptationFinding {
    AdaptationFinding {
        code,
        block,
        detail: detail.into(),
    }
}

/// Admission with identities from `mint`: one per node the materializer can need, minted after
/// decoding and before materialization so they never depend on model text or array position.
pub fn admit_output_with_minter(
    bytes: &[u8],
    source: &ImportResponse,
    binding: &TrustedBinding,
    mut mint: impl FnMut() -> String,
) -> Result<AdaptationProposal, AdaptationProblem> {
    let raw = decode(bytes)?;
    let count = 8
        + raw.characters.len()
        + raw
            .scenes
            .iter()
            .map(|scene| 1 + scene.lines.len() * 2 + scene.cues.len())
            .sum::<usize>();
    let ids: Vec<String> = (0..count).map(|_| mint()).collect();
    materialize(raw, source, binding, &ids)
}

/// Deterministic oracle entry: callers supply independently minted, unique opaque identities.
pub fn admit_output_with_ids(
    bytes: &[u8],
    source: &ImportResponse,
    binding: &TrustedBinding,
    ids: &[String],
) -> Result<AdaptationProposal, AdaptationProblem> {
    materialize(decode(bytes)?, source, binding, ids)
}

fn materialize(
    raw: RawProposal,
    source: &ImportResponse,
    binding: &TrustedBinding,
    ids: &[String],
) -> Result<AdaptationProposal, AdaptationProblem> {
    let admitted = check_binding(source, binding)?;
    materialize_bound(raw, admitted, ids)
}

fn materialize_bound(
    raw: RawProposal,
    admitted: BoundSource<'_>,
    ids: &[String],
) -> Result<AdaptationProposal, AdaptationProblem> {
    let source = admitted.source();
    let binding = admitted.facts();
    let extraction = extracted(source)?;
    let mut identities = ids.iter();
    let mut next_id = || {
        identities
            .next()
            .cloned()
            .ok_or_else(|| problem("identity_pool_exhausted", "/", "minted_ids"))
    };
    let mut script = if let Some(base) = &binding.base_script_json {
        let content = read_script(base.as_bytes()).map_err(script_problem)?;
        serde_json::from_slice::<Value>(&content.export_bytes())
            .map_err(|_| problem("invalid_base", "/expected_revision", "canonical"))?
    } else {
        json!({"schema_version":"0.1.0",
            "work":{"id":next_id()?,"title":raw.title,"source_ref":"pending","rights_record_id":binding.rights_record_id},
            "adaptation":{"id":next_id()?,"language":"vi-VN","rights_record_id":binding.rights_record_id,"provenance_refs":[]},
            "characters":[],"episode":{"id":next_id()?,"title":raw.title,"acts":[]},"provenance":[]})
    };
    let origin = format!("origin_{}", binding.id_namespace);
    let generated = format!("generated_{}", binding.id_namespace);
    if binding.base_script_json.is_none() {
        script["work"]["source_ref"] = json!(origin);
    }
    let provenance = script["provenance"]
        .as_array_mut()
        .ok_or_else(|| problem("invalid_base", "/provenance", "array"))?;
    provenance.push(json!({"id":origin,"kind":"imported","source_record_id":binding.source_id,"rights_record_id":binding.rights_record_id}));
    provenance.push(json!({"id":generated,"kind":"generated","source_record_id":binding.source_id,"rights_record_id":binding.rights_record_id,"generation_record_id":binding.generation_record_id}));
    script["adaptation"]["rights_record_id"] = json!(binding.rights_record_id);
    let refs: Vec<Value> = array(&script["provenance"])?
        .iter()
        .map(|item| item["id"].clone())
        .collect();
    script["adaptation"]["provenance_refs"] = json!(refs);
    script["episode"]["title"] = json!(raw.title);
    let mut findings: Vec<AdaptationFinding> = extraction
        .warnings
        .iter()
        .map(|warning| {
            finding(
                AdaptationFindingCode::SourceWarning,
                warning.block,
                &warning.code,
            )
        })
        .collect();
    findings.push(finding(AdaptationFindingCode::SourceCoverageRequiresReview, None,
        "Citations record the model's claimed coverage; an editor must compare source facts, chronology and attribution."));
    let chars = script["characters"]
        .as_array_mut()
        .ok_or_else(|| problem("invalid_base", "/characters", "array"))?;
    let mut speakers: BTreeMap<String, String> = chars
        .iter()
        .filter_map(|item| Some((item["name"].as_str()?.into(), item["id"].as_str()?.into())))
        .collect();
    let existing_narrator = chars
        .iter()
        .find(|item| item["role"] == "narrator")
        .and_then(|item| item["id"].as_str())
        .map(str::to_owned);
    let narrator = match existing_narrator {
        Some(id) => id,
        None => {
            let id = next_id()?;
            chars.push(json!({"id":id,"name":"Người dẫn chuyện","role":"narrator","personality":"Narration proposed by AI; requires source review."}));
            id
        }
    };
    speakers.insert("Người dẫn chuyện".into(), narrator.clone());
    for character in raw.characters {
        if !speakers.contains_key(&character.name) {
            let id = next_id()?;
            speakers.insert(character.name.clone(), id.clone());
            chars.push(json!({"id":id,"name":character.name,"role":"character","personality":character.personality}));
        }
    }
    let source_labels: BTreeSet<&str> = extraction
        .blocks
        .iter()
        .filter_map(|block| block.speaker.as_deref())
        .collect();
    let mut coverage_map: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    let mut scenes = Vec::new();
    for scene in raw.scenes {
        let mut lines = Vec::new();
        for line in scene.lines {
            for block in &line.source_blocks {
                if extraction
                    .blocks
                    .get(*block as usize)
                    .is_none_or(|item| item.index != *block)
                {
                    return Err(problem(
                        "invalid_source_reference",
                        "/scenes/lines/source_blocks",
                        "existing_block",
                    ));
                }
            }
            let name = line.speaker.as_deref().unwrap_or("Người nói chưa xác định");
            let speaker = if name == "Người dẫn chuyện" {
                narrator.clone()
            } else {
                if !speakers.contains_key(name) {
                    let id = next_id()?;
                    speakers.insert(name.into(), id.clone());
                    chars.push(json!({"id":id,"name":name,"role":"character","personality":"Attribution unresolved; creator confirmation required."}));
                }
                speakers[name].clone()
            };
            if line.speaker.is_none() {
                findings.push(finding(
                    AdaptationFindingCode::UnresolvedSpeaker,
                    line.source_blocks.first().copied(),
                    "Speaker is unknown; the placeholder is not a resolved attribution.",
                ));
            } else if name != "Người dẫn chuyện" && !source_labels.contains(name) {
                findings.push(finding(
                    AdaptationFindingCode::NewSpeaker,
                    line.source_blocks.first().copied(),
                    name,
                ));
            }
            let id = next_id()?;
            for block in &line.source_blocks {
                let source_block = &extraction.blocks[*block as usize];
                coverage_map.entry(*block).or_default().push(id.clone());
                if source_block.text != line.text {
                    findings.push(finding(AdaptationFindingCode::TextChanged, Some(*block), "Adapted spoken text differs from its cited source block; compare meaning and omissions."));
                }
                if (source_block.kind == ImportBlockKind::Dialogue && name == "Người dẫn chuyện")
                    || (source_block.kind == ImportBlockKind::Narration
                        && name != "Người dẫn chuyện")
                {
                    findings.push(finding(
                        AdaptationFindingCode::PossibleNarrationConfusion,
                        Some(*block),
                        "Source type and proposed speaker role differ.",
                    ));
                }
            }
            if line.text.contains(['[', ']', '<', '>']) {
                findings.push(finding(AdaptationFindingCode::CueLikeMarkup, line.source_blocks.first().copied(), "Spoken text contains markup-like characters; review whether they are spoken or a separate cue."));
            }
            if let Some(note) = line.prosody_note {
                findings.push(finding(
                    AdaptationFindingCode::UnsupportedPerformanceControl,
                    line.source_blocks.first().copied(),
                    &note,
                ));
            }
            lines.push(json!({"id":id,"speaker_id":speaker,"text":line.text,
                "delivery":{"emotion":line.delivery.emotion,"intensity_permille":line.delivery.intensity_permille}}));
        }
        let mut cues = Vec::new();
        for cue in scene.cues {
            for block in &cue.source_blocks {
                if extraction
                    .blocks
                    .get(*block as usize)
                    .is_none_or(|item| item.index != *block)
                {
                    return Err(problem(
                        "invalid_source_reference",
                        "/scenes/cues/source_blocks",
                        "existing_block",
                    ));
                }
                coverage_map
                    .entry(*block)
                    .or_default()
                    .push(text(&lines[cue.line_index]["id"])?.into());
            }
            cues.push(
                json!({"id":next_id()?,"kind":cue.kind,"description":cue.description,
                "anchor":{"dialogue_id":lines[cue.line_index]["id"],"edge":cue.edge}}),
            );
        }
        if let Some(note) = scene.pacing_note {
            findings.push(finding(
                AdaptationFindingCode::UnsupportedPerformanceControl,
                None,
                &note,
            ));
        }
        scenes
            .push(json!({"id":next_id()?,"title":scene.title,"dialogues":lines,"sound_cues":cues}));
    }
    let mut omissions = BTreeMap::new();
    for omitted in raw.omitted_blocks {
        if extraction
            .blocks
            .get(omitted.block as usize)
            .is_none_or(|item| item.index != omitted.block)
            || coverage_map.contains_key(&omitted.block)
            || omissions.insert(omitted.block, omitted.reason).is_some()
        {
            return Err(problem(
                "invalid_source_reference",
                "/omitted_blocks",
                "unique_unrepresented_block",
            ));
        }
    }
    let mut coverage = Vec::new();
    for block in &extraction.blocks {
        if let Some(mut dialogue_ids) = coverage_map.remove(&block.index) {
            dialogue_ids.sort();
            dialogue_ids.dedup();
            coverage.push(AdaptationCoverage {
                block: block.index,
                dialogue_ids,
                disposition: AdaptationCoverageDisposition::Represented,
                reason: None,
            });
        } else {
            let reason = omissions.remove(&block.index);
            findings.push(finding(
                if reason.is_some() {
                    AdaptationFindingCode::OmittedSource
                } else {
                    AdaptationFindingCode::UncoveredSource
                },
                Some(block.index),
                reason
                    .as_deref()
                    .unwrap_or("No proposed line or cue cites this source block."),
            ));
            coverage.push(AdaptationCoverage {
                block: block.index,
                dialogue_ids: vec![],
                disposition: AdaptationCoverageDisposition::Omitted,
                reason,
            });
        }
    }
    findings.extend(
        raw.review_notes
            .iter()
            .map(|note| finding(AdaptationFindingCode::ProviderReviewNote, None, note)),
    );
    if findings.len() > MAX_FINDINGS {
        return Err(problem("output_findings_too_large", "/", "finding_limit"));
    }
    script["episode"]["acts"] =
        json!([{"id":next_id()?,"title":"Adaptation proposal","scenes":scenes}]);
    let bytes = serde_json::to_vec(&script).map_err(|_| problem("invalid_output", "/", "json"))?;
    let content = read_script(&bytes).map_err(script_problem)?;
    Ok(AdaptationProposal {
        script_json: String::from_utf8(content.export_bytes())
            .map_err(|_| problem("invalid_output", "/", "utf8"))?,
        findings,
        coverage,
    })
}
