use sha2::{Digest, Sha256};

use super::model::{
    Act, Character, ContentDigest, Cue, Delivery, Dialogue, Episode, Pronunciation, Scene,
    ScriptContent, Source, SourceKind, SpokenLine,
};
use super::wire::WRITE_VERSION;

// Fixed ASCII keys are written in sorted order. Validated text contains no controls.
// This deliberately has no general map, float, arbitrary-value or reflection interface.
struct Writer(String);

struct Object<'a> {
    writer: &'a mut Writer,
    first: bool,
}

impl Writer {
    fn string(&mut self, text: &str) {
        self.0.push('"');
        for ch in text.chars() {
            if matches!(ch, '"' | '\\') {
                self.0.push('\\');
            }
            self.0.push(ch);
        }
        self.0.push('"');
    }

    fn object(&mut self, write: impl FnOnce(&mut Object<'_>)) {
        self.0.push('{');
        write(&mut Object {
            writer: self,
            first: true,
        });
        self.0.push('}');
    }

    fn array<'a, T: 'a>(
        &mut self,
        items: impl IntoIterator<Item = &'a T>,
        mut write: impl FnMut(&mut Writer, &T),
    ) {
        self.0.push('[');
        let mut first = true;
        for item in items {
            if !first {
                self.0.push(',');
            }
            first = false;
            write(self, item);
        }
        self.0.push(']');
    }
}

impl Object<'_> {
    fn field(&mut self, name: &str, write: impl FnOnce(&mut Writer)) {
        if !self.first {
            self.writer.0.push(',');
        }
        self.first = false;
        self.writer.string(name);
        self.writer.0.push(':');
        write(self.writer);
    }

    fn string(&mut self, name: &str, value: &str) {
        self.field(name, |w| w.string(value));
    }
}

impl ScriptContent {
    /// Canonical performed/displayed content; metadata is deliberately excluded.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.encode(false)
    }

    /// Canonical versioned interchange, including immutable provenance/rights references.
    /// Unlike content bytes, this is a complete document readable by read_script.
    pub fn export_bytes(&self) -> Vec<u8> {
        self.encode(true)
    }

    pub fn content_digest(&self) -> ContentDigest {
        let mut hash = Sha256::new();
        hash.update(b"cantos/script-content/c1\n");
        hash.update(self.canonical_bytes());
        ContentDigest(hash.finalize().into())
    }

    fn encode(&self, metadata: bool) -> Vec<u8> {
        let mut writer = Writer(String::new());
        writer.object(|o| {
            o.field("adaptation", |w| {
                w.object(|a| {
                    a.string("id", &self.adaptation.id.0);
                    a.string("language", self.adaptation.language.as_str());
                    if metadata {
                        a.field("provenance_refs", |w| {
                            w.array(&self.adaptation.provenance_refs, |w, id| w.string(&id.0))
                        });
                        a.string("rights_record_id", &self.adaptation.rights_record_id.0);
                    }
                })
            });
            o.field("characters", |w| w.array(&self.characters, character));
            o.field("episode", |w| episode(w, &self.episode, metadata));
            if metadata {
                o.field("provenance", |w| w.array(&self.provenance, source));
                o.string("schema_version", WRITE_VERSION);
            }
            o.field("work", |w| {
                w.object(|o| {
                    o.string("id", &self.work.id.0);
                    if metadata {
                        o.string("rights_record_id", &self.work.rights_record_id.0);
                        o.string("source_ref", &self.work.source_ref.0);
                    }
                    o.string("title", &self.work.title.0);
                })
            });
        });
        writer.0.into_bytes()
    }
}

