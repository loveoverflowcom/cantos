//! Validated, immutable Script IR and its versioned JSON boundary.
//!
//! The domain contains no transport, persistence, clock or provider types. Only
//! `parse` constructs content; its wire adapter cannot bypass semantic validation.
mod wire;

use std::collections::BTreeSet;

use serde::Serialize;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

pub const SCHEMA_VERSION: &str = "0.1.0";
const DIGEST_DOMAIN: &[u8] = b"cantos/script-content/v1\n";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationCode {
    InvalidShape,
    UnsupportedSchemaVersion,
    InvalidId,
    DuplicateId,
    EmptyCollection,
    EmptyText,
    TextTooLong,
    ForbiddenTextCharacter,
    UnsupportedLanguage,
    NarratorRequired,
    UnknownSpeaker,
    CueAnchorUnresolved,
    CueRightsRequired,
    InvalidIntensity,
    PronunciationTargetMissing,
    PronunciationTargetOverlap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationIssue {
    pub code: ValidationCode,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl std::fmt::Display for ValidationReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "script validation failed ({} issues)",
            self.issues.len()
        )
    }
}
impl std::error::Error for ValidationReport {}

/// A value admitted through the only public construction path. There is no
/// `Deserialize`, public field, mutable accessor or unchecked constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptContent {
    work: Work,
    adaptation: Adaptation,
    characters: Vec<Character>,
    episode: Episode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StableId(String);
