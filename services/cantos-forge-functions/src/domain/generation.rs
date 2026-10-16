//! What one generation request pins, and the fingerprint that makes a retry recognizable.
//!
//! A request names the exact input revision, instruction digest, model, voice, audio profile
//! and workflow version it will be produced with. Its fingerprint covers every one of them: the
//! same key with the same fingerprint is a retry of one decision, while the same key with a
//! different fingerprint is a different decision and conflicts. Policy that the operator
//! controls (budget, attempt limit, cost estimate) is deliberately not part of a request.

use std::fmt;

use super::{
    artifact::InputPin,
    audio_profile::AudioProfile,
    digest::Sha256Digest,
    label::{ModelLabel, VoiceLabel, WorkflowVersion},
};

/// Domain separation: no other digest in Forge starts with these bytes.
const FINGERPRINT_DOMAIN: &[u8] = b"cantos-forge.generation-request/v1";

/// The pinned inputs of one bounded generation. Source text or generated instructions cannot
/// widen it: only the fields below take part in the decision to spend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationRequest {
    /// The exact artifact revision and bytes the speech is produced from.
    pub input: InputPin,
    /// Digest of the instruction or style bundle in force.
    pub instruction: Sha256Digest,
    pub model: ModelLabel,
    pub voice: VoiceLabel,
    /// The Cantos intake profile the result must later be encoded to. Naming it here does not
    /// encode anything.
    pub profile: AudioProfile,
    pub workflow: WorkflowVersion,
}

/// SHA-256 over a request's canonical encoding. It identifies a decision, not a result: it
/// says nothing about what a provider returned.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestFingerprint(Sha256Digest);

impl RequestFingerprint {
    /// Restores a fingerprint read from a record. Only [`GenerationRequest::fingerprint`]
    /// computes one; the records the shell reads are checked by replaying them.
    pub fn from_digest(digest: Sha256Digest) -> Self {
        Self(digest)
    }

    pub fn digest(self) -> Sha256Digest {
        self.0
    }
}

impl fmt::Display for RequestFingerprint {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, out)
    }
}

impl fmt::Debug for RequestFingerprint {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "RequestFingerprint({})", self.0)
    }
}

impl GenerationRequest {
    /// Hashes `domain || (tag, 8-byte big-endian length, value)*` in a fixed field order.
    /// Length prefixes keep adjacent text fields from sliding into one another.
    pub fn fingerprint(&self) -> RequestFingerprint {
        let mut bytes = FINGERPRINT_DOMAIN.to_vec();
        field(&mut bytes, 1, self.input.artifact.to_string().as_bytes());
        field(&mut bytes, 2, &self.input.revision.get().to_be_bytes());
        field(&mut bytes, 3, self.input.digest.to_string().as_bytes());
        field(&mut bytes, 4, self.instruction.to_string().as_bytes());
        field(&mut bytes, 5, self.model.as_str().as_bytes());
        field(&mut bytes, 6, self.voice.as_str().as_bytes());
        field(&mut bytes, 7, self.profile.id().as_bytes());
        field(&mut bytes, 8, self.workflow.as_str().as_bytes());
        RequestFingerprint(Sha256Digest::of(&bytes))
    }
}

