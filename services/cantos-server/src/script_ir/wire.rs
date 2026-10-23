use serde::Deserialize;

use super::model::{CueKind, Edge, Emotion, Language, Role};
use super::validation::Diagnostic;

pub const WRITE_VERSION: &str = "0.1.0";
const MAX_ITEMS: usize = 10_000;
pub const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum ReadError {
    DocumentTooLarge {
        max_bytes: usize,
    },
    InvalidDocument {
        line: usize,
        column: usize,
        detail: String,
    },
    MissingSchemaVersion,
    UnsupportedSchemaVersion {
        found: String,
        supported: &'static str,
    },
    Shape(Vec<ShapeDiagnostic>),
    Semantic(Vec<Diagnostic>),
    NonCanonicalDocument,
}

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct ShapeDiagnostic {
    pub path: String,
    pub rule: ShapeRule,
}

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeRule {
    IdFormat,
    TextLength,
    ArrayLength,
    IntensityRange,
}

#[derive(Deserialize)]
struct VersionProbe {
    schema_version: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawScript {
    #[serde(rename = "schema_version")]
    pub _schema_version: SchemaVersion,
    pub work: RawWork,
    pub adaptation: RawAdaptation,
    pub characters: Vec<RawCharacter>,
    pub episode: RawEpisode,
    pub provenance: Vec<RawSource>,
}

#[derive(Clone, Deserialize)]
pub(super) enum SchemaVersion {
    #[serde(rename = "0.1.0")]
    V0_1,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawWork {
    pub id: String,
    pub title: String,
    pub source_ref: String,
    pub rights_record_id: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawAdaptation {
    pub id: String,
    pub language: Language,
    pub rights_record_id: String,
    pub provenance_refs: Vec<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCharacter {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub personality: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawEpisode {
    pub id: String,
    pub title: String,
    pub acts: Vec<RawAct>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawAct {
    pub id: String,
    pub title: String,
    pub scenes: Vec<RawScene>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawScene {
    pub id: String,
    pub title: String,
    pub dialogues: Vec<RawDialogue>,
    #[serde(default)]
    pub sound_cues: Vec<RawCue>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawDialogue {
    pub id: String,
    pub speaker_id: String,
    pub text: String,
    pub delivery: RawDelivery,
    #[serde(default)]
    pub pronunciation_overrides: Vec<RawPronunciation>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawDelivery {
    pub emotion: Emotion,
    #[serde(deserialize_with = "integer_intensity")]
    pub intensity_permille: u16,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPronunciation {
    pub surface: String,
    pub replacement: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCue {
    pub id: String,
    pub kind: CueKind,
    pub description: String,
    pub anchor: RawAnchor,
    #[serde(default, deserialize_with = "asset_selection")]
    pub asset: Option<RawAsset>,
}

fn asset_selection<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<RawAsset>, D::Error> {
    RawAsset::deserialize(decoder).map(Some)
}

// JSON Schema's integer includes 300.0 and 3e2; normalize those wire spellings to u16.
fn integer_intensity<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<u16, D::Error> {
    let number = Box::<serde_json::value::RawValue>::deserialize(decoder)?;
    exact_unsigned_integer(number.get())
        .ok_or_else(|| serde::de::Error::custom("expected an unsigned integer"))
}

// Work on the JSON number token: binary floats can round fractions to integers or zero.
// JSON syntax was checked by RawValue; only exact integrality and the u16 bound live here.
fn exact_unsigned_integer(number: &str) -> Option<u16> {
    if !number.starts_with(|ch: char| ch.is_ascii_digit() || ch == '-') {
        return None;
    }
    let unsigned = number.strip_prefix('-').unwrap_or(number);
    let (coefficient, exponent) = unsigned.split_once(['e', 'E']).unwrap_or((unsigned, "0"));
    let fraction_digits = coefficient
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    let digits = coefficient.replace('.', "");
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() {
        return Some(0);
    }
    if number.starts_with('-') {
        return None;
    }
    let trimmed = significant.trim_end_matches('0');
    let scale = exponent
        .parse::<i64>()
        .ok()?
        .checked_sub(fraction_digits as i64)?
        .checked_add((significant.len() - trimmed.len()) as i64)?;
    if !(0..=5).contains(&scale) || trimmed.len() > 5 - scale as usize {
        return None;
    }
    trimmed
        .parse::<u16>()
        .ok()?
        .checked_mul(10u16.checked_pow(scale as u32)?)
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawAnchor {
    pub dialogue_id: String,
    pub edge: Edge,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawAsset {
    pub id: String,
    pub rights_record_id: String,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum RawSource {
    Original {
        id: String,
        source_record_id: String,
        rights_record_id: String,
    },
    Imported {
        id: String,
        source_record_id: String,
        rights_record_id: String,
    },
    Generated {
        id: String,
        source_record_id: String,
        rights_record_id: String,
        generation_record_id: String,
    },
}

impl RawSource {
    pub fn fields(&self) -> (&str, &str, &str, Option<&str>) {
        match self {
            Self::Original {
                id,
                source_record_id,
                rights_record_id,
            }
            | Self::Imported {
                id,
                source_record_id,
                rights_record_id,
            } => (id, source_record_id, rights_record_id, None),
            Self::Generated {
                id,
                source_record_id,
                rights_record_id,
                generation_record_id,
            } => (
                id,
                source_record_id,
                rights_record_id,
                Some(generation_record_id),
            ),
        }
    }
}

fn document_error(error: serde_json::Error) -> ReadError {
    ReadError::InvalidDocument {
        line: error.line(),
        column: error.column(),
        detail: error.to_string(),
    }
}

pub(super) fn decode(bytes: &[u8]) -> Result<RawScript, ReadError> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(ReadError::DocumentTooLarge {
            max_bytes: MAX_DOCUMENT_BYTES,
        });
    }
    let probe: VersionProbe = serde_json::from_slice(bytes).map_err(document_error)?;
    match probe.schema_version {
        None => return Err(ReadError::MissingSchemaVersion),
        Some(version) if version != WRITE_VERSION => {
            return Err(ReadError::UnsupportedSchemaVersion {
                found: version,
                supported: WRITE_VERSION,
            })
        }
        Some(_) => {}
    }
    let raw: RawScript = serde_json::from_slice(bytes).map_err(document_error)?;
    let issues = check_shape(&raw);
    if issues.is_empty() {
        Ok(raw)
    } else {
        Err(ReadError::Shape(issues))
    }
}

struct ShapeCheck(Vec<ShapeDiagnostic>);

impl ShapeCheck {
    fn issue(&mut self, path: String, rule: ShapeRule) {
        self.0.push(ShapeDiagnostic { path, rule });
    }

    fn id(&mut self, id: &str, path: String) {
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
        {
            self.issue(path, ShapeRule::IdFormat);
        }
    }

    fn text(&mut self, text: &str, path: String) {
        let length = text.chars().count();
        if !(1..=10_000).contains(&length) {
            self.issue(path, ShapeRule::TextLength);
        }
    }

    fn array<T>(&mut self, items: &[T], minimum: usize, path: String) {
        if items.len() < minimum || items.len() > MAX_ITEMS {
            self.issue(path, ShapeRule::ArrayLength);
        }
    }
}

fn check_shape(raw: &RawScript) -> Vec<ShapeDiagnostic> {
    let mut check = ShapeCheck(Vec::new());
    check.id(&raw.work.id, "/work/id".into());
    check.text(&raw.work.title, "/work/title".into());
    check.id(&raw.work.source_ref, "/work/source_ref".into());
    check.id(&raw.work.rights_record_id, "/work/rights_record_id".into());
    check.id(&raw.adaptation.id, "/adaptation/id".into());
    check.id(
        &raw.adaptation.rights_record_id,
        "/adaptation/rights_record_id".into(),
    );
    check.array(
        &raw.adaptation.provenance_refs,
        1,
        "/adaptation/provenance_refs".into(),
    );
    for (i, id) in raw.adaptation.provenance_refs.iter().enumerate() {
        check.id(id, format!("/adaptation/provenance_refs/{i}"));
    }
    check.array(&raw.characters, 1, "/characters".into());
    for (i, character) in raw.characters.iter().enumerate() {
        check.id(&character.id, format!("/characters/{i}/id"));
        check.text(&character.name, format!("/characters/{i}/name"));
        check.text(
            &character.personality,
            format!("/characters/{i}/personality"),
        );
    }
    check.id(&raw.episode.id, "/episode/id".into());
    check.text(&raw.episode.title, "/episode/title".into());
    check.array(&raw.episode.acts, 1, "/episode/acts".into());
    for (a, act) in raw.episode.acts.iter().enumerate() {
        let ap = format!("/episode/acts/{a}");
        check.id(&act.id, format!("{ap}/id"));
        check.text(&act.title, format!("{ap}/title"));
        check.array(&act.scenes, 1, format!("{ap}/scenes"));
        for (s, scene) in act.scenes.iter().enumerate() {
            let sp = format!("{ap}/scenes/{s}");
            check.id(&scene.id, format!("{sp}/id"));
            check.text(&scene.title, format!("{sp}/title"));
            check.array(&scene.dialogues, 1, format!("{sp}/dialogues"));
            for (d, dialogue) in scene.dialogues.iter().enumerate() {
                let dp = format!("{sp}/dialogues/{d}");
                check.id(&dialogue.id, format!("{dp}/id"));
                check.id(&dialogue.speaker_id, format!("{dp}/speaker_id"));
                check.text(&dialogue.text, format!("{dp}/text"));
                if dialogue.delivery.intensity_permille > 1000 {
                    check.issue(
                        format!("{dp}/delivery/intensity_permille"),
                        ShapeRule::IntensityRange,
                    );
                }
                check.array(
                    &dialogue.pronunciation_overrides,
                    0,
                    format!("{dp}/pronunciation_overrides"),
                );
                for (p, pronunciation) in dialogue.pronunciation_overrides.iter().enumerate() {
                    check.text(
                        &pronunciation.surface,
                        format!("{dp}/pronunciation_overrides/{p}/surface"),
                    );
                    check.text(
                        &pronunciation.replacement,
                        format!("{dp}/pronunciation_overrides/{p}/replacement"),
                    );
                }
            }
            check.array(&scene.sound_cues, 0, format!("{sp}/sound_cues"));
            for (c, cue) in scene.sound_cues.iter().enumerate() {
                let cp = format!("{sp}/sound_cues/{c}");
                check.id(&cue.id, format!("{cp}/id"));
                check.text(&cue.description, format!("{cp}/description"));
                check.id(&cue.anchor.dialogue_id, format!("{cp}/anchor/dialogue_id"));
                if let Some(asset) = &cue.asset {
                    check.id(&asset.id, format!("{cp}/asset/id"));
                    check.id(
                        &asset.rights_record_id,
                        format!("{cp}/asset/rights_record_id"),
                    );
                }
            }
        }
    }
    check.array(&raw.provenance, 1, "/provenance".into());
    for (p, source) in raw.provenance.iter().enumerate() {
        let (id, record, rights, generation) = source.fields();
        check.id(id, format!("/provenance/{p}/id"));
        check.id(record, format!("/provenance/{p}/source_record_id"));
        check.id(rights, format!("/provenance/{p}/rights_record_id"));
        if let Some(generation) = generation {
            check.id(generation, format!("/provenance/{p}/generation_record_id"));
        }
    }
    check.0
}
