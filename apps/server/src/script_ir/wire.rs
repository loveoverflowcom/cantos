//! JSON shape DTOs and encoding: values cross into the domain only via validation.
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Dialogue, SCHEMA_VERSION, ScriptContent, SoundCue};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawScript {
    #[serde(rename = "schema_version")]
    _schema_version: String,
    pub work: RawWork,
    pub adaptation: RawAdaptation,
    pub characters: Vec<RawCharacter>,
    pub episode: RawEpisode,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawWork {
    pub id: String,
    pub title: String,
    pub source_ref: String,
    pub rights_ref: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawAdaptation {
    pub id: String,
    pub language: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCharacter {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub characterization: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawEpisode {
    pub id: String,
    pub title: String,
    pub acts: Vec<RawAct>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawAct {
    pub id: String,
    pub title: String,
    pub scenes: Vec<RawScene>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawScene {
    pub id: String,
    pub title: String,
    pub dialogues: Vec<RawDialogue>,
    #[serde(default)]
    pub sound_cues: Vec<RawCue>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawDialogue {
    pub id: String,
    pub speaker_id: String,
    pub text: String,
    pub delivery: Option<RawDelivery>,
    #[serde(default, deserialize_with = "read_pronunciation")]
    pub pronunciation: Vec<RawPronunciation>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawDelivery {
    pub emotion: Emotion,
    #[serde(deserialize_with = "read_integer")]
    pub intensity_permille: u16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPronunciation {
    pub surface: String,
    pub respelling: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCue {
    pub id: String,
    pub kind: CueKind,
    pub description: String,
    pub anchor: RawAnchor,
    pub asset_ref: Option<String>,
    pub rights_ref: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawAnchor {
    pub dialogue_id: String,
    pub edge: Edge,
}

macro_rules! wire_enum {
    ($name:ident { $($variant:ident => $spelling:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
        pub(super) enum $name { $(#[serde(rename = $spelling)] $variant),+ }
        impl $name {
            fn as_str(self) -> &'static str { match self { $(Self::$variant => $spelling),+ } }
        }
    };
}
wire_enum!(Role { Narrator => "narrator", Character => "character" });
wire_enum!(Emotion { Neutral => "neutral", Calm => "calm", Hopeful => "hopeful", Warm => "warm", Sad => "sad", Joyful => "joyful", Angry => "angry", Fearful => "fearful" });
wire_enum!(CueKind { Ambience => "ambience", Music => "music", Effect => "effect" });
wire_enum!(Edge { Start => "start", End => "end" });

pub(super) fn encode(content: &ScriptContent) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "work": {"id": content.work.id.0, "title": content.work.title.0, "source_ref": content.work.source_ref.0, "rights_ref": content.work.rights_ref.0},
        "adaptation": {"id": content.adaptation.id.0, "language": content.adaptation.language},
        "characters": content.characters.iter().map(|c| {
            let mut value = json!({"id":c.id.0,"name":c.name.0,"role":c.role.as_str()});
            if let Some(text) = &c.characterization { value["characterization"] = json!(text.0); }
            value
        }).collect::<Vec<_>>(),
        "episode": {
            "id":content.episode.id.0,"title":content.episode.title.0,
            "acts":content.episode.acts.iter().map(|act| json!({
                "id":act.id.0,"title":act.title.0,
                "scenes":act.scenes.iter().map(|scene| {
                    let mut value = json!({"id":scene.id.0,"title":scene.title.0,"dialogues":scene.dialogues.iter().map(dialogue).collect::<Vec<_>>()});
                    if !scene.sound_cues.is_empty() { value["sound_cues"] = json!(scene.sound_cues.iter().map(cue).collect::<Vec<_>>()); }
                    value
                }).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        }
    })
}

fn dialogue(d: &Dialogue) -> Value {
    let mut value = json!({"id":d.id.0,"speaker_id":d.speaker_id.0,"text":d.text.0});
    if let Some(delivery) = &d.delivery {
        value["delivery"] = json!({"emotion":delivery.emotion.as_str(),"intensity_permille":delivery.intensity_permille});
    }
    if !d.pronunciation.is_empty() {
        value["pronunciation"] = json!(
            d.pronunciation
                .iter()
                .map(|p| json!({"surface":p.surface.0,"respelling":p.respelling.0}))
                .collect::<Vec<_>>()
        );
    }
    value
}

fn cue(c: &SoundCue) -> Value {
    let mut value = json!({"id":c.id.0,"kind":c.kind.as_str(),"description":c.description.0,"anchor":{"dialogue_id":c.anchor.dialogue_id.0,"edge":c.anchor.edge.as_str()}});
    if let Some(asset) = &c.selected_asset {
        value["asset_ref"] = json!(asset.asset_ref.0);
        value["rights_ref"] = json!(asset.rights_ref.0);
    }
    value
}

/// Explicit JSON-subset encoder: sorts keys even if serde's `preserve_order`
/// feature is enabled elsewhere. The domain never admits non-integer numbers.
pub(super) fn canonical_bytes(value: &Value) -> Vec<u8> {
    let mut output = String::new();
    write_value(value, &mut output);
    output.into_bytes()
}

fn write_value(value: &Value, output: &mut String) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => write_string(value, output),
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                write_value(value, output);
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            let mut fields: Vec<_> = values.iter().collect();
            fields.sort_by_key(|(a, _)| *a);
            for (index, (key, value)) in fields.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                write_string(key, output);
                output.push(':');
                write_value(value, output);
            }
            output.push('}');
        }
    }
}

fn write_string(value: &str, output: &mut String) {
    output.push('"');
    for c in value.chars() {
        match c {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            c if c <= '\u{001f}' => {
                let number = c as u32;
                let hex = b"0123456789abcdef";
                output.push_str("\\u00");
                output.push(char::from(hex[(number >> 4) as usize]));
                output.push(char::from(hex[(number & 15) as usize]));
            }
            c => output.push(c),
        }
    }
    output.push('"');
}

// JSON Schema integer means a mathematical integer, including spellings such as
// 300.0 or 3e2. Normalize them here so the domain and canonical bytes stay integer.
fn read_integer<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u16, D::Error> {
    struct IntegerVisitor;
    impl serde::de::Visitor<'_> for IntegerVisitor {
        type Value = u16;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an integer in the unsigned 16-bit range")
        }
        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<u16, E> {
            u16::try_from(value).map_err(|_| E::custom("integer out of range"))
        }
        fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<u16, E> {
            u16::try_from(value).map_err(|_| E::custom("integer out of range"))
        }
        fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<u16, E> {
            if value.is_finite()
                && (0.0..=f64::from(u16::MAX)).contains(&value)
                && value.fract() == 0.0
            {
                Ok(value as u16)
            } else {
                Err(E::custom("expected an integer in range"))
            }
        }
    }
    deserializer.deserialize_any(IntegerVisitor)
}

fn read_pronunciation<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<RawPronunciation>, D::Error> {
    let values = Vec::<RawPronunciation>::deserialize(deserializer)?;
    if values.len() > 64 {
        return Err(serde::de::Error::custom(
            "at most 64 pronunciation overrides",
        ));
    }
    Ok(values)
}
