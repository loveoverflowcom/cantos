//! Flattens admitted content into rows: one per stable ID, each with named, comparable fields.
//!
//! Every entity kind has exactly one constructor, so two rows of the same kind always carry the
//! same fields in the same order and can be compared pairwise.

use serde::Serialize;

use crate::script_ir::model::{
    Act, Adaptation, Character, Cue, Dialogue, Episode, Scene, ScriptContent, Source, SourceKind,
    Work,
};

/// Declaration order is the report order: the hierarchy from the top, then the shared tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Entity {
    Work,
    Adaptation,
    Episode,
    Act,
    Scene,
    Dialogue,
    Cue,
    Character,
    Provenance,
}

impl Entity {
    pub const ALL: [Entity; 9] = [
        Entity::Work,
        Entity::Adaptation,
        Entity::Episode,
        Entity::Act,
        Entity::Scene,
        Entity::Dialogue,
        Entity::Cue,
        Entity::Character,
        Entity::Provenance,
    ];
}

/// What a reviewer is asking about: each field belongs to exactly one aspect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Aspect {
    Text,
    Speaker,
    Delivery,
    Pronunciation,
    /// Reported for moves, never for a field: position is not a field of any entity.
    Order,
    Cue,
    Character,
    Title,
    Language,
    Provenance,
    Rights,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PronunciationOverride {
    pub surface: String,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum FieldValue {
    /// An optional field that is not set; serialized as `null`.
    Absent,
    Text(String),
    Permille(u16),
    TextList(Vec<String>),
    Pronunciations(Vec<PronunciationOverride>),
}

impl FieldValue {
    pub fn is_unset(&self) -> bool {
        match self {
            Self::Absent => true,
            Self::TextList(items) => items.is_empty(),
            Self::Pronunciations(items) => items.is_empty(),
            Self::Text(_) | Self::Permille(_) => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub field: &'static str,
    pub aspect: Aspect,
    pub value: FieldValue,
}

/// Zero-based position among the siblings of one parent in a single version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Place {
    pub parent: String,
    pub index: usize,
}

#[derive(Debug)]
pub struct Row {
    pub entity: Entity,
    pub id: String,
    /// `None` for entities with no position: the one work, adaptation and episode, and the
    /// characters and provenance sources, which validation keeps sorted by ID.
    pub place: Option<Place>,
    pub fields: Vec<Field>,
}

/// Rows in document order; entities of one kind keep their relative script order.
pub fn rows(script: &ScriptContent) -> Vec<Row> {
    let episode = &script.episode;
    let mut rows = vec![
        work_row(&script.work),
        adaptation_row(&script.adaptation),
        episode_row(episode),
    ];
    for (act_index, act) in episode.acts.iter().enumerate() {
        rows.push(act_row(act, &episode.id.0, act_index));
        for (scene_index, scene) in act.scenes.iter().enumerate() {
            rows.push(scene_row(scene, &act.id.0, scene_index));
            rows.extend(
                (scene.dialogues.iter().enumerate())
                    .map(|(index, dialogue)| dialogue_row(dialogue, &scene.id.0, index)),
            );
            rows.extend(
                (scene.sound_cues.iter().enumerate())
                    .map(|(index, cue)| cue_row(cue, &scene.id.0, index)),
            );
        }
    }
    rows.extend(script.characters.iter().map(character_row));
    rows.extend(script.provenance.iter().map(provenance_row));
    rows
}

fn text(field: &'static str, aspect: Aspect, value: &str) -> Field {
    Field {
        field,
        aspect,
        value: FieldValue::Text(value.to_owned()),
    }
}

fn optional_text(field: &'static str, aspect: Aspect, value: Option<&str>) -> Field {
    Field {
        field,
        aspect,
        value: value.map_or(FieldValue::Absent, |value| {
            FieldValue::Text(value.to_owned())
        }),
    }
}

fn row(entity: Entity, id: &str, place: Option<Place>, fields: Vec<Field>) -> Row {
    Row {
        entity,
        id: id.to_owned(),
        place,
        fields,
    }
}

fn placed(parent: &str, index: usize) -> Option<Place> {
    Some(Place {
        parent: parent.to_owned(),
        index,
    })
}

fn work_row(work: &Work) -> Row {
    let fields = vec![
        text("title", Aspect::Title, &work.title.0),
        text("source_ref", Aspect::Provenance, &work.source_ref.0),
        text("rights_record_id", Aspect::Rights, &work.rights_record_id.0),
    ];
    row(Entity::Work, &work.id.0, None, fields)
}

fn adaptation_row(adaptation: &Adaptation) -> Row {
    let refs = adaptation.provenance_refs.iter();
    let fields = vec![
        text("language", Aspect::Language, adaptation.language.as_str()),
        text(
            "rights_record_id",
            Aspect::Rights,
            &adaptation.rights_record_id.0,
        ),
        Field {
            field: "provenance_refs",
            aspect: Aspect::Provenance,
            value: FieldValue::TextList(refs.map(|id| id.0.clone()).collect()),
        },
    ];
    row(Entity::Adaptation, &adaptation.id.0, None, fields)
}

fn episode_row(episode: &Episode) -> Row {
    let fields = vec![text("title", Aspect::Title, &episode.title.0)];
    row(Entity::Episode, &episode.id.0, None, fields)
}

fn act_row(act: &Act, episode_id: &str, index: usize) -> Row {
    let fields = vec![text("title", Aspect::Title, &act.title.0)];
    row(Entity::Act, &act.id.0, placed(episode_id, index), fields)
}

fn scene_row(scene: &Scene, act_id: &str, index: usize) -> Row {
    let fields = vec![text("title", Aspect::Title, &scene.title.0)];
    row(Entity::Scene, &scene.id.0, placed(act_id, index), fields)
}

fn dialogue_row(dialogue: &Dialogue, scene_id: &str, index: usize) -> Row {
    let overrides = dialogue.pronunciation.iter().map(|pronunciation| {
        let (surface, replacement) = (&pronunciation.surface, &pronunciation.replacement);
        PronunciationOverride {
            surface: surface.0.clone(),
            replacement: replacement.0.clone(),
        }
    });
    let fields = vec![
        text("speaker_id", Aspect::Speaker, &dialogue.speaker.0),
        text("text", Aspect::Text, &dialogue.text.0),
        text(
            "delivery.emotion",
            Aspect::Delivery,
            dialogue.delivery.emotion.as_str(),
        ),
        Field {
            field: "delivery.intensity_permille",
            aspect: Aspect::Delivery,
            value: FieldValue::Permille(dialogue.delivery.intensity_permille),
        },
        Field {
            field: "pronunciation_overrides",
            aspect: Aspect::Pronunciation,
            value: FieldValue::Pronunciations(overrides.collect()),
        },
    ];
    row(
        Entity::Dialogue,
        &dialogue.id.0,
        placed(scene_id, index),
        fields,
    )
}

fn cue_row(cue: &Cue, scene_id: &str, index: usize) -> Row {
    let asset = cue.asset.as_ref();
    let fields = vec![
        text("kind", Aspect::Cue, cue.kind.as_str()),
        text("description", Aspect::Cue, &cue.description.0),
        text("anchor.dialogue_id", Aspect::Cue, &cue.dialogue.0),
        text("anchor.edge", Aspect::Cue, cue.edge.as_str()),
        optional_text("asset.id", Aspect::Cue, asset.map(|a| a.id.0.as_str())),
        optional_text(
            "asset.rights_record_id",
            Aspect::Rights,
            asset.map(|a| a.rights_record_id.0.as_str()),
        ),
    ];
    row(Entity::Cue, &cue.id.0, placed(scene_id, index), fields)
}

fn character_row(character: &Character) -> Row {
    let fields = vec![
        text("name", Aspect::Character, &character.name.0),
        text("role", Aspect::Character, character.role.as_str()),
        text("personality", Aspect::Character, &character.personality.0),
    ];
    row(Entity::Character, &character.id.0, None, fields)
}

fn provenance_row(source: &Source) -> Row {
    let (kind, generation) = match &source.kind {
        SourceKind::Original => ("original", None),
        SourceKind::Imported => ("imported", None),
        SourceKind::Generated {
            generation_record_id,
        } => ("generated", Some(generation_record_id.0.as_str())),
    };
    let fields = vec![
        text("kind", Aspect::Provenance, kind),
        text(
            "source_record_id",
            Aspect::Provenance,
            &source.source_record_id.0,
        ),
        text(
            "rights_record_id",
            Aspect::Rights,
            &source.rights_record_id.0,
        ),
        optional_text("generation_record_id", Aspect::Provenance, generation),
    ];
    row(Entity::Provenance, &source.id.0, None, fields)
}