fn field(out: &mut Vec<u8>, tag: u8, value: &[u8]) {
    out.push(tag);
    out.extend_from_slice(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    out.extend_from_slice(value);
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use crate::domain::{artifact::RevisionNumber, identity::ArtifactId};

    /// A request with every field set, for tests that change one field at a time.
    pub fn request() -> GenerationRequest {
        GenerationRequest {
            input: InputPin {
                artifact: ArtifactId::from_bits(7),
                revision: RevisionNumber::new(3).unwrap(),
                digest: Sha256Digest::of(b"story"),
            },
            instruction: Sha256Digest::of(b"instructions"),
            model: ModelLabel::parse("gemini-test-tts").unwrap(),
            voice: VoiceLabel::parse("Kore").unwrap(),
            profile: AudioProfile::OpusMono24V1,
            workflow: WorkflowVersion::parse("forge-0.1.0").unwrap(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::request, *};
    use crate::domain::{artifact::RevisionNumber, identity::ArtifactId};

    // Computed once by an independent Python implementation of the layout documented on
    // `fingerprint`, not by this code.
    const GOLDEN: &str = "sha256:f3094dd967a0a5c0950db0248fac02b489664970e97ce36da4994c8fff6a86bb";

    #[test]
    fn the_fingerprint_matches_the_documented_layout() {
        assert_eq!(request().fingerprint().to_string(), GOLDEN);
    }

    #[test]
    fn the_fingerprint_is_deterministic_and_restores_from_its_digest() {
        let fingerprint = request().fingerprint();
        assert_eq!(fingerprint, request().fingerprint());
        assert_eq!(
            RequestFingerprint::from_digest(fingerprint.digest()),
            fingerprint
        );
    }

    #[test]
    fn changing_any_single_pinned_input_changes_the_fingerprint() {
        let base = request();
        let changed: Vec<(&str, GenerationRequest)> = vec![
            (
                "input artifact",
                GenerationRequest {
                    input: InputPin {
                        artifact: ArtifactId::from_bits(8),
                        ..base.input
                    },
                    ..base.clone()
                },
            ),
            (
                "input revision",
                GenerationRequest {
                    input: InputPin {
                        revision: RevisionNumber::new(4).unwrap(),
                        ..base.input
                    },
                    ..base.clone()
                },
            ),
            (
                "input digest",
                GenerationRequest {
                    input: InputPin {
                        digest: Sha256Digest::of(b"another story"),
                        ..base.input
                    },
                    ..base.clone()
                },
            ),
            (
                "instruction",
                GenerationRequest {
                    instruction: Sha256Digest::of(b"other instructions"),
                    ..base.clone()
                },
            ),
            (
                "model",
                GenerationRequest {
                    model: ModelLabel::parse("gemini-other-tts").unwrap(),
                    ..base.clone()
                },
            ),
            (
                "voice",
                GenerationRequest {
                    voice: VoiceLabel::parse("Puck").unwrap(),
                    ..base.clone()
                },
            ),
            (
                "profile",
                GenerationRequest {
                    profile: AudioProfile::OpusMono32V1,
                    ..base.clone()
                },
            ),
            (
                "workflow",
                GenerationRequest {
                    workflow: WorkflowVersion::parse("forge-0.2.0").unwrap(),
                    ..base.clone()
                },
            ),
        ];
        for (name, request) in changed {
            assert_ne!(request.fingerprint(), base.fingerprint(), "{name}");
        }
    }

    #[test]
    fn a_fingerprint_is_written_as_its_digest() {
        let fingerprint = request().fingerprint();
        assert_eq!(fingerprint.to_string(), GOLDEN);
        assert_eq!(
            format!("{fingerprint:?}"),
            format!("RequestFingerprint({GOLDEN})")
        );
    }

    #[test]
    fn adjacent_text_fields_cannot_slide_into_each_other() {
        let split = |model: &str, voice: &str| {
            GenerationRequest {
                model: ModelLabel::parse(model).unwrap(),
                voice: VoiceLabel::parse(voice).unwrap(),
                ..request()
            }
            .fingerprint()
        };
        assert_ne!(split("ab", "c"), split("a", "bc"));
        assert_ne!(split("tts", "Kore"), split("ttsK", "ore"));
    }

    #[test]
    fn vietnamese_labels_hash_their_exact_bytes() {
        let voice = |text: &str| {
            GenerationRequest {
                voice: VoiceLabel::parse(text).unwrap(),
                ..request()
            }
            .fingerprint()
        };
        // NFC and NFD spellings of the same word are different decisions.
        assert_ne!(
            voice("H\u{e0} N\u{1ed9}i"),
            voice("Ha\u{300} No\u{323}\u{302}i")
        );
    }
}
