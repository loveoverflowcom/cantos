use std::collections::{BTreeMap, BTreeSet};

use unicode_normalization::UnicodeNormalization;

use super::model::{
    Act, Adaptation, Asset, Character, CharacterId, Cue, Delivery, Dialogue, DialogueId, Episode,
    Id, Pronunciation, Role, Scene, ScriptContent, Source, SourceKind, Text, Work,
};
use super::wire::{RawAct, RawCue, RawDialogue, RawEpisode, RawScene, RawScript, RawSource};

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    pub path: String,
    pub issue: ValidationIssue,
}

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum ValidationIssue {
    DuplicateId { id: String, first_seen: String },
    NarratorCount { found: usize },
    UnknownSpeaker { speaker_id: String },
    CueAnchorUnresolved { dialogue_id: String },
    UnknownProvenance { source_id: String },
    DuplicateProvenanceRef { source_id: String },
    EmptyText,
    NormalizedTextTooLong { max_chars: usize },
    ForbiddenCharacter { code_point: u32 },
    PronunciationTargetMissing { surface: String },
    PronunciationOverlap { surface: String },
}

struct Validator {
    issues: Vec<Diagnostic>,
    ids: BTreeMap<String, String>,
}

impl Validator {
    fn issue(&mut self, path: String, issue: ValidationIssue) {
        self.issues.push(Diagnostic { path, issue });
    }

    fn identity(&mut self, id: &str, path: String) {
        if let Some(first_seen) = self.ids.get(id) {
            self.issue(
                path,
                ValidationIssue::DuplicateId {
                    id: id.into(),
                    first_seen: first_seen.clone(),
                },
            );
        } else {
            self.ids.insert(id.into(), path);
        }
    }

    fn text(&mut self, text: &mut String, path: String) {
        let mut normalized = String::new();
        let mut space = false;
        for ch in text.chars() {
            if matches!(ch, '\u{200b}' | '\u{feff}' | '\u{ad}') {
                continue;
            }
            if is_white_space(ch) {
                space = !normalized.is_empty();
                continue;
            }
            if forbidden(ch) {
                self.issue(
                    path.clone(),
                    ValidationIssue::ForbiddenCharacter {
                        code_point: ch as u32,
                    },
                );
                continue;
            }
            if space {
                normalized.push(' ');
                space = false;
            }
            normalized.push(ch);
        }
        if normalized.is_empty() {
            self.issue(path.clone(), ValidationIssue::EmptyText);
        }
        *text = normalized.nfc().collect();
        if text.chars().count() > 10_000 {
            self.issue(
                path,
                ValidationIssue::NormalizedTextTooLong { max_chars: 10_000 },
            );
        }
    }

    fn dialogue(&mut self, raw: &mut RawDialogue, path: &str, speakers: &BTreeSet<String>) {
        self.identity(&raw.id, format!("{path}/id"));
        if !speakers.contains(&raw.speaker_id) {
            self.issue(
                format!("{path}/speaker_id"),
                ValidationIssue::UnknownSpeaker {
                    speaker_id: raw.speaker_id.clone(),
                },
            );
        }
        self.text(&mut raw.text, format!("{path}/text"));
        // One boolean per normalized UTF-8 byte bounds memory by the dialogue length.
        // Retaining every repeated match across invalid overrides multiplies that memory.
        let mut occupied = vec![false; raw.text.len()];
        for (i, pronunciation) in raw.pronunciation_overrides.iter_mut().enumerate() {
            let pp = format!("{path}/pronunciation_overrides/{i}");
            self.text(&mut pronunciation.surface, format!("{pp}/surface"));
            self.text(&mut pronunciation.replacement, format!("{pp}/replacement"));
            if pronunciation.surface.is_empty() {
                continue;
            }
            let mut found = false;
            let mut overlap = false;
            for (start, text) in raw.text.match_indices(&pronunciation.surface) {
                found = true;
                let span = &mut occupied[start..start + text.len()];
                overlap |= span.iter().any(|used| *used);
                span.fill(true);
            }
            if !found {
                self.issue(
                    format!("{pp}/surface"),
                    ValidationIssue::PronunciationTargetMissing {
                        surface: pronunciation.surface.clone(),
                    },
                );
            } else if overlap {
                self.issue(
                    format!("{pp}/surface"),
                    ValidationIssue::PronunciationOverlap {
                        surface: pronunciation.surface.clone(),
                    },
                );
            }
        }
        raw.pronunciation_overrides
            .sort_by(|a, b| a.surface.cmp(&b.surface));
    }
}

