//! Pure decision: may an already registered, verified clip stand in for a new synthesis step?
//!
//! A matching fingerprint is necessary and nowhere near sufficient. Reuse also needs the same
//! creator scope, the same voice-rights record, the same kind of producer (a synthetic fixture
//! never satisfies a real engine), the same output contract, and fresh evidence from the stored
//! object: it exists, it is safe to open, it decodes under the contracted profile and its
//! checksum still equals the registered one. A completed job is none of these facts.
//!
//! The shell gathers the facts (rows, rights standing, a probe of the object) and calls
//! [`resolve_reuse`]; nothing here reads a database, a file or a clock.

use std::{error::Error, fmt, str::FromStr};

use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::contexts::synthesis::{fingerprint::SpeechFingerprint, jobs::ExecutionMode};

/// Lowercase 64-character hexadecimal SHA-256 of an audio file's bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AudioDigest(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidAudioDigest;

impl fmt::Display for InvalidAudioDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("audio digest must be 64 lowercase hexadecimal characters")
    }
}

impl Error for InvalidAudioDigest {}

impl AudioDigest {
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for AudioDigest {
    type Err = InvalidAudioDigest;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(InvalidAudioDigest);
        }
        Ok(Self(value.to_owned()))
    }
}

impl fmt::Display for AudioDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for AudioDigest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// A registry row: what was audited when the clip was registered. Immutable once written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredClip {
    pub id: String,
    /// Registration order. Selection is oldest-first so a later duplicate never flips a choice.
    pub sequence: i64,
    pub fingerprint: SpeechFingerprint,
    pub owner_id: String,
    pub producer: ExecutionMode,
    pub voice_rights_record_id: String,
    pub output_contract: String,
    pub audio_sha256: AudioDigest,
}

/// A registry row with the locator of its object. The locator is shell data; the decision
/// below never reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredClip {
    pub clip: RegisteredClip,
    pub attempt_id: String,
    pub output_path: String,
}

/// What the shell observed about the stored object, outside any transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactProbe {
    /// No storage access was available, so nothing was verified. Never a hit.
    NotProbed,
    /// Absent or unreadable. May be transient; it is a miss now, not a verdict on the clip.
    Missing,
    /// Present but unsafe to open (link, device, oversize) or not valid audio for the contract.
    Invalid,
    /// Opened safely, decoded under the requested output contract, hashed.
    Present { audio_sha256: AudioDigest },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipCandidate {
    pub clip: RegisteredClip,
    pub probe: ArtifactProbe,
}

/// The step asking for audio. All fields come from the frozen, currently authorized input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipRequest {
    pub fingerprint: SpeechFingerprint,
    pub owner_id: String,
    pub producer: ExecutionMode,
    pub voice_rights_record_id: String,
    pub output_contract: String,
}

