use serde::Deserialize;

/// Immutable validated content. No Deserialize, Default, setters or public fields.
#[derive(Debug, Clone)]
pub struct ScriptContent {
    pub(super) work: Work,
    pub(super) adaptation: Adaptation,
    pub(super) characters: Vec<Character>,
    pub(super) episode: Episode,
    pub(super) provenance: Vec<Source>,
}

#[derive(Debug, Clone)]
pub(super) struct Id(pub(super) String);

#[derive(Debug, Clone)]
pub(super) struct CharacterId(pub(super) String);

#[derive(Debug, Clone)]
pub(super) struct DialogueId(pub(super) String);

#[derive(Debug, Clone)]
pub(super) struct Text(pub(super) String);

#[derive(Debug, Clone)]
pub(super) struct Work {
    pub id: Id,
    pub title: Text,
    pub source_ref: Id,
    pub rights_record_id: Id,
}

#[derive(Debug, Clone)]
pub(super) struct Adaptation {
    pub id: Id,
    pub language: Language,
    pub rights_record_id: Id,
    pub provenance_refs: Vec<Id>,
}

#[derive(Debug, Clone)]
pub(super) struct Character {
    pub id: CharacterId,
    pub name: Text,
    pub role: Role,
    pub personality: Text,
}

#[derive(Debug, Clone)]
pub(super) struct Episode {
    pub id: Id,
    pub title: Text,
    pub acts: Vec<Act>,
}

#[derive(Debug, Clone)]
pub(super) struct Act {
    pub id: Id,
    pub title: Text,
    pub scenes: Vec<Scene>,
}

#[derive(Debug, Clone)]
pub(super) struct Scene {
    pub id: Id,
    pub title: Text,
    pub dialogues: Vec<Dialogue>,
    pub sound_cues: Vec<Cue>,
}

#[derive(Debug, Clone)]
pub(super) struct Dialogue {
    pub id: DialogueId,
    pub speaker: CharacterId,
    pub text: Text,
    pub delivery: Delivery,
    pub pronunciation: Vec<Pronunciation>,
}

#[derive(Debug, Clone)]
pub(super) struct Delivery {
    pub emotion: Emotion,
    pub intensity_permille: u16,
}

#[derive(Debug, Clone)]
pub(super) struct Pronunciation {
    pub surface: Text,
    pub replacement: Text,
}

#[derive(Debug, Clone)]
pub(super) struct Cue {
    pub id: Id,
    pub kind: CueKind,
    pub description: Text,
    pub dialogue: DialogueId,
    pub edge: Edge,
    pub asset: Option<Asset>,
}

#[derive(Debug, Clone)]
pub(super) struct Asset {
    pub id: Id,
    pub rights_record_id: Id,
}

#[derive(Debug, Clone)]
pub(super) struct Source {
    pub id: Id,
    pub kind: SourceKind,
    pub source_record_id: Id,
    pub rights_record_id: Id,
}

#[derive(Debug, Clone)]
pub(super) enum SourceKind {
    Original,
    Imported,
    Generated { generation_record_id: Id },
}

// Closed enums have no invariants beyond their spelling, so wire DTOs can reuse them.
macro_rules! wire_enum {
    ($name:ident { $($variant:ident => $spelling:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
        pub(super) enum $name { $(#[serde(rename = $spelling)] $variant),+ }
        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $spelling),+ }
            }
        }
    };
}

wire_enum!(Language { Vi => "vi", ViVn => "vi-VN", En => "en", EnUs => "en-US" });
wire_enum!(Role { Narrator => "narrator", Character => "character" });
wire_enum!(Emotion { Neutral => "neutral", Calm => "calm", Hopeful => "hopeful", Warm => "warm", Sad => "sad", Angry => "angry" });
wire_enum!(CueKind { Ambience => "ambience", Music => "music", Sfx => "sfx" });
wire_enum!(Edge { Start => "start", End => "end" });

/// The pipeline uses speaker to resolve separately revisioned casting.
/// Canonical speech bytes exclude this line's identity, position and speaker.
pub struct SpokenLine<'a> {
    pub(super) dialogue: &'a Dialogue,
    pub(super) language: Language,
}

impl SpokenLine<'_> {
    pub fn dialogue_id(&self) -> &str {
        &self.dialogue.id.0
    }
    pub fn speaker_id(&self) -> &str {
        &self.dialogue.speaker.0
    }
    pub fn text(&self) -> &str {
        &self.dialogue.text.0
    }
}

impl ScriptContent {
    pub fn spoken_lines(&self) -> impl Iterator<Item = SpokenLine<'_>> {
        self.episode
            .acts
            .iter()
            .flat_map(|act| &act.scenes)
            .flat_map(|scene| &scene.dialogues)
            .map(|dialogue| SpokenLine {
                dialogue,
                language: self.adaptation.language,
            })
    }

    /// External evidence references, resolved and authorized by the application shell.
    /// This pure projection does not establish rights eligibility.
    pub fn evidence_refs(&self) -> Vec<(&'static str, &str)> {
        let mut refs = vec![
            ("rights", self.work.rights_record_id.0.as_str()),
            ("rights", self.adaptation.rights_record_id.0.as_str()),
        ];
        for source in &self.provenance {
            refs.push(("source", source.source_record_id.0.as_str()));
            refs.push(("rights", source.rights_record_id.0.as_str()));
            if let SourceKind::Generated {
                generation_record_id,
            } = &source.kind
            {
                refs.push(("generation", generation_record_id.0.as_str()));
            }
        }
        for act in &self.episode.acts {
            for scene in &act.scenes {
                for cue in &scene.sound_cues {
                    if let Some(asset) = &cue.asset {
                        refs.push(("asset", asset.id.0.as_str()));
                        refs.push(("rights", asset.rights_record_id.0.as_str()));
                    }
                }
            }
        }
        refs.sort_unstable();
        refs.dedup();
        refs
    }
}

/// Only scheme c1 is constructible; a future scheme needs an explicit comparison API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDigest(pub(super) [u8; 32]);

impl std::fmt::Display for ContentDigest {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("sir-c1:sha256:")?;
        for byte in self.0 {
            write!(out, "{byte:02x}")?;
        }
        Ok(())
    }
}
