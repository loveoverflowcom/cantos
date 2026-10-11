//! Lossless structured editing at the Studio boundary, independent of Leptos and transport.
//!
//! Read models are projections for rendering only. Every edit patches the complete JSON value;
//! provenance, rights, optional assets and fields the current UI does not understand survive.
//! These checks protect editing structure and identity. Backend admission remains authoritative.
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::sync::Arc;

pub const SCHEMA_VERSION: &str = "0.1.0";
/// Browser preflight matching the current contract; this grants no storage admission.
pub const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
const HISTORY_LIMIT: usize = 40;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthoringError {
    DocumentTooLarge,
    InvalidJson {
        line: usize,
        column: usize,
        detail: String,
    },
    NumericPrecisionLoss(String),
    UnsupportedVersion(String),
    Shape {
        path: String,
        expected: &'static str,
    },
    MissingTarget(Target),
    DuplicateId(String),
    InvalidId(String),
    UnsupportedField {
        target: Target,
        field: Field,
    },
    InvalidInteger(String),
    Referenced {
        target: Target,
        references: Vec<String>,
    },
    NotInCollection(String),
    MissingPronunciation {
        dialogue_id: String,
        index: usize,
    },
}

impl fmt::Display for AuthoringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocumentTooLarge => {
                write!(formatter, "Document exceeds {MAX_DOCUMENT_BYTES} bytes")
            }
            Self::InvalidJson {
                line,
                column,
                detail,
            } => write!(formatter, "JSON {line}:{column}: {detail}"),
            Self::NumericPrecisionLoss(value) => write!(
                formatter,
                "JSON number cannot be preserved exactly: {value}"
            ),
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "Unsupported Script IR {version}; this editor reads {SCHEMA_VERSION}"
            ),
            Self::Shape { path, expected } => write!(formatter, "{path}: expected {expected}"),
            Self::MissingTarget(target) => write!(formatter, "Missing {target:?}"),
            Self::DuplicateId(id) => write!(formatter, "ID already exists: {id}"),
            Self::InvalidId(id) => write!(formatter, "Invalid new ID: {id}"),
            Self::UnsupportedField { target, field } => {
                write!(formatter, "{field:?} does not belong to {target:?}")
            }
            Self::InvalidInteger(value) => {
                write!(formatter, "Expected a nonnegative integer: {value}")
            }
            Self::Referenced { target, references } => write!(
                formatter,
                "{target:?} is referenced by {}",
                references.join(", ")
            ),
            Self::NotInCollection(id) => write!(formatter, "{id} is not in the same collection"),
            Self::MissingPronunciation { dialogue_id, index } => {
                write!(formatter, "Missing pronunciation {index} in {dialogue_id}")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Work,
    Adaptation,
    Episode,
    Character(String),
    Act(String),
    Scene(String),
    Dialogue(String),
    Cue(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Title,
    Name,
    Role,
    Personality,
    Language,
    Text,
    Speaker,
    Emotion,
    Intensity,
    CueKind,
    CueDescription,
    CueDialogue,
    CueEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PronunciationField {
    Surface,
    Replacement,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    pub work_title: String,
    pub episode_title: String,
    pub language: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Character {
    pub id: String,
    pub name: String,
    pub role: String,
    pub personality: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Act {
    pub id: String,
    pub title: String,
    pub scenes: Vec<Scene>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scene {
    pub id: String,
    pub title: String,
    pub dialogues: Vec<Dialogue>,
    pub sound_cues: Vec<Cue>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dialogue {
    pub id: String,
    pub speaker_id: String,
    pub text: String,
    pub emotion: String,
    pub intensity_permille: u64,
    pub pronunciation_overrides: Vec<Pronunciation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pronunciation {
    pub surface: String,
    pub replacement: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cue {
    pub id: String,
    pub kind: String,
    pub description: String,
    pub dialogue_id: String,
    pub edge: String,
    pub asset: Option<CueAsset>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CueAsset {
    pub id: String,
    pub rights_record_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    raw: Arc<Value>,
}

impl Document {
    pub fn parse(text: &str) -> Result<Self, AuthoringError> {
        if text.len() > MAX_DOCUMENT_BYTES {
            return Err(AuthoringError::DocumentTooLarge);
        }
        let parsed: UniqueValue =
            serde_json::from_str(text).map_err(|error| AuthoringError::InvalidJson {
                line: error.line(),
                column: error.column(),
                detail: error.to_string(),
            })?;
        check_number_precision(text)?;
        Self::from_value(parsed.0)
    }

    fn from_value(raw: Value) -> Result<Self, AuthoringError> {
        validate_shape(&raw)?;
        Ok(Self { raw: Arc::new(raw) })
    }

    fn raw_mut(&mut self) -> &mut Value {
        Arc::make_mut(&mut self.raw)
    }

    pub fn to_json(&self) -> String {
        self.raw.to_string()
    }

    pub fn metadata(&self) -> Metadata {
        Metadata {
            work_title: text(&self.raw["work"], "title"),
            episode_title: text(&self.raw["episode"], "title"),
            language: text(&self.raw["adaptation"], "language"),
        }
    }

    pub fn characters(&self) -> Vec<Character> {
        values(&self.raw["characters"])
            .iter()
            .map(|value| Character {
                id: text(value, "id"),
                name: text(value, "name"),
                role: text(value, "role"),
                personality: text(value, "personality"),
            })
            .collect()
    }

    pub fn acts(&self) -> Vec<Act> {
        values(&self.raw["episode"]["acts"])
            .iter()
            .map(|act| Act {
                id: text(act, "id"),
                title: text(act, "title"),
                scenes: values(&act["scenes"])
                    .iter()
                    .map(|scene| Scene {
                        id: text(scene, "id"),
                        title: text(scene, "title"),
                        dialogues: values(&scene["dialogues"])
                            .iter()
                            .map(|dialogue| Dialogue {
                                id: text(dialogue, "id"),
                                speaker_id: text(dialogue, "speaker_id"),
                                text: text(dialogue, "text"),
                                emotion: text(&dialogue["delivery"], "emotion"),
                                intensity_permille: dialogue["delivery"]["intensity_permille"]
                                    .as_u64()
                                    .unwrap_or_default(),
                                pronunciation_overrides: values(
                                    &dialogue["pronunciation_overrides"],
                                )
                                .iter()
                                .map(|pronunciation| Pronunciation {
                                    surface: text(pronunciation, "surface"),
                                    replacement: text(pronunciation, "replacement"),
                                })
                                .collect(),
                            })
                            .collect(),
                        sound_cues: values(&scene["sound_cues"])
                            .iter()
                            .map(|cue| Cue {
                                id: text(cue, "id"),
                                kind: text(cue, "kind"),
                                description: text(cue, "description"),
                                dialogue_id: text(&cue["anchor"], "dialogue_id"),
                                edge: text(&cue["anchor"], "edge"),
                                asset: cue.get("asset").map(|asset| CueAsset {
                                    id: text(asset, "id"),
                                    rights_record_id: text(asset, "rights_record_id"),
                                }),
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect()
    }

    /// Current JSON pointer for matching server diagnostics without using array indexes as identity.
    pub fn path(&self, target: &Target) -> Result<String, AuthoringError> {
        match target {
            Target::Work => return Ok("/work".into()),
            Target::Adaptation => return Ok("/adaptation".into()),
            Target::Episode => return Ok("/episode".into()),
            Target::Character(id) => {
                find_in(&self.raw["characters"], id).map(|index| format!("/characters/{index}"))
            }
            Target::Act(id) => find_in(&self.raw["episode"]["acts"], id)
                .map(|index| format!("/episode/acts/{index}")),
            Target::Scene(id) | Target::Dialogue(id) | Target::Cue(id) => {
                for (act_index, act) in values(&self.raw["episode"]["acts"]).iter().enumerate() {
                    for (scene_index, scene) in values(&act["scenes"]).iter().enumerate() {
                        let scene_path = format!("/episode/acts/{act_index}/scenes/{scene_index}");
                        match target {
                            Target::Scene(_) if text(scene, "id") == *id => return Ok(scene_path),
                            Target::Dialogue(_) => {
                                if let Some(index) = find_in(&scene["dialogues"], id) {
                                    return Ok(format!("{scene_path}/dialogues/{index}"));
                                }
                            }
                            Target::Cue(_) => {
                                if let Some(index) = find_in(&scene["sound_cues"], id) {
                                    return Ok(format!("{scene_path}/sound_cues/{index}"));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                None
            }
        }
        .ok_or_else(|| AuthoringError::MissingTarget(target.clone()))
    }

    pub fn field_path(&self, target: &Target, field: Field) -> Result<String, AuthoringError> {
        let suffix = match (target, field) {
            (Target::Work | Target::Episode | Target::Act(_) | Target::Scene(_), Field::Title) => {
                "title"
            }
            (Target::Adaptation, Field::Language) => "language",
            (Target::Character(_), Field::Name) => "name",
            (Target::Character(_), Field::Role) => "role",
            (Target::Character(_), Field::Personality) => "personality",
            (Target::Dialogue(_), Field::Text) => "text",
            (Target::Dialogue(_), Field::Speaker) => "speaker_id",
            (Target::Dialogue(_), Field::Emotion) => "delivery/emotion",
            (Target::Dialogue(_), Field::Intensity) => "delivery/intensity_permille",
            (Target::Cue(_), Field::CueKind) => "kind",
            (Target::Cue(_), Field::CueDescription) => "description",
            (Target::Cue(_), Field::CueDialogue) => "anchor/dialogue_id",
            (Target::Cue(_), Field::CueEdge) => "anchor/edge",
            _ => {
                return Err(AuthoringError::UnsupportedField {
                    target: target.clone(),
                    field,
                })
            }
        };
        Ok(format!("{}/{suffix}", self.path(target)?))
    }

    /// Shape diagnostics use current JSON positions; semantic diagnostics use stable IDs.
    /// This only locates server findings and never duplicates the server validation decision.
    pub fn diagnostic_paths(
        &self,
        target: &Target,
        field: Field,
    ) -> Result<Vec<String>, AuthoringError> {
        let pointer = self.path(target)?;
        let field_pointer = self.field_path(target, field)?;
        let suffix = field_pointer
            .strip_prefix(&pointer)
            .ok_or_else(|| shape(&field_pointer, "field below target"))?;
        let semantic = match target {
            Target::Work => "work".into(),
            Target::Adaptation => "adaptation".into(),
            Target::Episode => "episode".into(),
            Target::Character(id) => format!("character:{id}"),
            Target::Act(id) => format!("episode/act:{id}"),
            Target::Scene(_) | Target::Dialogue(_) | Target::Cue(_) => {
                let segments: Vec<_> = pointer.split('/').collect();
                let act_index = segments
                    .get(3)
                    .ok_or_else(|| shape(&pointer, "act position"))?;
                let scene_index = segments
                    .get(5)
                    .ok_or_else(|| shape(&pointer, "scene position"))?;
                let act_pointer = format!("/episode/acts/{act_index}");
                let scene_pointer = format!("{act_pointer}/scenes/{scene_index}");
                let act = self
                    .raw
                    .pointer(&act_pointer)
                    .ok_or_else(|| shape(&act_pointer, "existing act"))?;
                let scene = self
                    .raw
                    .pointer(&scene_pointer)
                    .ok_or_else(|| shape(&scene_pointer, "existing scene"))?;
                let parent = format!(
                    "episode/act:{}/scene:{}",
                    text(act, "id"),
                    text(scene, "id")
                );
                match target {
                    Target::Dialogue(id) => format!("{parent}/dialogue:{id}"),
                    Target::Cue(id) => format!("{parent}/cue:{id}"),
                    _ => parent,
                }
            }
        };
        Ok(vec![field_pointer.clone(), format!("{semantic}{suffix}")])
    }

    pub fn field(&self, target: &Target, field: Field) -> Result<String, AuthoringError> {
        let path = self.field_path(target, field)?;
        self.raw
            .pointer(&path)
            .map(display_value)
            .ok_or_else(|| shape(&path, "existing field"))
    }

    pub fn set(&self, target: &Target, field: Field, value: &str) -> Result<Self, AuthoringError> {
        let path = self.field_path(target, field)?;
        let replacement = if field == Field::Intensity {
            Value::from(
                value
                    .parse::<u64>()
                    .map_err(|_| AuthoringError::InvalidInteger(value.into()))?,
            )
        } else {
            Value::String(value.into())
        };
        if self.raw.pointer(&path) == Some(&replacement) {
            return Ok(self.clone());
        }
        let mut next = self.clone();
        let slot = next
            .raw_mut()
            .pointer_mut(&path)
            .ok_or_else(|| AuthoringError::Shape {
                path: path.clone(),
                expected: "existing field",
            })?;
        *slot = replacement;
        Ok(next)
    }

    pub fn insert_character(
        &self,
        id: &str,
        name: &str,
        role: &str,
        personality: &str,
    ) -> Result<Self, AuthoringError> {
        self.insert(
            "/characters",
            id,
            json!({ "id": id, "name": name, "role": role, "personality": personality }),
        )
    }

    pub fn insert_act(&self, id: &str, title: &str) -> Result<Self, AuthoringError> {
        self.insert(
            "/episode/acts",
            id,
            json!({ "id": id, "title": title, "scenes": [] }),
        )
    }

    pub fn insert_scene(
        &self,
        act_id: &str,
        id: &str,
        title: &str,
    ) -> Result<Self, AuthoringError> {
        let path = format!("{}/scenes", self.path(&Target::Act(act_id.into()))?);
        self.insert(
            &path,
            id,
            json!({ "id": id, "title": title, "dialogues": [], "sound_cues": [] }),
        )
    }

    pub fn insert_dialogue(
        &self,
        scene_id: &str,
        id: &str,
        speaker_id: &str,
        spoken_text: &str,
    ) -> Result<Self, AuthoringError> {
        self.path(&Target::Character(speaker_id.into()))?;
        let path = format!("{}/dialogues", self.path(&Target::Scene(scene_id.into()))?);
        self.insert(&path, id, json!({ "id": id, "speaker_id": speaker_id, "text": spoken_text, "delivery": { "emotion": "neutral", "intensity_permille": 500 }, "pronunciation_overrides": [] }))
    }

    pub fn insert_cue(
        &self,
        scene_id: &str,
        id: &str,
        kind: &str,
        description: &str,
        dialogue_id: &str,
        edge: &str,
    ) -> Result<Self, AuthoringError> {
        let path = self.path(&Target::Scene(scene_id.into()))?;
        let scene = self
            .raw
            .pointer(&path)
            .ok_or_else(|| AuthoringError::MissingTarget(Target::Scene(scene_id.into())))?;
        if find_in(&scene["dialogues"], dialogue_id).is_none() {
            return Err(AuthoringError::NotInCollection(dialogue_id.into()));
        }
        let mut next = self.clone();
        let scene = next
            .raw_mut()
            .pointer_mut(&path)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| shape(&path, "scene object"))?;
        scene
            .entry("sound_cues")
            .or_insert_with(|| Value::Array(vec![]));
        next.insert(&format!("{path}/sound_cues"), id, json!({ "id": id, "kind": kind, "description": description, "anchor": { "dialogue_id": dialogue_id, "edge": edge } }))
    }

    fn insert(&self, path: &str, id: &str, value: Value) -> Result<Self, AuthoringError> {
        if id.is_empty()
            || id.len() > 64
            || !id.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
            })
        {
            return Err(AuthoringError::InvalidId(id.into()));
        }
        if contains_id(&self.raw, id) {
            return Err(AuthoringError::DuplicateId(id.into()));
        }
        let mut next = self.clone();
        let array = next
            .raw_mut()
            .pointer_mut(path)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| shape(path, "array"))?;
        array.push(value);
        Ok(next)
    }

    /// Refuse reference-breaking deletion; users must choose corrections explicitly first.
    pub fn remove(&self, target: &Target) -> Result<Self, AuthoringError> {
        let path = self.path(target)?;
        let mut references = vec![];
        match target {
            Target::Character(id) => {
                for act in self.acts() {
                    for scene in act.scenes {
                        for dialogue in scene.dialogues {
                            if dialogue.speaker_id == *id {
                                references.push(dialogue.id);
                            }
                        }
                    }
                }
            }
            Target::Dialogue(id) => {
                for act in self.acts() {
                    for scene in act.scenes {
                        for cue in scene.sound_cues {
                            if cue.dialogue_id == *id {
                                references.push(cue.id);
                            }
                        }
                    }
                }
            }
            Target::Act(_) | Target::Scene(_) | Target::Cue(_) => {}
            _ => return Err(AuthoringError::NotInCollection(path)),
        }
        if !references.is_empty() {
            return Err(AuthoringError::Referenced {
                target: target.clone(),
                references,
            });
        }
        let (parent, index) = array_position(&path)?;
        let mut next = self.clone();
        let array = next
            .raw_mut()
            .pointer_mut(parent)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| shape(parent, "array"))?;
        array.remove(index);
        Ok(next)
    }

    /// Reorder within the existing parent, preserving IDs, complete values and references.
    pub fn move_before(
        &self,
        target: &Target,
        before: Option<&str>,
    ) -> Result<Self, AuthoringError> {
        let path = self.path(target)?;
        let (parent, index) = array_position(&path)?;
        let mut next = self.clone();
        let array = next
            .raw_mut()
            .pointer_mut(parent)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| shape(parent, "array"))?;
        let destination = match before {
            Some(id) => find_in_slice(array, id)
                .ok_or_else(|| AuthoringError::NotInCollection(id.into()))?,
            None => array.len(),
        };
        if index != destination {
            let row = array.remove(index);
            array.insert(
                if destination > index {
                    destination - 1
                } else {
                    destination
                },
                row,
            );
        }
        Ok(next)
    }

    pub fn set_pronunciation(
        &self,
        dialogue_id: &str,
        index: usize,
        field: PronunciationField,
        value: &str,
    ) -> Result<Self, AuthoringError> {
        let path = self.pronunciation_path(dialogue_id)?;
        let key = match field {
            PronunciationField::Surface => "surface",
            PronunciationField::Replacement => "replacement",
        };
        let mut next = self.clone();
        let slot = next
            .raw_mut()
            .pointer_mut(&format!("{path}/{index}/{key}"))
            .ok_or_else(|| AuthoringError::MissingPronunciation {
                dialogue_id: dialogue_id.into(),
                index,
            })?;
        *slot = Value::String(value.into());
        Ok(next)
    }

    pub fn add_pronunciation(
        &self,
        dialogue_id: &str,
        surface: &str,
        replacement: &str,
    ) -> Result<Self, AuthoringError> {
        let path = self.path(&Target::Dialogue(dialogue_id.into()))?;
        let mut next = self.clone();
        let dialogue = next
            .raw_mut()
            .pointer_mut(&path)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| shape(&path, "dialogue object"))?;
        let array = dialogue
            .entry("pronunciation_overrides")
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .ok_or_else(|| shape(&path, "pronunciation array"))?;
        array.push(json!({ "surface": surface, "replacement": replacement }));
        Ok(next)
    }

    pub fn remove_pronunciation(
        &self,
        dialogue_id: &str,
        index: usize,
    ) -> Result<Self, AuthoringError> {
        let path = self.pronunciation_path(dialogue_id)?;
        let mut next = self.clone();
        let array = next
            .raw_mut()
            .pointer_mut(&path)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| AuthoringError::MissingPronunciation {
                dialogue_id: dialogue_id.into(),
                index,
            })?;
        if index >= array.len() {
            return Err(AuthoringError::MissingPronunciation {
                dialogue_id: dialogue_id.into(),
                index,
            });
        }
        array.remove(index);
        Ok(next)
    }

    fn pronunciation_path(&self, dialogue_id: &str) -> Result<String, AuthoringError> {
        Ok(format!(
            "{}/pronunciation_overrides",
            self.path(&Target::Dialogue(dialogue_id.into()))?
        ))
    }

    pub fn changes_from(&self, base: &Self) -> Vec<Change> {
        let mut changes = vec![];
        compare(Some(&base.raw), Some(&self.raw), "", None, &mut changes);
        changes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
    Reordered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub entity_id: Option<String>,
    pub kind: ChangeKind,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Tracks each unfinished field independently. Cancelling one field cannot hide another draft.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputActivity {
    pending: HashSet<String>,
}

impl InputActivity {
    pub fn mark(&self, id: &str, pending: bool) -> Self {
        let mut next = self.clone();
        if pending {
            next.pending.insert(id.into());
        } else {
            next.pending.remove(id);
        }
        next
    }

    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

/// A bounded local history. Undo restores the exact complete value, including deleted IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    current: Document,
    past: Vec<Document>,
    future: Vec<Document>,
}

impl History {
    pub fn new(document: Document) -> Self {
        Self {
            current: document,
            past: vec![],
            future: vec![],
        }
    }
    pub fn current(&self) -> &Document {
        &self.current
    }
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
    pub fn apply(&self, document: Document) -> Self {
        if document == self.current {
            return self.clone();
        }
        let mut next = self.clone();
        next.past.push(self.current.clone());
        if next.past.len() > HISTORY_LIMIT {
            next.past.remove(0);
        }
        next.current = document;
        next.future.clear();
        next
    }
    pub fn undo(&self) -> Self {
        let mut next = self.clone();
        if let Some(previous) = next.past.pop() {
            next.future.push(next.current);
            next.current = previous;
        }
        next
    }
    pub fn redo(&self) -> Self {
        let mut next = self.clone();
        if let Some(following) = next.future.pop() {
            next.past.push(next.current);
            next.current = following;
        }
        next
    }
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().into()
}
fn values(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or_default()
}
fn find_in(value: &Value, id: &str) -> Option<usize> {
    find_in_slice(values(value), id)
}
fn find_in_slice(array: &[Value], id: &str) -> Option<usize> {
    array
        .iter()
        .position(|value| value["id"].as_str() == Some(id))
}
fn contains_id(value: &Value, id: &str) -> bool {
    match value {
        Value::Object(object) => {
            object.get("id").and_then(Value::as_str) == Some(id)
                || object.values().any(|nested| contains_id(nested, id))
        }
        Value::Array(array) => array.iter().any(|nested| contains_id(nested, id)),
        _ => false,
    }
}
fn shape(path: &str, expected: &'static str) -> AuthoringError {
    AuthoringError::Shape {
        path: path.into(),
        expected,
    }
}

/// `Value` can represent u64/i64 and finite f64. Refuse a broader JSON number if parsing
/// would round it, including numbers in otherwise unknown metadata. No token is rewritten.
fn check_number_precision(source: &str) -> Result<(), AuthoringError> {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            index += 1;
            while index < bytes.len() {
                match bytes[index] {
                    b'\\' => index += 2,
                    b'"' => {
                        index += 1;
                        break;
                    }
                    _ => index += 1,
                }
            }
        } else if bytes[index] == b'-' || bytes[index].is_ascii_digit() {
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_digit()
                    || matches!(bytes[index], b'-' | b'+' | b'.' | b'e' | b'E'))
            {
                index += 1;
            }
            let token = &source[start..index];
            // The full document was already parsed, so these tokens have valid JSON syntax.
            let number: serde_json::Number = serde_json::from_str(token)
                .map_err(|_| AuthoringError::NumericPrecisionLoss(token.into()))?;
            let original = normalized_decimal(token)
                .ok_or_else(|| AuthoringError::NumericPrecisionLoss(token.into()))?;
            if Some(original) != normalized_decimal(&number.to_string()) {
                return Err(AuthoringError::NumericPrecisionLoss(token.into()));
            }
        } else {
            index += 1;
        }
    }
    Ok(())
}

/// Compare decimal meaning without converting the decimal coefficient to a machine integer.
fn normalized_decimal(source: &str) -> Option<(bool, String, i64)> {
    let negative = source.starts_with('-');
    let unsigned = source.strip_prefix('-').unwrap_or(source);
    let (mantissa, exponent) = unsigned
        .split_once(['e', 'E'])
        .map_or((unsigned, 0), |(mantissa, exponent)| {
            (mantissa, exponent.parse::<i64>().unwrap_or(i64::MIN))
        });
    if exponent == i64::MIN {
        return None;
    }
    let fraction = mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    let mut coefficient = mantissa.replace('.', "").trim_start_matches('0').to_owned();
    if coefficient.is_empty() {
        return Some((false, "0".into(), 0));
    }
    let trailing = coefficient.len() - coefficient.trim_end_matches('0').len();
    coefficient.truncate(coefficient.len() - trailing);
    let power = exponent
        .checked_sub(fraction as i64)?
        .checked_add(trailing as i64)?;
    Some((negative, coefficient, power))
}
fn array_position(path: &str) -> Result<(&str, usize), AuthoringError> {
    let (parent, index) = path
        .rsplit_once('/')
        .ok_or_else(|| AuthoringError::NotInCollection(path.into()))?;
    Ok((
        parent,
        index
            .parse()
            .map_err(|_| AuthoringError::NotInCollection(path.into()))?,
    ))
}
fn require_object<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a Map<String, Value>, AuthoringError> {
    value.as_object().ok_or_else(|| shape(path, "object"))
}
fn require_text(value: &Value, path: &str) -> Result<(), AuthoringError> {
    if value.as_str().is_none() {
        Err(shape(path, "string"))
    } else {
        Ok(())
    }
}
fn require_array<'a>(value: &'a Value, path: &str) -> Result<&'a [Value], AuthoringError> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| shape(path, "array"))
}
fn check_entity(
    value: &Value,
    path: &str,
    fields: &[&str],
    ids: &mut HashSet<String>,
) -> Result<(), AuthoringError> {
    require_object(value, path)?;
    for field in fields {
        require_text(&value[*field], &format!("{path}/{field}"))?;
    }
    let id = text(value, "id");
    if !ids.insert(id.clone()) {
        return Err(AuthoringError::DuplicateId(id));
    }
    Ok(())
}

fn validate_shape(raw: &Value) -> Result<(), AuthoringError> {
    require_object(raw, "")?;
    require_text(&raw["schema_version"], "/schema_version")?;
    let version = text(raw, "schema_version");
    if version != SCHEMA_VERSION {
        return Err(AuthoringError::UnsupportedVersion(version));
    }
    let mut ids = HashSet::new();
    check_entity(
        &raw["work"],
        "/work",
        &["id", "title", "source_ref", "rights_record_id"],
        &mut ids,
    )?;
    check_entity(
        &raw["adaptation"],
        "/adaptation",
        &["id", "language", "rights_record_id"],
        &mut ids,
    )?;
    for (index, reference) in require_array(
        &raw["adaptation"]["provenance_refs"],
        "/adaptation/provenance_refs",
    )?
    .iter()
    .enumerate()
    {
        require_text(reference, &format!("/adaptation/provenance_refs/{index}"))?;
    }
    for (index, character) in require_array(&raw["characters"], "/characters")?
        .iter()
        .enumerate()
    {
        check_entity(
            character,
            &format!("/characters/{index}"),
            &["id", "name", "role", "personality"],
            &mut ids,
        )?;
    }
    check_entity(&raw["episode"], "/episode", &["id", "title"], &mut ids)?;
    for (act_index, act) in require_array(&raw["episode"]["acts"], "/episode/acts")?
        .iter()
        .enumerate()
    {
        let act_path = format!("/episode/acts/{act_index}");
        check_entity(act, &act_path, &["id", "title"], &mut ids)?;
        for (scene_index, scene) in require_array(&act["scenes"], &format!("{act_path}/scenes"))?
            .iter()
            .enumerate()
        {
            let scene_path = format!("{act_path}/scenes/{scene_index}");
            check_entity(scene, &scene_path, &["id", "title"], &mut ids)?;
            for (index, dialogue) in
                require_array(&scene["dialogues"], &format!("{scene_path}/dialogues"))?
                    .iter()
                    .enumerate()
            {
                let path = format!("{scene_path}/dialogues/{index}");
                check_entity(dialogue, &path, &["id", "speaker_id", "text"], &mut ids)?;
                require_object(&dialogue["delivery"], &format!("{path}/delivery"))?;
                require_text(
                    &dialogue["delivery"]["emotion"],
                    &format!("{path}/delivery/emotion"),
                )?;
                if dialogue["delivery"]["intensity_permille"]
                    .as_u64()
                    .is_none()
                {
                    return Err(shape(
                        &format!("{path}/delivery/intensity_permille"),
                        "nonnegative integer",
                    ));
                }
                if let Some(pronunciations) = dialogue.get("pronunciation_overrides") {
                    for (index, pronunciation) in
                        require_array(pronunciations, &format!("{path}/pronunciation_overrides"))?
                            .iter()
                            .enumerate()
                    {
                        let path = format!("{path}/pronunciation_overrides/{index}");
                        require_object(pronunciation, &path)?;
                        require_text(&pronunciation["surface"], &format!("{path}/surface"))?;
                        require_text(
                            &pronunciation["replacement"],
                            &format!("{path}/replacement"),
                        )?;
                    }
                }
            }
            if let Some(cues) = scene.get("sound_cues") {
                for (index, cue) in require_array(cues, &format!("{scene_path}/sound_cues"))?
                    .iter()
                    .enumerate()
                {
                    let path = format!("{scene_path}/sound_cues/{index}");
                    check_entity(cue, &path, &["id", "kind", "description"], &mut ids)?;
                    require_object(&cue["anchor"], &format!("{path}/anchor"))?;
                    require_text(
                        &cue["anchor"]["dialogue_id"],
                        &format!("{path}/anchor/dialogue_id"),
                    )?;
                    require_text(&cue["anchor"]["edge"], &format!("{path}/anchor/edge"))?;
                    if let Some(asset) = cue.get("asset") {
                        require_object(asset, &format!("{path}/asset"))?;
                        require_text(&asset["id"], &format!("{path}/asset/id"))?;
                        require_text(
                            &asset["rights_record_id"],
                            &format!("{path}/asset/rights_record_id"),
                        )?;
                    }
                }
            }
        }
    }
    // Provenance is preserved verbatim, never projected into editable authoring data.
    require_array(&raw["provenance"], "/provenance")?;
    Ok(())
}

fn pointer_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}
fn display_value(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
fn keyed(array: &[Value]) -> bool {
    let mut ids = HashSet::new();
    !array.is_empty()
        && array.iter().all(|value| {
            value
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| ids.insert(id))
        })
}
fn compare(
    before: Option<&Value>,
    after: Option<&Value>,
    path: &str,
    identity: Option<&str>,
    changes: &mut Vec<Change>,
) {
    if before == after {
        return;
    }
    let identity = after
        .or(before)
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .or(identity);
    match (before, after) {
        (Some(Value::Object(left)), Some(Value::Object(right))) => {
            let keys: BTreeSet<&String> = left.keys().chain(right.keys()).collect();
            for key in keys {
                compare(
                    left.get(key),
                    right.get(key),
                    &format!("{path}/{}", pointer_key(key)),
                    identity,
                    changes,
                );
            }
        }
        (Some(Value::Array(left)), Some(Value::Array(right)))
            if (keyed(left) || left.is_empty()) && (keyed(right) || right.is_empty()) =>
        {
            for (index, value) in right.iter().enumerate() {
                let id = text(value, "id");
                let previous = find_in_slice(left, &id);
                let row_path = format!("{path}/{index}");
                compare(
                    previous.map(|index| &left[index]),
                    Some(value),
                    &row_path,
                    Some(&id),
                    changes,
                );
                if let Some(previous) = previous {
                    if previous != index {
                        changes.push(Change {
                            path: row_path,
                            entity_id: Some(id),
                            kind: ChangeKind::Reordered,
                            before: Some(previous.to_string()),
                            after: Some(index.to_string()),
                        });
                    }
                }
            }
            for (index, value) in left.iter().enumerate() {
                let id = text(value, "id");
                if find_in_slice(right, &id).is_none() {
                    compare(
                        Some(value),
                        None,
                        &format!("{path}/{index}"),
                        Some(&id),
                        changes,
                    );
                }
            }
        }
        (Some(Value::Array(left)), Some(Value::Array(right))) => {
            for index in 0..left.len().max(right.len()) {
                compare(
                    left.get(index),
                    right.get(index),
                    &format!("{path}/{index}"),
                    identity,
                    changes,
                );
            }
        }
        _ => changes.push(Change {
            path: path.into(),
            entity_id: identity.map(str::to_owned),
            kind: match (before, after) {
                (None, _) => ChangeKind::Added,
                (_, None) => ChangeKind::Removed,
                _ => ChangeKind::Changed,
            },
            before: before.map(display_value),
            after: after.map(display_value),
        }),
    }
}

/// Duplicate JSON keys are rejected before projection: accepting them would silently lose data.
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("JSON without duplicate object keys")
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::from(value)))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::from(value)))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::from(value)))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|value| UniqueValue(Value::Number(value)))
                    .ok_or_else(|| E::custom("nonfinite number"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(value.into())))
            }
            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(value)))
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut values = vec![];
                while let Some(UniqueValue(value)) = access.next_element()? {
                    values.push(value);
                }
                Ok(UniqueValue(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut object = Map::new();
                while let Some((key, UniqueValue(value))) =
                    access.next_entry::<String, UniqueValue>()?
                {
                    if object.contains_key(&key) {
                        return Err(de::Error::custom(format!("duplicate object key: {key}")));
                    }
                    object.insert(key, value);
                }
                Ok(UniqueValue(Value::Object(object)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = include_str!(
        "../../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json"
    );

    fn document() -> Document {
        Document::parse(FIXTURE).unwrap()
    }

    #[test]
    fn structured_edit_preserves_every_other_field_and_unknown_metadata() {
        let mut raw: Value = serde_json::from_str(FIXTURE).unwrap();
        raw["future_annotation"] = json!({"not_yet_editable": [null, true, "giữ nguyên"]});
        raw["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["delivery"]["future"] =
            json!({"pace": 17});
        let original = Document::parse(&raw.to_string()).unwrap();
        let changed = original
            .set(
                &Target::Dialogue("dialogue-02".into()),
                Field::Text,
                "Đêm nay, Vọng Đài vẫn sáng.",
            )
            .unwrap();
        let mut expected = raw;
        expected["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["text"] =
            json!("Đêm nay, Vọng Đài vẫn sáng.");
        assert_eq!(*changed.raw, expected);
        assert_eq!(original, Document::parse(&original.to_json()).unwrap());
        assert_eq!(
            changed.changes_from(&original),
            vec![Change {
                path: "/episode/acts/0/scenes/0/dialogues/1/text".into(),
                entity_id: Some("dialogue-02".into()),
                kind: ChangeKind::Changed,
                before: Some("Ngày mai, mình có diễn tiếp không?".into()),
                after: Some("Đêm nay, Vọng Đài vẫn sáng.".into())
            }]
        );
    }

    #[test]
    fn all_current_fixtures_read_without_normalizing_the_raw_document() {
        for fixture in [
            FIXTURE,
            include_str!(
                "../../../../packages/contracts/fixtures/script-ir/0.1.0/accept/provenance.json"
            ),
        ] {
            let document = Document::parse(fixture).unwrap();
            assert_eq!(
                *document.raw,
                serde_json::from_str::<Value>(fixture).unwrap()
            );
        }
    }

    #[test]
    fn unsupported_and_ambiguous_inputs_fail_before_any_edit() {
        let mut raw = document().raw.as_ref().clone();
        raw["schema_version"] = json!("0.2.0");
        assert_eq!(
            Document::parse(&raw.to_string()),
            Err(AuthoringError::UnsupportedVersion("0.2.0".into()))
        );
        assert_eq!(
            Document::parse("{}"),
            Err(shape("/schema_version", "string"))
        );
        let error = Document::parse(&FIXTURE.replace(
            "\"title\": \"Một lời hẹn\"",
            "\"title\": \"Một lời hẹn\", \"title\": \"lost\"",
        ))
        .unwrap_err();
        assert!(
            matches!(error, AuthoringError::InvalidJson { detail, .. } if detail.starts_with("duplicate object key: title"))
        );
        raw["schema_version"] = json!(SCHEMA_VERSION);
        raw["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["delivery"] = json!(false);
        assert_eq!(
            Document::parse(&raw.to_string()),
            Err(shape(
                "/episode/acts/0/scenes/0/dialogues/0/delivery",
                "object"
            ))
        );
    }

    #[test]
    fn unknown_numeric_metadata_never_silently_loses_precision() {
        for token in ["18446744073709551616", "0.123456789012345678901"] {
            let input = FIXTURE.replacen('{', &format!("{{\"future_number\":{token},"), 1);
            assert_eq!(
                Document::parse(&input),
                Err(AuthoringError::NumericPrecisionLoss(token.into()))
            );
        }
        for token in ["18446744073709551615", "125e-2", "-0", "0.1"] {
            let input = FIXTURE.replacen('{', &format!("{{\"future_number\":{token},"), 1);
            let original = Document::parse(&input).unwrap();
            let edited = original
                .set(&Target::Work, Field::Title, "Vọng Đài")
                .unwrap();
            assert_eq!(edited.raw["future_number"], original.raw["future_number"]);
        }
        // Numeric-looking characters inside JSON strings are ordinary user content.
        assert_eq!(
            check_number_precision(r#"{"text":"18446744073709551616 \" 0.123456789012345678901"}"#),
            Ok(())
        );
    }

    #[test]
    fn narration_and_cue_edits_keep_asset_and_rights_records_exact() {
        let initial = document();
        let edited = initial
            .set(
                &Target::Character("narrator".into()),
                Field::Name,
                "Lão nhân kể chuyện",
            )
            .unwrap()
            .set(
                &Target::Dialogue("dialogue-01".into()),
                Field::Emotion,
                "sad",
            )
            .unwrap()
            .set(
                &Target::Dialogue("dialogue-01".into()),
                Field::Intensity,
                "700",
            )
            .unwrap()
            .set(
                &Target::Cue("cue-02".into()),
                Field::CueDescription,
                "Tiếng gió bên thềm",
            )
            .unwrap();
        assert_eq!(
            edited.acts()[0].scenes[0].dialogues[0].speaker_id,
            "narrator"
        );
        assert_eq!(
            edited.acts()[0].scenes[0].dialogues[0].intensity_permille,
            700
        );
        assert_eq!(edited.raw["provenance"], initial.raw["provenance"]);
        assert_eq!(
            edited.raw["work"]["rights_record_id"],
            initial.raw["work"]["rights_record_id"]
        );
        assert_eq!(
            edited.acts()[0].scenes[1].sound_cues[0].asset,
            initial.acts()[0].scenes[1].sound_cues[0].asset
        );
        assert_eq!(
            edited.set(
                &Target::Dialogue("dialogue-01".into()),
                Field::Intensity,
                "1.2"
            ),
            Err(AuthoringError::InvalidInteger("1.2".into()))
        );
    }

    #[test]
    fn new_rows_have_permanent_unique_ids_and_dangling_deletions_are_refused() {
        let initial = document();
        assert_eq!(
            initial.insert_character("an", "Other", "character", "Other"),
            Err(AuthoringError::DuplicateId("an".into()))
        );
        assert_eq!(
            initial.insert_character("bad id", "Other", "character", "Other"),
            Err(AuthoringError::InvalidId("bad id".into()))
        );
        assert_eq!(
            initial.remove(&Target::Character("an".into())),
            Err(AuthoringError::Referenced {
                target: Target::Character("an".into()),
                references: vec!["dialogue-02".into(), "dialogue-04".into()]
            })
        );
        assert_eq!(
            initial.remove(&Target::Dialogue("dialogue-01".into())),
            Err(AuthoringError::Referenced {
                target: Target::Dialogue("dialogue-01".into()),
                references: vec!["cue-01".into()]
            })
        );
        let edited = initial
            .insert_character("linh", "Linh", "character", "Điềm tĩnh")
            .unwrap()
            .insert_dialogue("scene-01", "dialogue-linh", "linh", "Tôi đã về.")
            .unwrap()
            .insert_cue(
                "scene-01",
                "cue-linh",
                "sfx",
                "Chuông xa",
                "dialogue-linh",
                "end",
            )
            .unwrap();
        assert_eq!(edited.acts()[0].scenes[0].dialogues[3].id, "dialogue-linh");
        assert_eq!(
            edited.acts()[0].scenes[0].sound_cues[1].dialogue_id,
            "dialogue-linh"
        );
        assert_eq!(
            edited.insert_cue(
                "scene-02",
                "cue-other",
                "sfx",
                "Chuông",
                "dialogue-linh",
                "start"
            ),
            Err(AuthoringError::NotInCollection("dialogue-linh".into()))
        );
    }

    #[test]
    fn new_hierarchy_roundtrips_and_ids_cannot_collide_with_unedited_provenance() {
        let initial = document();
        let changed = initial
            .insert_act("act-new", "Chương hai — Đường về")
            .unwrap()
            .insert_scene("act-new", "scene-new", "Trước cổng Vọng Đài")
            .unwrap()
            .insert_dialogue("scene-new", "dialogue-new", "narrator", "Mưa đã ngừng.")
            .unwrap()
            .insert_cue(
                "scene-new",
                "cue-new",
                "ambience",
                "Giọt nước sau mưa",
                "dialogue-new",
                "start",
            )
            .unwrap();
        assert_eq!(Document::parse(&changed.to_json()).unwrap(), changed);
        assert_eq!(changed.acts()[1].scenes[0].dialogues[0].id, "dialogue-new");
        assert_eq!(changed.raw["provenance"], initial.raw["provenance"]);
        assert_eq!(
            initial.insert_act("origin-demo", "Conflict"),
            Err(AuthoringError::DuplicateId("origin-demo".into()))
        );
        assert_eq!(
            changed.remove(&Target::Act("act-new".into())).unwrap(),
            initial
        );
        assert_eq!(
            initial.set(&Target::Work, Field::Text, "wrong field"),
            Err(AuthoringError::UnsupportedField {
                target: Target::Work,
                field: Field::Text
            })
        );
        assert_eq!(
            initial.field(&Target::Scene("missing-scene".into()), Field::Title),
            Err(AuthoringError::MissingTarget(Target::Scene(
                "missing-scene".into()
            )))
        );
    }

    #[test]
    fn edits_to_optional_arrays_do_not_reconstruct_the_rest_of_a_row() {
        let mut raw = document().raw.as_ref().clone();
        raw["episode"]["acts"][0]["scenes"][0]
            .as_object_mut()
            .unwrap()
            .remove("sound_cues");
        raw["episode"]["acts"][0]["scenes"][0]["dialogues"][0]
            .as_object_mut()
            .unwrap()
            .remove("pronunciation_overrides");
        let original = Document::parse(&raw.to_string()).unwrap();
        let edited = original
            .insert_cue(
                "scene-01",
                "cue-new",
                "music",
                "Sáo trúc xa xa",
                "dialogue-01",
                "end",
            )
            .unwrap()
            .add_pronunciation("dialogue-01", "Ánh", "Ánh")
            .unwrap();
        raw["episode"]["acts"][0]["scenes"][0]["sound_cues"] = json!([{ "id":"cue-new", "kind":"music", "description":"Sáo trúc xa xa", "anchor":{"dialogue_id":"dialogue-01", "edge":"end"}}]);
        raw["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["pronunciation_overrides"] =
            json!([{ "surface":"Ánh", "replacement":"Ánh" }]);
        assert_eq!(*edited.raw, raw);
        assert_eq!(original.acts()[0].scenes[0].sound_cues, vec![]);
    }

    #[test]
    fn reorder_changes_only_sequence_and_field_paths_follow_current_positions() {
        let initial = document();
        let target = Target::Dialogue("dialogue-03".into());
        let moved = initial.move_before(&target, Some("dialogue-01")).unwrap();
        let mut expected = initial.raw.as_ref().clone();
        let dialogues = expected["episode"]["acts"][0]["scenes"][0]["dialogues"]
            .as_array_mut()
            .unwrap();
        let last = dialogues.pop().unwrap();
        dialogues.insert(0, last);
        assert_eq!(*moved.raw, expected);
        assert_eq!(
            moved.field_path(&target, Field::Text).unwrap(),
            "/episode/acts/0/scenes/0/dialogues/0/text"
        );
        assert!(moved
            .changes_from(&initial)
            .iter()
            .all(|change| change.kind == ChangeKind::Reordered));
        assert_eq!(
            initial.move_before(&target, Some("dialogue-04")),
            Err(AuthoringError::NotInCollection("dialogue-04".into()))
        );
    }

    #[test]
    fn diagnostic_paths_match_current_backend_fixtures_and_follow_reordered_rows() {
        let original = document();
        let cases: Value = serde_json::from_str(include_str!(
            "../../../../packages/contracts/fixtures/script-ir/0.1.0/cases.json"
        ))
        .unwrap();
        // The committed backend oracle contains these exact semantic paths.
        let cases_text = cases.to_string();
        let speaker = "episode/act:act-01/scene:scene-01/dialogue:dialogue-02/speaker_id";
        let anchor = "episode/act:act-01/scene:scene-01/cue:cue-01/anchor/dialogue_id";
        assert!(cases_text.contains(speaker));
        assert!(cases_text.contains(anchor));
        assert_eq!(
            original
                .diagnostic_paths(&Target::Dialogue("dialogue-02".into()), Field::Speaker)
                .unwrap(),
            vec!["/episode/acts/0/scenes/0/dialogues/1/speaker_id", speaker]
        );
        assert_eq!(
            original
                .diagnostic_paths(&Target::Cue("cue-01".into()), Field::CueDialogue)
                .unwrap(),
            vec![
                "/episode/acts/0/scenes/0/sound_cues/0/anchor/dialogue_id",
                anchor
            ]
        );
        assert_eq!(
            original
                .diagnostic_paths(&Target::Character("an".into()), Field::Name)
                .unwrap(),
            vec!["/characters/1/name", "character:an/name"]
        );
        assert_eq!(
            original
                .diagnostic_paths(&Target::Work, Field::Title)
                .unwrap(),
            vec!["/work/title", "work/title"]
        );
        assert_eq!(
            original
                .diagnostic_paths(&Target::Adaptation, Field::Language)
                .unwrap(),
            vec!["/adaptation/language", "adaptation/language"]
        );
        assert_eq!(
            original
                .diagnostic_paths(&Target::Episode, Field::Title)
                .unwrap(),
            vec!["/episode/title", "episode/title"]
        );
        assert_eq!(
            original
                .diagnostic_paths(&Target::Act("act-01".into()), Field::Title)
                .unwrap(),
            vec!["/episode/acts/0/title", "episode/act:act-01/title"]
        );
        assert_eq!(
            original
                .diagnostic_paths(&Target::Scene("scene-01".into()), Field::Title)
                .unwrap(),
            vec![
                "/episode/acts/0/scenes/0/title",
                "episode/act:act-01/scene:scene-01/title"
            ]
        );
        let moved = original
            .move_before(&Target::Dialogue("dialogue-02".into()), Some("dialogue-01"))
            .unwrap();
        assert_eq!(
            moved
                .diagnostic_paths(&Target::Dialogue("dialogue-02".into()), Field::Speaker)
                .unwrap(),
            vec!["/episode/acts/0/scenes/0/dialogues/0/speaker_id", speaker]
        );
    }

    #[test]
    fn resolving_one_pending_input_never_hides_another_uncommitted_field() {
        let activity = InputActivity::default()
            .mark("intensity:dialogue-01", true)
            .mark("text:dialogue-02", true);
        assert_eq!(activity.pending_count(), 2);
        let resolved_text = activity.mark("text:dialogue-02", false);
        assert!(resolved_text.is_pending());
        assert_eq!(resolved_text.pending_count(), 1);
        let escaped_intensity = resolved_text.mark("intensity:dialogue-01", false);
        assert!(!escaped_intensity.is_pending());
        assert_eq!(escaped_intensity.pending_count(), 0);
        assert_eq!(activity.mark("text:dialogue-02", true), activity);
    }

    #[test]
    fn pronunciation_patch_preserves_other_overrides_and_unknown_fields() {
        let mut initial = document();
        initial.raw_mut()["episode"]["acts"][0]["scenes"][1]["dialogues"][0]
            ["pronunciation_overrides"][0]["future_offset"] = json!(3);
        let updated = initial
            .set_pronunciation(
                "dialogue-04",
                0,
                PronunciationField::Replacement,
                "Vọng Đài mới",
            )
            .unwrap();
        assert_eq!(
            updated.raw["episode"]["acts"][0]["scenes"][1]["dialogues"][0]
                ["pronunciation_overrides"][0]["future_offset"],
            3
        );
        assert_eq!(
            updated.acts()[0].scenes[1].dialogues[0].pronunciation_overrides[0].surface,
            "Vọng Đài"
        );
        assert_eq!(
            updated.remove_pronunciation("dialogue-04", 9),
            Err(AuthoringError::MissingPronunciation {
                dialogue_id: "dialogue-04".into(),
                index: 9
            })
        );
        let added = updated
            .add_pronunciation("dialogue-03", "Có", "Có")
            .unwrap();
        assert_eq!(
            added.acts()[0].scenes[0].dialogues[2]
                .pronunciation_overrides
                .len(),
            1
        );
        assert_eq!(
            added.remove_pronunciation("dialogue-03", 0).unwrap(),
            updated
        );
    }

    #[test]
    fn structural_undo_restores_the_same_ids_and_future_is_cleared_by_a_new_edit() {
        let initial = document();
        let removed = initial
            .remove(&Target::Dialogue("dialogue-02".into()))
            .unwrap();
        let history = History::new(initial.clone()).apply(removed.clone());
        let undone = history.undo();
        assert_eq!(undone.current(), &initial);
        assert!(undone.can_redo());
        assert_eq!(undone.redo().current(), &removed);
        let different = undone.apply(
            initial
                .set(&Target::Work, Field::Title, "Đường về")
                .unwrap(),
        );
        assert!(!different.can_redo());
        assert_eq!(different.undo().current(), &initial);
    }
}