/// Rights and approval are facts about the requester's current authority, never part of a key.
/// A lapse blocks; regenerating under the same lapsed rights would be no more permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightsStanding {
    Current,
    Lapsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    FingerprintMismatch,
    OwnerScope,
    VoiceRightsRecord,
    ProducerMismatch,
    OutputContractMismatch,
    Unverified,
    Unavailable,
    ArtifactInvalid,
    ChecksumMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClipRefusal {
    pub clip_id: String,
    pub reason: Refusal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReuseDecision {
    Reuse {
        clip_id: String,
        audio_sha256: AudioDigest,
    },
    /// A cache miss. An empty list means no registered clip matched the fingerprint at all.
    Produce { refusals: Vec<ClipRefusal> },
    /// The requester may not use any clip, and may not produce a new one either.
    Blocked,
}

fn refusal(request: &ClipRequest, candidate: &ClipCandidate) -> Option<Refusal> {
    let clip = &candidate.clip;
    if clip.fingerprint != request.fingerprint {
        return Some(Refusal::FingerprintMismatch);
    }
    if clip.owner_id != request.owner_id {
        return Some(Refusal::OwnerScope);
    }
    if clip.voice_rights_record_id != request.voice_rights_record_id {
        return Some(Refusal::VoiceRightsRecord);
    }
    if clip.producer != request.producer {
        return Some(Refusal::ProducerMismatch);
    }
    if clip.output_contract != request.output_contract {
        return Some(Refusal::OutputContractMismatch);
    }
    match &candidate.probe {
        ArtifactProbe::NotProbed => Some(Refusal::Unverified),
        ArtifactProbe::Missing => Some(Refusal::Unavailable),
        ArtifactProbe::Invalid => Some(Refusal::ArtifactInvalid),
        ArtifactProbe::Present { audio_sha256 } if audio_sha256 != &clip.audio_sha256 => {
            Some(Refusal::ChecksumMismatch)
        }
        ArtifactProbe::Present { .. } => None,
    }
}

/// Oldest valid clip wins; candidate order supplied by the caller never matters.
pub fn resolve_reuse(
    request: &ClipRequest,
    rights: RightsStanding,
    candidates: &[ClipCandidate],
) -> ReuseDecision {
    if rights == RightsStanding::Lapsed {
        return ReuseDecision::Blocked;
    }
    let mut ordered: Vec<&ClipCandidate> = candidates.iter().collect();
    ordered.sort_by(|left, right| {
        (left.clip.sequence, &left.clip.id).cmp(&(right.clip.sequence, &right.clip.id))
    });
    let mut refusals = Vec::new();
    for candidate in ordered {
        match refusal(request, candidate) {
            None => {
                return ReuseDecision::Reuse {
                    clip_id: candidate.clip.id.clone(),
                    audio_sha256: candidate.clip.audio_sha256.clone(),
                }
            }
            Some(reason) => refusals.push(ClipRefusal {
                clip_id: candidate.clip.id.clone(),
                reason,
            }),
        }
    }
    ReuseDecision::Produce { refusals }
}

#[cfg(test)]
mod tests {
    use cantos_contracts::{ResolvedSynthesisInput, SynthesisPerformance};

    use super::*;
    use crate::contexts::production::catalog::local as local_contract;
    use crate::contexts::synthesis::fingerprint::SpeechInput;

    type Spoil = fn(&mut RegisteredClip);

    fn fingerprint(text: &str) -> SpeechFingerprint {
        SpeechInput::from_resolved(&ResolvedSynthesisInput {
            dialogue_id: "dialogue-01".into(),
            character_id: "an".into(),
            text: text.into(),
            language: "vi".into(),
            provider_id: local_contract::PROVIDER_ID.into(),
            model_id: local_contract::MODEL_ID.into(),
            voice_id: "vi".into(),
            voice_rights_record_id: "voice-espeak".into(),
            adapter_version: local_contract::ADAPTER_VERSION.into(),
            output_contract: local_contract::OUTPUT_CONTRACT.into(),
            performance: SynthesisPerformance {
                rate_permille: 1_000,
                pitch_semitones: 0,
                emotion: Some("neutral".into()),
                intensity_permille: Some(0),
            },
            pronunciation: Vec::new(),
        })
        .unwrap()
        .fingerprint()
    }

    fn digest(seed: u8) -> AudioDigest {
        AudioDigest::of_bytes(&[seed])
    }

    fn request() -> ClipRequest {
        ClipRequest {
            fingerprint: fingerprint("Xin chào."),
            owner_id: "alice".into(),
            producer: ExecutionMode::LocalEspeak,
            voice_rights_record_id: "voice-espeak".into(),
            output_contract: local_contract::OUTPUT_CONTRACT.into(),
        }
    }

    fn clip(id: &str, sequence: i64, seed: u8) -> RegisteredClip {
        let request = request();
        RegisteredClip {
            id: id.into(),
            sequence,
            fingerprint: request.fingerprint,
            owner_id: request.owner_id,
            producer: request.producer,
            voice_rights_record_id: request.voice_rights_record_id,
            output_contract: request.output_contract,
            audio_sha256: digest(seed),
        }
    }

    fn present(clip: RegisteredClip) -> ClipCandidate {
        let probe = ArtifactProbe::Present {
            audio_sha256: clip.audio_sha256.clone(),
        };
        ClipCandidate { clip, probe }
    }

    fn refused(reason: Refusal, id: &str) -> ReuseDecision {
        ReuseDecision::Produce {
            refusals: vec![ClipRefusal {
                clip_id: id.into(),
                reason,
            }],
        }
    }

    #[test]
    fn a_fully_verified_clip_in_scope_is_reused() {
        assert_eq!(
            resolve_reuse(
                &request(),
                RightsStanding::Current,
                &[present(clip("a", 1, 1))]
            ),
            ReuseDecision::Reuse {
                clip_id: "a".into(),
                audio_sha256: digest(1)
            }
        );
    }

    #[test]
    fn no_registered_clip_is_a_plain_miss_not_a_refusal() {
        assert_eq!(
            resolve_reuse(&request(), RightsStanding::Current, &[]),
            ReuseDecision::Produce {
                refusals: Vec::new()
            }
        );
    }

    #[test]
    fn each_scope_and_compatibility_mismatch_is_refused_with_its_own_reason() {
        let cases: [(Refusal, Spoil); 5] = [
            (Refusal::FingerprintMismatch, |c| {
                c.fingerprint = fingerprint("Khác.")
            }),
            (Refusal::OwnerScope, |c| c.owner_id = "mallory".into()),
            (Refusal::VoiceRightsRecord, |c| {
                c.voice_rights_record_id = "voice-other".into()
            }),
            (Refusal::ProducerMismatch, |c| {
                c.producer = ExecutionMode::SyntheticFake
            }),
            (Refusal::OutputContractMismatch, |c| {
                c.output_contract = "wav-pcm-s16le-mono-24000-v1".into()
            }),
        ];
        for (reason, mutate) in cases {
            let mut other = clip("a", 1, 1);
            mutate(&mut other);
            assert_eq!(
                resolve_reuse(&request(), RightsStanding::Current, &[present(other)]),
                refused(reason, "a")
            );
        }
    }

    #[test]
    fn a_synthetic_fixture_never_satisfies_a_real_engine_run_and_the_reverse() {
        for (clip_producer, requester) in [
            (ExecutionMode::SyntheticFake, ExecutionMode::LocalEspeak),
            (ExecutionMode::LocalEspeak, ExecutionMode::SyntheticFake),
        ] {
            let mut stored = clip("a", 1, 1);
            stored.producer = clip_producer;
            let mut asked = request();
            asked.producer = requester;
            assert_eq!(
                resolve_reuse(&asked, RightsStanding::Current, &[present(stored)]),
                refused(Refusal::ProducerMismatch, "a")
            );
        }
    }

    #[test]
    fn every_unverified_object_state_is_a_miss() {
        let stored = clip("a", 1, 1);
        let cases = [
            (ArtifactProbe::NotProbed, Refusal::Unverified),
            (ArtifactProbe::Missing, Refusal::Unavailable),
            (ArtifactProbe::Invalid, Refusal::ArtifactInvalid),
            (
                ArtifactProbe::Present {
                    audio_sha256: digest(9),
                },
                Refusal::ChecksumMismatch,
            ),
        ];
        for (probe, reason) in cases {
            let candidate = ClipCandidate {
                clip: stored.clone(),
                probe,
            };
            assert_eq!(
                resolve_reuse(&request(), RightsStanding::Current, &[candidate]),
                refused(reason, "a")
            );
        }
    }

    #[test]
    fn lapsed_rights_block_even_a_perfect_clip_and_an_empty_registry() {
        assert_eq!(
            resolve_reuse(
                &request(),
                RightsStanding::Lapsed,
                &[present(clip("a", 1, 1))]
            ),
            ReuseDecision::Blocked
        );
        assert_eq!(
            resolve_reuse(&request(), RightsStanding::Lapsed, &[]),
            ReuseDecision::Blocked
        );
    }

    #[test]
    fn the_oldest_valid_clip_wins_whatever_order_candidates_arrive_in() {
        let corrupt_oldest = ClipCandidate {
            clip: clip("c0", 1, 1),
            probe: ArtifactProbe::Present {
                audio_sha256: digest(200),
            },
        };
        let valid_older = present(clip("c1", 2, 2));
        let valid_newer = present(clip("c2", 3, 3));
        let all = [corrupt_oldest, valid_older, valid_newer];
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for order in permutations {
            let shuffled: Vec<_> = order.iter().map(|index| all[*index].clone()).collect();
            assert_eq!(
                resolve_reuse(&request(), RightsStanding::Current, &shuffled),
                ReuseDecision::Reuse {
                    clip_id: "c1".into(),
                    audio_sha256: digest(2)
                },
                "{order:?}"
            );
        }
    }

    #[test]
    fn refusals_list_every_candidate_in_registration_order() {
        let mut first = clip("b", 1, 1);
        first.owner_id = "mallory".into();
        let second = ClipCandidate {
            clip: clip("a", 2, 2),
            probe: ArtifactProbe::Missing,
        };
        assert_eq!(
            resolve_reuse(
                &request(),
                RightsStanding::Current,
                &[second, present(first)]
            ),
            ReuseDecision::Produce {
                refusals: vec![
                    ClipRefusal {
                        clip_id: "b".into(),
                        reason: Refusal::OwnerScope
                    },
                    ClipRefusal {
                        clip_id: "a".into(),
                        reason: Refusal::Unavailable
                    },
                ]
            }
        );
    }

    #[test]
    fn digest_parses_only_lowercase_sha256_hex() {
        let good = digest(1);
        assert_eq!(good.as_str().parse(), Ok(good.clone()));
        for bad in [
            "",
            &good.as_str().to_uppercase(),
            &good.as_str()[1..],
            &format!("{}0", good.as_str()),
            "g".repeat(64).as_str(),
        ] {
            assert_eq!(bad.parse::<AudioDigest>(), Err(InvalidAudioDigest), "{bad}");
        }
    }
}
