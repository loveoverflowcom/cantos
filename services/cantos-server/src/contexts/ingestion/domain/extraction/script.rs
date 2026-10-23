//! A complete Script IR document: admitted by the strict validator, then projected into
//! reviewable blocks while the canonical document is preserved whole.
use std::collections::BTreeMap;

use crate::contract::studio::{Extraction, ImportBlockKind, ImportCueKind, ImportProblem};
use serde::Deserialize;

use super::prose::decode_text;
use super::{problem, Builder};
use crate::script_ir::{read_script, ReadError};

// The complete document first crosses the existing strict validator. These DTOs are only a
// projection of its canonical export, never a second Script IR admission path.
#[derive(Deserialize)]
struct ScriptProjection {
    characters: Vec<CharacterProjection>,
    episode: EpisodeProjection,
}

#[derive(Deserialize)]
struct CharacterProjection {
    id: String,
    name: String,
    role: String,
}

#[derive(Deserialize)]
struct EpisodeProjection {
    acts: Vec<ActProjection>,
}

#[derive(Deserialize)]
struct ActProjection {
    scenes: Vec<SceneProjection>,
}

#[derive(Deserialize)]
struct SceneProjection {
    id: String,
    title: String,
    dialogues: Vec<DialogueProjection>,
    #[serde(default)]
    sound_cues: Vec<CueProjection>,
}

#[derive(Deserialize)]
struct DialogueProjection {
    id: String,
    speaker_id: String,
    text: String,
}

#[derive(Deserialize)]
struct CueProjection {
    kind: ImportCueKind,
    description: String,
    anchor: AnchorProjection,
}

#[derive(Deserialize)]
struct AnchorProjection {
    dialogue_id: String,
    edge: String,
}

pub(super) fn extract_script(bytes: &[u8]) -> Result<Extraction, ImportProblem> {
    let mut output = Builder::new();
    let decoded = decode_text(bytes, &mut output)?;
    let script = read_script(decoded.as_bytes()).map_err(|error| match error {
        ReadError::UnsupportedSchemaVersion { .. } => problem("unsupported_script_ir_version"),
        ReadError::MissingSchemaVersion => problem("missing_script_ir_version"),
        ReadError::DocumentTooLarge { .. } => problem("decoded_text_limit"),
        ReadError::Shape(_) => problem("script_ir_shape_invalid"),
        ReadError::Semantic(_) => problem("script_ir_semantics_invalid"),
        ReadError::InvalidDocument { .. } | ReadError::NonCanonicalDocument => {
            problem("invalid_script_ir")
        }
    })?;
    let canonical = script.export_bytes();
    let projection: ScriptProjection =
        serde_json::from_slice(&canonical).map_err(|_| problem("script_ir_projection_failed"))?;
    let characters: BTreeMap<_, _> = projection
        .characters
        .into_iter()
        .map(|character| (character.id.clone(), character))
        .collect();
    for act in projection.episode.acts {
        for scene in act.scenes {
            output.push(
                ImportBlockKind::SceneCue,
                scene.title,
                None,
                Some(scene.id.clone()),
                None,
            )?;
            let mut cues = BTreeMap::<(&str, &str), Vec<&CueProjection>>::new();
            for cue in &scene.sound_cues {
                cues.entry((&cue.anchor.dialogue_id, &cue.anchor.edge))
                    .or_default()
                    .push(cue);
            }
            for dialogue in scene.dialogues {
                if let Some(start) = cues.get(&(dialogue.id.as_str(), "start")) {
                    for cue in start {
                        push_cue(cue, &scene.id, &mut output)?;
                    }
                }
                let character = characters
                    .get(&dialogue.speaker_id)
                    .ok_or_else(|| problem("script_ir_projection_failed"))?;
                output.push(
                    if character.role == "narrator" {
                        ImportBlockKind::Narration
                    } else {
                        ImportBlockKind::Dialogue
                    },
                    dialogue.text,
                    Some(character.name.clone()),
                    Some(scene.id.clone()),
                    None,
                )?;
                if let Some(end) = cues.get(&(dialogue.id.as_str(), "end")) {
                    for cue in end {
                        push_cue(cue, &scene.id, &mut output)?;
                    }
                }
            }
        }
    }
    output.warn("script_ir_full_document_preserved", None);
    let script_json =
        String::from_utf8(canonical).map_err(|_| problem("script_ir_projection_failed"))?;
    output.finish(Some(script_json))
}

fn push_cue(cue: &CueProjection, scene: &str, output: &mut Builder) -> Result<(), ImportProblem> {
    output.push(
        ImportBlockKind::SoundCue,
        cue.description.clone(),
        None,
        Some(scene.into()),
        Some(cue.kind),
    )?;
    Ok(())
}