#[derive(Debug, Clone, PartialEq, Eq)]
struct Text(String);
#[derive(Debug, Clone, PartialEq, Eq)]
struct Work {
    id: StableId,
    title: Text,
    source_ref: StableId,
    rights_ref: StableId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Adaptation {
    id: StableId,
    language: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Character {
    id: StableId,
    name: Text,
    role: wire::Role,
    characterization: Option<Text>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Episode {
    id: StableId,
    title: Text,
    acts: Vec<Act>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Act {
    id: StableId,
    title: Text,
    scenes: Vec<Scene>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Scene {
    id: StableId,
    title: Text,
    dialogues: Vec<Dialogue>,
    sound_cues: Vec<SoundCue>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Dialogue {
    id: StableId,
    speaker_id: StableId,
    text: Text,
    delivery: Option<Delivery>,
    pronunciation: Vec<Pronunciation>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Delivery {
    emotion: wire::Emotion,
    intensity_permille: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pronunciation {
    surface: Text,
    respelling: Text,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct SoundCue {
    id: StableId,
    kind: wire::CueKind,
    description: Text,
    anchor: CueAnchor,
    selected_asset: Option<SelectedAsset>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct CueAnchor {
    dialogue_id: StableId,
    edge: wire::Edge,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedAsset {
    asset_ref: StableId,
    rights_ref: StableId,
}

/// Decode shape first, then accumulate independent semantic issues. Error
/// reports deliberately contain no input text or serializer diagnostics.
pub fn parse(value: serde_json::Value) -> Result<ScriptContent, ValidationReport> {
    if let Some(version) = value
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        && version != SCHEMA_VERSION
    {
        return Err(single_issue(
            ValidationCode::UnsupportedSchemaVersion,
            "/schema_version",
        ));
    }
    let raw: wire::RawScript = serde_json::from_value(value)
        .map_err(|_| single_issue(ValidationCode::InvalidShape, "/"))?;
    Validator::default().validate(raw)
}

fn single_issue(code: ValidationCode, path: &str) -> ValidationReport {
    ValidationReport {
        issues: vec![ValidationIssue {
            code,
            path: path.into(),
        }],
    }
}

#[derive(Default)]
struct Validator {
    issues: Vec<ValidationIssue>,
    ids: BTreeSet<String>,
    speakers: BTreeSet<String>,
}

impl Validator {
    fn issue(&mut self, code: ValidationCode, path: &str) {
        self.issues.push(ValidationIssue {
            code,
            path: path.into(),
        });
    }

    fn id(&mut self, value: String, path: &str, entity: bool) -> StableId {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        {
            self.issue(ValidationCode::InvalidId, path);
        }
        if entity && !self.ids.insert(value.clone()) {
            self.issue(ValidationCode::DuplicateId, path);
        }
        StableId(value)
    }

    fn text(&mut self, value: String, path: &str, limit: usize) -> Text {
        if value.chars().any(forbidden_character) {
            self.issue(ValidationCode::ForbiddenTextCharacter, path);
        }
        let visible: String = value
            .chars()
            .filter(|c| !matches!(c, '\u{200b}' | '\u{feff}' | '\u{00ad}'))
            .collect();
        let normalized = visible
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .nfc()
            .collect::<String>();
        if normalized.is_empty() {
            self.issue(ValidationCode::EmptyText, path);
        }
        if normalized.len() > limit {
            self.issue(ValidationCode::TextTooLong, path);
        }
        Text(normalized)
    }

    fn nonempty<T>(&mut self, values: &[T], path: &str) {
        if values.is_empty() {
            self.issue(ValidationCode::EmptyCollection, path);
        }
    }

    fn validate(mut self, raw: wire::RawScript) -> Result<ScriptContent, ValidationReport> {
        // The candidate is private and never returned until the whole report is empty.
        let work = Work {
            id: self.id(raw.work.id, "/work/id", true),
            title: self.text(raw.work.title, "/work/title", 1024),
            source_ref: self.id(raw.work.source_ref, "/work/source_ref", false),
            rights_ref: self.id(raw.work.rights_ref, "/work/rights_ref", false),
        };
        let adaptation_id = self.id(raw.adaptation.id, "/adaptation/id", true);
        if !matches!(raw.adaptation.language.as_str(), "vi-VN" | "en" | "en-US") {
            self.issue(ValidationCode::UnsupportedLanguage, "/adaptation/language");
        }
        let adaptation = Adaptation {
            id: adaptation_id,
            language: raw.adaptation.language,
        };
        self.nonempty(&raw.characters, "/characters");
        self.speakers = raw.characters.iter().map(|c| c.id.clone()).collect();
        if raw
            .characters
            .iter()
            .filter(|c| c.role == wire::Role::Narrator)
            .count()
            != 1
        {
            self.issue(ValidationCode::NarratorRequired, "/characters");
        }
        let mut characters = raw
            .characters
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let p = format!("/characters/{i}");
                Character {
                    id: self.id(c.id, &format!("{p}/id"), true),
                    name: self.text(c.name, &format!("{p}/name"), 1024),
                    role: c.role,
                    characterization: c
                        .characterization
                        .map(|s| self.text(s, &format!("{p}/characterization"), 4096)),
                }
            })
            .collect::<Vec<_>>();
        characters.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        let episode = self.episode(raw.episode);
        if !self.issues.is_empty() {
            return Err(ValidationReport {
                issues: self.issues,
            });
        }
        Ok(ScriptContent {
            work,
            adaptation,
            characters,
            episode,
        })
    }

    fn episode(&mut self, raw: wire::RawEpisode) -> Episode {
        let id = self.id(raw.id, "/episode/id", true);
        let title = self.text(raw.title, "/episode/title", 1024);
        self.nonempty(&raw.acts, "/episode/acts");
        let acts = raw
            .acts
            .into_iter()
            .enumerate()
            .map(|(i, a)| {
                let p = format!("/episode/acts/{i}");
                let id = self.id(a.id, &format!("{p}/id"), true);
                let title = self.text(a.title, &format!("{p}/title"), 1024);
                self.nonempty(&a.scenes, &format!("{p}/scenes"));
                let scenes = a
                    .scenes
                    .into_iter()
                    .enumerate()
                    .map(|(j, s)| self.scene(s, &format!("{p}/scenes/{j}")))
                    .collect();
                Act { id, title, scenes }
            })
            .collect();
        Episode { id, title, acts }
    }

    fn scene(&mut self, raw: wire::RawScene, p: &str) -> Scene {
        let id = self.id(raw.id, &format!("{p}/id"), true);
        let title = self.text(raw.title, &format!("{p}/title"), 1024);
        self.nonempty(&raw.dialogues, &format!("{p}/dialogues"));
        let dialogue_ids: BTreeSet<_> = raw.dialogues.iter().map(|d| d.id.clone()).collect();
        let dialogues = raw
            .dialogues
            .into_iter()
            .enumerate()
            .map(|(i, d)| self.dialogue(d, &format!("{p}/dialogues/{i}")))
            .collect();
        let sound_cues = raw
            .sound_cues
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let p = format!("{p}/sound_cues/{i}");
                let id = self.id(c.id, &format!("{p}/id"), true);
                let description = self.text(c.description, &format!("{p}/description"), 4096);
                if !dialogue_ids.contains(&c.anchor.dialogue_id) {
                    self.issue(
                        ValidationCode::CueAnchorUnresolved,
                        &format!("{p}/anchor/dialogue_id"),
                    );
                }
                let anchor = CueAnchor {
                    dialogue_id: self.id(
                        c.anchor.dialogue_id,
                        &format!("{p}/anchor/dialogue_id"),
                        false,
                    ),
                    edge: c.anchor.edge,
                };
                let selected_asset = match (c.asset_ref, c.rights_ref) {
                    (Some(asset), Some(rights)) => Some(SelectedAsset {
                        asset_ref: self.id(asset, &format!("{p}/asset_ref"), false),
                        rights_ref: self.id(rights, &format!("{p}/rights_ref"), false),
                    }),
                    (None, None) => None,
                    _ => {
                        self.issue(ValidationCode::CueRightsRequired, &p);
                        None
                    }
                };
                SoundCue {
                    id,
                    kind: c.kind,
                    description,
                    anchor,
                    selected_asset,
                }
            })
            .collect();
        Scene {
            id,
            title,
            dialogues,
            sound_cues,
        }
    }

    fn dialogue(&mut self, raw: wire::RawDialogue, p: &str) -> Dialogue {
        let id = self.id(raw.id, &format!("{p}/id"), true);
        if !self.speakers.contains(&raw.speaker_id) {
            self.issue(ValidationCode::UnknownSpeaker, &format!("{p}/speaker_id"));
        }
        let speaker_id = self.id(raw.speaker_id, &format!("{p}/speaker_id"), false);
        let text = self.text(raw.text, &format!("{p}/text"), 16384);
        let delivery = raw.delivery.map(|d| {
            if d.intensity_permille > 1000 {
                self.issue(
                    ValidationCode::InvalidIntensity,
                    &format!("{p}/delivery/intensity_permille"),
                );
            }
            Delivery {
                emotion: d.emotion,
                intensity_permille: d.intensity_permille,
            }
        });
        let mut occupied_bytes = vec![false; text.0.len()];
        let pronunciation = raw
            .pronunciation
            .into_iter()
            .enumerate()
            .map(|(i, pr)| {
                let p = format!("{p}/pronunciation/{i}");
                let surface = self.text(pr.surface, &format!("{p}/surface"), 1024);
                let respelling = self.text(pr.respelling, &format!("{p}/respelling"), 1024);
                if !surface.0.is_empty() {
                    let found: Vec<_> = text
                        .0
                        .match_indices(&surface.0)
                        .map(|(start, value)| (start, start + value.len()))
                        .collect();
                    if found.is_empty() {
                        self.issue(
                            ValidationCode::PronunciationTargetMissing,
                            &format!("{p}/surface"),
                        );
                    }
                    if found.iter().any(|(start, end)| {
                        occupied_bytes[*start..*end]
                            .iter()
                            .any(|occupied| *occupied)
                    }) {
                        self.issue(
                            ValidationCode::PronunciationTargetOverlap,
                            &format!("{p}/surface"),
                        );
                    }
                    for (start, end) in found {
                        occupied_bytes[start..end].fill(true);
                    }
                }
                Pronunciation {
                    surface,
                    respelling,
                }
            })
            .collect();
        Dialogue {
            id,
            speaker_id,
            text,
            delivery,
            pronunciation,
        }
    }
}

fn forbidden_character(c: char) -> bool {
    (!c.is_whitespace() && c.is_control())
        || matches!(c,
        '\u{0600}'..='\u{0605}' | '\u{061c}' | '\u{06dd}' | '\u{070f}' |
        '\u{0890}'..='\u{0891}' | '\u{08e2}' | '\u{180e}' |
        '\u{200c}'..='\u{200f}' | '\u{202a}'..='\u{202e}' |
        '\u{2060}'..='\u{206f}' | '\u{fff9}'..='\u{fffb}' |
        '\u{110bd}' | '\u{110cd}' | '\u{13430}'..='\u{1343f}' |
        '\u{1bca0}'..='\u{1bca3}' | '\u{1d173}'..='\u{1d17a}' |
        '\u{e0001}' | '\u{e0020}'..='\u{e007f}')
}

impl ScriptContent {
    pub fn work_id(&self) -> &str {
        &self.work.id.0
    }
    pub fn adaptation_id(&self) -> &str {
        &self.adaptation.id.0
    }
    pub fn episode_id(&self) -> &str {
        &self.episode.id.0
    }
    pub fn source_ref(&self) -> &str {
        &self.work.source_ref.0
    }
    pub fn rights_ref(&self) -> &str {
        &self.work.rights_ref.0
    }

    /// A detached wire value. Mutating it cannot mutate this validated content.
    pub fn as_json(&self) -> serde_json::Value {
        wire::encode(self)
    }

    /// Canonical *performance content*, distinct from the complete wire record.
    /// Provenance and rights references are retained by the revision envelope.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut value = self.as_json();
        if let Some(object) = value.as_object_mut() {
            object.remove("schema_version");
        }
        if let Some(work) = value
            .get_mut("work")
            .and_then(serde_json::Value::as_object_mut)
        {
            work.remove("source_ref");
            work.remove("rights_ref");
        }
        // Cue rights identify external evidence, not the performed asset itself.
        for act in &mut value["episode"]["acts"]
            .as_array_mut()
            .into_iter()
            .flatten()
        {
            for scene in &mut act["scenes"].as_array_mut().into_iter().flatten() {
                for cue in scene
                    .get_mut("sound_cues")
                    .and_then(serde_json::Value::as_array_mut)
                    .into_iter()
                    .flatten()
                {
                    if let Some(object) = cue.as_object_mut() {
                        object.remove("rights_ref");
                    }
                }
            }
        }
        wire::canonical_bytes(&value)
    }

    pub fn digest(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(DIGEST_DOMAIN);
        hash.update(self.canonical_bytes());
        format!("cantos-script-v1:sha256:{:x}", hash.finalize())
    }
}

#[cfg(test)]
mod tests;