impl SpokenLine<'_> {
    /// Effective script-side speech inputs. This alone is not a production cache key.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut writer = Writer(String::new());
        writer.object(|o| {
            o.field("delivery", |w| delivery(w, &self.dialogue.delivery));
            o.string("language", self.language.as_str());
            if !self.dialogue.pronunciation.is_empty() {
                o.field("pronunciation_overrides", |w| {
                    w.array(&self.dialogue.pronunciation, pronunciation)
                });
            }
            o.string("text", &self.dialogue.text.0);
        });
        writer.0.into_bytes()
    }
}

fn character(w: &mut Writer, c: &Character) {
    w.object(|o| {
        o.string("id", &c.id.0);
        o.string("name", &c.name.0);
        o.string("personality", &c.personality.0);
        o.string("role", c.role.as_str());
    });
}

fn episode(w: &mut Writer, episode: &Episode, metadata: bool) {
    w.object(|o| {
        o.field("acts", |w| {
            w.array(&episode.acts, |w, a| act(w, a, metadata))
        });
        o.string("id", &episode.id.0);
        o.string("title", &episode.title.0);
    });
}

fn act(w: &mut Writer, act: &Act, metadata: bool) {
    w.object(|o| {
        o.string("id", &act.id.0);
        o.field("scenes", |w| {
            w.array(&act.scenes, |w, s| scene(w, s, metadata))
        });
        o.string("title", &act.title.0);
    });
}

fn scene(w: &mut Writer, scene: &Scene, metadata: bool) {
    w.object(|o| {
        o.field("dialogues", |w| w.array(&scene.dialogues, dialogue));
        o.string("id", &scene.id.0);
        if !scene.sound_cues.is_empty() {
            o.field("sound_cues", |w| {
                w.array(&scene.sound_cues, |w, c| cue(w, c, metadata))
            });
        }
        o.string("title", &scene.title.0);
    });
}

fn dialogue(w: &mut Writer, d: &Dialogue) {
    w.object(|o| {
        o.field("delivery", |w| delivery(w, &d.delivery));
        o.string("id", &d.id.0);
        if !d.pronunciation.is_empty() {
            o.field("pronunciation_overrides", |w| {
                w.array(&d.pronunciation, pronunciation)
            });
        }
        o.string("speaker_id", &d.speaker.0);
        o.string("text", &d.text.0);
    });
}

fn delivery(w: &mut Writer, delivery: &Delivery) {
    w.object(|o| {
        o.string("emotion", delivery.emotion.as_str());
        o.field("intensity_permille", |w| {
            w.0.push_str(&delivery.intensity_permille.to_string())
        });
    });
}

fn pronunciation(w: &mut Writer, pronunciation: &Pronunciation) {
    w.object(|o| {
        o.string("replacement", &pronunciation.replacement.0);
        o.string("surface", &pronunciation.surface.0);
    });
}

fn cue(w: &mut Writer, cue: &Cue, metadata: bool) {
    w.object(|o| {
        o.field("anchor", |w| {
            w.object(|o| {
                o.string("dialogue_id", &cue.dialogue.0);
                o.string("edge", cue.edge.as_str());
            })
        });
        if let Some(asset) = &cue.asset {
            o.field("asset", |w| {
                w.object(|o| {
                    o.string("id", &asset.id.0);
                    if metadata {
                        o.string("rights_record_id", &asset.rights_record_id.0);
                    }
                })
            });
        }
        o.string("description", &cue.description.0);
        o.string("id", &cue.id.0);
        o.string("kind", cue.kind.as_str());
    });
}

fn source(w: &mut Writer, source: &Source) {
    w.object(|o| {
        if let SourceKind::Generated {
            generation_record_id,
        } = &source.kind
        {
            o.string("generation_record_id", &generation_record_id.0);
        }
        o.string("id", &source.id.0);
        o.string(
            "kind",
            match source.kind {
                SourceKind::Original => "original",
                SourceKind::Imported => "imported",
                SourceKind::Generated { .. } => "generated",
            },
        );
        o.string("rights_record_id", &source.rights_record_id.0);
        o.string("source_record_id", &source.source_record_id.0);
    });
}