// Freeze the Unicode White_Space property, excluding C1 control NEL (U+0085).
fn is_white_space(ch: char) -> bool {
    matches!(
        ch,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

fn forbidden(ch: char) -> bool {
    // Vietnamese admission profile: controls, Unicode format controls, invisible fillers,
    // variation selectors and supplementary tags. No provider markup is interpreted.
    matches!(ch as u32, 0..=31 | 127..=159 | 0x600..=0x605 | 0x61c | 0x6dd | 0x70f |
        0x890..=0x891 | 0x8e2 | 0x115f..=0x1160 | 0x17b4..=0x17b5 | 0x180b..=0x180f |
        0x200c..=0x200f | 0x202a..=0x202e | 0x2060..=0x206f | 0x3164 | 0xfe00..=0xfe0f |
        0xffa0 | 0xfff9..=0xfffb | 0x110bd | 0x110cd | 0x13430..=0x13455 |
        0x1bca0..=0x1bca3 | 0x1d173..=0x1d17a | 0xe0000..=0xe0fff)
}

pub(super) fn validate(mut raw: RawScript) -> Result<ScriptContent, Vec<Diagnostic>> {
    let mut validator = Validator {
        issues: Vec::new(),
        ids: BTreeMap::new(),
    };
    let sources: BTreeSet<_> = raw
        .provenance
        .iter()
        .map(|source| source.fields().0.to_owned())
        .collect();
    validator.identity(&raw.work.id, "work/id".into());
    validator.text(&mut raw.work.title, "work/title".into());
    if !sources.contains(&raw.work.source_ref) {
        validator.issue(
            "work/source_ref".into(),
            ValidationIssue::UnknownProvenance {
                source_id: raw.work.source_ref.clone(),
            },
        );
    }
    validator.identity(&raw.adaptation.id, "adaptation/id".into());
    let mut refs = BTreeSet::new();
    for (i, source) in raw.adaptation.provenance_refs.iter().enumerate() {
        let path = format!("adaptation/provenance_refs/{i}");
        if !sources.contains(source) {
            validator.issue(
                path.clone(),
                ValidationIssue::UnknownProvenance {
                    source_id: source.clone(),
                },
            );
        }
        if !refs.insert(source) {
            validator.issue(
                path,
                ValidationIssue::DuplicateProvenanceRef {
                    source_id: source.clone(),
                },
            );
        }
    }
    let speakers = raw.characters.iter().map(|c| c.id.clone()).collect();
    let narrators = raw
        .characters
        .iter()
        .filter(|c| c.role == Role::Narrator)
        .count();
    if narrators != 1 {
        validator.issue(
            "characters".into(),
            ValidationIssue::NarratorCount { found: narrators },
        );
    }
    for character in &mut raw.characters {
        let path = format!("character:{}", character.id);
        validator.identity(&character.id, format!("{path}/id"));
        validator.text(&mut character.name, format!("{path}/name"));
        validator.text(&mut character.personality, format!("{path}/personality"));
    }
    validator.identity(&raw.episode.id, "episode/id".into());
    validator.text(&mut raw.episode.title, "episode/title".into());
    for act in &mut raw.episode.acts {
        let ap = format!("episode/act:{}", act.id);
        validator.identity(&act.id, format!("{ap}/id"));
        validator.text(&mut act.title, format!("{ap}/title"));
        for scene in &mut act.scenes {
            let sp = format!("{ap}/scene:{}", scene.id);
            validator.identity(&scene.id, format!("{sp}/id"));
            validator.text(&mut scene.title, format!("{sp}/title"));
            let local_lines: BTreeSet<_> = scene.dialogues.iter().map(|d| d.id.clone()).collect();
            for dialogue in &mut scene.dialogues {
                let dp = format!("{sp}/dialogue:{}", dialogue.id);
                validator.dialogue(dialogue, &dp, &speakers);
            }
            for cue in &mut scene.sound_cues {
                let cp = format!("{sp}/cue:{}", cue.id);
                validator.identity(&cue.id, format!("{cp}/id"));
                validator.text(&mut cue.description, format!("{cp}/description"));
                if !local_lines.contains(&cue.anchor.dialogue_id) {
                    validator.issue(
                        format!("{cp}/anchor/dialogue_id"),
                        ValidationIssue::CueAnchorUnresolved {
                            dialogue_id: cue.anchor.dialogue_id.clone(),
                        },
                    );
                }
            }
        }
    }
    for source in &raw.provenance {
        let id = source.fields().0;
        validator.identity(id, format!("provenance:{id}/id"));
    }
    if !validator.issues.is_empty() {
        return Err(validator.issues);
    }
    raw.characters.sort_by(|a, b| a.id.cmp(&b.id));
    raw.provenance
        .sort_by(|a, b| a.fields().0.cmp(b.fields().0));
    raw.adaptation.provenance_refs.sort();
    Ok(into_content(raw))
}

// The only conversion into domain fields, called after complete shape + semantic checks.
fn into_content(raw: RawScript) -> ScriptContent {
    ScriptContent {
        work: Work {
            id: Id(raw.work.id),
            title: Text(raw.work.title),
            source_ref: Id(raw.work.source_ref),
            rights_record_id: Id(raw.work.rights_record_id),
        },
        adaptation: Adaptation {
            id: Id(raw.adaptation.id),
            language: raw.adaptation.language,
            rights_record_id: Id(raw.adaptation.rights_record_id),
            provenance_refs: raw.adaptation.provenance_refs.into_iter().map(Id).collect(),
        },
        characters: raw
            .characters
            .into_iter()
            .map(|c| Character {
                id: CharacterId(c.id),
                name: Text(c.name),
                role: c.role,
                personality: Text(c.personality),
            })
            .collect(),
        episode: into_episode(raw.episode),
        provenance: raw.provenance.into_iter().map(into_source).collect(),
    }
}

fn into_episode(raw: RawEpisode) -> Episode {
    Episode {
        id: Id(raw.id),
        title: Text(raw.title),
        acts: raw.acts.into_iter().map(into_act).collect(),
    }
}

fn into_act(raw: RawAct) -> Act {
    Act {
        id: Id(raw.id),
        title: Text(raw.title),
        scenes: raw.scenes.into_iter().map(into_scene).collect(),
    }
}

fn into_scene(raw: RawScene) -> Scene {
    Scene {
        id: Id(raw.id),
        title: Text(raw.title),
        dialogues: raw.dialogues.into_iter().map(into_dialogue).collect(),
        sound_cues: raw.sound_cues.into_iter().map(into_cue).collect(),
    }
}

fn into_dialogue(raw: RawDialogue) -> Dialogue {
    Dialogue {
        id: DialogueId(raw.id),
        speaker: CharacterId(raw.speaker_id),
        text: Text(raw.text),
        delivery: Delivery {
            emotion: raw.delivery.emotion,
            intensity_permille: raw.delivery.intensity_permille,
        },
        pronunciation: raw
            .pronunciation_overrides
            .into_iter()
            .map(|p| Pronunciation {
                surface: Text(p.surface),
                replacement: Text(p.replacement),
            })
            .collect(),
    }
}

fn into_cue(raw: RawCue) -> Cue {
    Cue {
        id: Id(raw.id),
        kind: raw.kind,
        description: Text(raw.description),
        dialogue: DialogueId(raw.anchor.dialogue_id),
        edge: raw.anchor.edge,
        asset: raw.asset.map(|a| Asset {
            id: Id(a.id),
            rights_record_id: Id(a.rights_record_id),
        }),
    }
}

fn into_source(raw: RawSource) -> Source {
    let (id, kind, source_record_id, rights_record_id) = match raw {
        RawSource::Original {
            id,
            source_record_id,
            rights_record_id,
        } => (id, SourceKind::Original, source_record_id, rights_record_id),
        RawSource::Imported {
            id,
            source_record_id,
            rights_record_id,
        } => (id, SourceKind::Imported, source_record_id, rights_record_id),
        RawSource::Generated {
            id,
            source_record_id,
            rights_record_id,
            generation_record_id,
        } => (
            id,
            SourceKind::Generated {
                generation_record_id: Id(generation_record_id),
            },
            source_record_id,
            rights_record_id,
        ),
    };
    Source {
        id: Id(id),
        kind,
        source_record_id: Id(source_record_id),
        rights_record_id: Id(rights_record_id),
    }
}
