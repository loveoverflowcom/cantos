//! The closed caller proposal wire format and its bounded decoder. Nothing here mints identity.
use std::collections::BTreeSet;

use cantos_contracts::AdaptationProblem;
use serde::Deserialize;

use super::problem;
use super::request::CONTRACT_VERSION;

pub const MAX_OUTPUT_BYTES: usize = 256 * 1024;
pub const MAX_NODES: usize = 1000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawProposal {
    pub(super) contract_version: String,
    pub(super) title: String,
    pub(super) characters: Vec<RawCharacter>,
    pub(super) scenes: Vec<RawScene>,
    pub(super) omitted_blocks: Vec<RawOmission>,
    pub(super) review_notes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCharacter {
    pub(super) name: String,
    pub(super) personality: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawScene {
    pub(super) title: String,
    pub(super) lines: Vec<RawLine>,
    pub(super) cues: Vec<RawCue>,
    #[serde(default, deserialize_with = "optional_note")]
    pub(super) pacing_note: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawLine {
    #[serde(deserialize_with = "nullable_speaker")]
    pub(super) speaker: Option<String>,
    pub(super) text: String,
    pub(super) source_blocks: Vec<u32>,
    pub(super) delivery: RawDelivery,
    #[serde(default, deserialize_with = "optional_note")]
    pub(super) prosody_note: Option<String>,
}

fn nullable_speaker<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(decoder)
}

fn optional_note<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Option<String>, D::Error> {
    String::deserialize(decoder).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawDelivery {
    pub(super) emotion: String,
    pub(super) intensity_permille: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCue {
    pub(super) kind: String,
    pub(super) description: String,
    pub(super) line_index: usize,
    pub(super) edge: String,
    pub(super) source_blocks: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawOmission {
    pub(super) block: u32,
    pub(super) reason: String,
}

fn bounded_text(text: &str, max: usize) -> bool {
    !text.trim().is_empty()
        && text.len() <= max
        && !text
            .chars()
            .any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
}

pub(super) fn decode(bytes: &[u8]) -> Result<RawProposal, AdaptationProblem> {
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(problem("output_too_large", "/", "byte_length"));
    }
    let raw: RawProposal = serde_json::from_slice(bytes)
        .map_err(|_| problem("invalid_output", "/", "json_contract"))?;
    if raw.contract_version != CONTRACT_VERSION {
        return Err(problem(
            "unsupported_output_contract",
            "/contract_version",
            "version",
        ));
    }
    let nodes = raw.scenes.len()
        + raw.characters.len()
        + raw
            .scenes
            .iter()
            .map(|scene| scene.lines.len() + scene.cues.len())
            .sum::<usize>();
    let citations = raw
        .scenes
        .iter()
        .map(|scene| {
            scene
                .lines
                .iter()
                .map(|line| line.source_blocks.len())
                .sum::<usize>()
                + scene
                    .cues
                    .iter()
                    .map(|cue| cue.source_blocks.len())
                    .sum::<usize>()
        })
        .sum::<usize>();
    if nodes > MAX_NODES || citations > 1024 || raw.scenes.is_empty() || raw.scenes.len() > 64 {
        return Err(problem("invalid_output", "/scenes", "node_limit"));
    }
    if !bounded_text(&raw.title, 1000)
        || raw.review_notes.len() > 128
        || raw
            .review_notes
            .iter()
            .any(|note| !bounded_text(note, 2048))
        || raw.characters.len() > 128
    {
        return Err(problem("invalid_output", "/", "text_or_count_limit"));
    }
    let mut names = BTreeSet::new();
    for character in &raw.characters {
        if !bounded_text(&character.name, 512)
            || !bounded_text(&character.personality, 2048)
            || !names.insert(&character.name)
        {
            return Err(problem(
                "invalid_output",
                "/characters",
                "unique_bounded_names",
            ));
        }
    }
    for scene in &raw.scenes {
        if !bounded_text(&scene.title, 1000) || scene.lines.is_empty() {
            return Err(problem("invalid_output", "/scenes", "nonempty_scene"));
        }
        if scene
            .pacing_note
            .as_ref()
            .is_some_and(|note| !bounded_text(note, 2048))
        {
            return Err(problem(
                "invalid_output",
                "/scenes/pacing_note",
                "text_limit",
            ));
        }
        for line in &scene.lines {
            if !bounded_text(&line.text, 16384)
                || line
                    .speaker
                    .as_ref()
                    .is_some_and(|name| !bounded_text(name, 512))
                || line
                    .prosody_note
                    .as_ref()
                    .is_some_and(|note| !bounded_text(note, 2048))
                || line.source_blocks.is_empty()
                || line.source_blocks.len() > 256
            {
                return Err(problem(
                    "invalid_output",
                    "/scenes/lines",
                    "bounded_line_and_citations",
                ));
            }
        }
        for cue in &scene.cues {
            if !bounded_text(&cue.description, 2048)
                || cue.line_index >= scene.lines.len()
                || cue.source_blocks.is_empty()
                || cue.source_blocks.len() > 256
            {
                return Err(problem(
                    "invalid_output",
                    "/scenes/cues",
                    "bounded_cue_and_anchor",
                ));
            }
        }
    }
    if raw.omitted_blocks.len() > 256
        || raw
            .omitted_blocks
            .iter()
            .any(|item| !bounded_text(&item.reason, 2048))
    {
        return Err(problem(
            "invalid_output",
            "/omitted_blocks",
            "bounded_omissions",
        ));
    }
    Ok(raw)
}
