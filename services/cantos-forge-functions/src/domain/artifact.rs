//! Immutable artifact revisions and their pinned inputs.
//!
//! An artifact is anything a producer creates or uses through Forge: an original or collected
//! manuscript, a Script IR document, audio, a report or an exported instruction bundle. Every
//! change is a new revision; a revision is never edited. Nothing here implies that an artifact
//! was evaluated, handed off, admitted by the Cantos API or published.

use std::{collections::BTreeMap, num::NonZeroU32};

use super::{
    digest::Sha256Digest,
    identity::ArtifactId,
    label::{ClaimReference, ProducerLabel},
};

/// Revision numbers of one artifact start at 1 and increase by exactly 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RevisionNumber(NonZeroU32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevisionNumberError {
    Zero,
}

impl RevisionNumber {
    pub const FIRST: Self = Self(NonZeroU32::MIN);

    pub fn new(value: u32) -> Result<Self, RevisionNumberError> {
        NonZeroU32::new(value)
            .map(Self)
            .ok_or(RevisionNumberError::Zero)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// Where manuscript text came from. AI- or human-authored original writing is never recorded
/// as a collected source, and a collected source never becomes original work by revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManuscriptOrigin {
    Original,
    Collected,
}

/// Type-specific facts. A variant gains fields only when a consumer needs them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtifactPayload {
    Manuscript {
        origin: ManuscriptOrigin,
    },
    /// Validation of the document belongs to the shared Script IR validator, not to Forge.
    ScriptIr,
    /// Audio may stand alone (a recording without any script) or derive from text.
    Audio,
    Report,
    /// An exported, self-contained instruction bundle that later revisions can pin.
    InstructionBundle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactKind {
    Manuscript,
    ScriptIr,
    Audio,
    Report,
    InstructionBundle,
}

impl ArtifactPayload {
    pub fn kind(&self) -> ArtifactKind {
        match self {
            Self::Manuscript { .. } => ArtifactKind::Manuscript,
            Self::ScriptIr => ArtifactKind::ScriptIr,
            Self::Audio => ArtifactKind::Audio,
            Self::Report => ArtifactKind::Report,
            Self::InstructionBundle => ArtifactKind::InstructionBundle,
        }
    }
}

/// How the bytes came to exist. Provider-managed production is not implemented yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// The AI host or a person created the bytes and registered them through Forge.
    HostAssisted { producer: ProducerLabel },
}

/// The producer's rights claim. A claim is never verified rights or a publication right.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RightsClaim {
    /// Explicitly not stated yet; a later handoff must refuse this.
    Unstated,
    OriginalWork,
    Licensed {
        reference: ClaimReference,
    },
    PermittedSource {
        reference: ClaimReference,
    },
}

/// The exact revision and bytes of another artifact that influenced a revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputPin {
    pub artifact: ArtifactId,
    pub revision: RevisionNumber,
    pub digest: Sha256Digest,
}

/// At most one pin per artifact, iterated in artifact order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputPins(BTreeMap<ArtifactId, (RevisionNumber, Sha256Digest)>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputPinsError {
    /// Two pins name the same artifact, so the lineage would be ambiguous.
    RepeatedArtifact { artifact: ArtifactId },
}

impl InputPins {
    pub fn new(pins: impl IntoIterator<Item = InputPin>) -> Result<Self, InputPinsError> {
        let mut by_artifact = BTreeMap::new();
        for pin in pins {
            if by_artifact
                .insert(pin.artifact, (pin.revision, pin.digest))
                .is_some()
            {
                return Err(InputPinsError::RepeatedArtifact {
                    artifact: pin.artifact,
                });
            }
        }
        Ok(Self(by_artifact))
    }

    pub fn iter(&self) -> impl Iterator<Item = InputPin> + '_ {
        self.0
            .iter()
            .map(|(&artifact, &(revision, digest))| InputPin {
                artifact,
                revision,
                digest,
            })
    }

    pub fn contains_artifact(&self, artifact: ArtifactId) -> bool {
        self.0.contains_key(&artifact)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Everything a registration declares about one revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevisionFacts {
    pub content: Sha256Digest,
    pub payload: ArtifactPayload,
    pub provenance: Provenance,
    pub rights: RightsClaim,
    pub inputs: InputPins,
}

/// A settled revision. Fields are private and there is no setter: a correction is a successor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactRevision {
    artifact: ArtifactId,
    revision: RevisionNumber,
    facts: RevisionFacts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevisionError {
    /// A revision may not pin its own artifact; its predecessor is implied by its number.
    SelfInput,
    KindChanged {
        from: ArtifactKind,
        to: ArtifactKind,
    },
    /// The manuscript origin is part of its identity, not an editable detail.
    ManuscriptOriginChanged,
    Exhausted,
}

/// The idempotent outcome of registering facts for an artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Registration {
    Created(ArtifactRevision),
    /// The same facts are already the head revision; a replayed command creates nothing.
    AlreadyCurrent(RevisionNumber),
}

/// Decides what registering `facts` for `artifact` means, given its current head (if any).
///
/// The caller supplies the head it read; persisting `Created` must still be conditional on
/// that head being unchanged.
pub fn decide_registration(
    artifact: ArtifactId,
    head: Option<&ArtifactRevision>,
    facts: RevisionFacts,
) -> Result<Registration, RevisionError> {
    match head {
        None => ArtifactRevision::first(artifact, facts).map(Registration::Created),
        Some(head) if head.facts == facts => Ok(Registration::AlreadyCurrent(head.revision)),
        Some(head) => head.successor(facts).map(Registration::Created),
    }
}

/// A recorded history that the registration rules would not have produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// The recorded facts at this position (0-based) break a revision rule.
    Refused {
        position: usize,
        error: RevisionError,
    },
    /// The recorded facts at this position repeat the previous revision; a replayed command
    /// creates nothing, so such a record cannot have been written by registration.
    RepeatsPrevious { position: usize },
}

/// Rebuilds an artifact's revisions from recorded facts by applying the same registration rule
/// that created them, so restored history can never hold a revision registration refuses.
/// `recorded` is in revision order, starting with revision 1.
pub fn replay(
    artifact: ArtifactId,
    recorded: impl IntoIterator<Item = RevisionFacts>,
) -> Result<Vec<ArtifactRevision>, ReplayError> {
    let mut revisions: Vec<ArtifactRevision> = Vec::new();
    for (position, facts) in recorded.into_iter().enumerate() {
        match decide_registration(artifact, revisions.last(), facts) {
            Ok(Registration::Created(revision)) => revisions.push(revision),
            Ok(Registration::AlreadyCurrent(_)) => {
                return Err(ReplayError::RepeatsPrevious { position })
            }
            Err(error) => return Err(ReplayError::Refused { position, error }),
        }
    }
    Ok(revisions)
}

impl ArtifactRevision {
    fn first(artifact: ArtifactId, facts: RevisionFacts) -> Result<Self, RevisionError> {
        refuse_self_input(artifact, &facts)?;
        Ok(Self {
            artifact,
            revision: RevisionNumber::FIRST,
            facts,
        })
    }

    fn successor(&self, facts: RevisionFacts) -> Result<Self, RevisionError> {
        refuse_self_input(self.artifact, &facts)?;
        let (from, to) = (self.facts.payload.kind(), facts.payload.kind());
        if from != to {
            return Err(RevisionError::KindChanged { from, to });
        }
        if let (
            ArtifactPayload::Manuscript { origin: before },
            ArtifactPayload::Manuscript { origin: after },
        ) = (&self.facts.payload, &facts.payload)
        {
            if before != after {
                return Err(RevisionError::ManuscriptOriginChanged);
            }
        }
        let revision = self.revision.next().ok_or(RevisionError::Exhausted)?;
        Ok(Self {
            artifact: self.artifact,
            revision,
            facts,
        })
    }

    pub fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub fn revision(&self) -> RevisionNumber {
        self.revision
    }

    pub fn facts(&self) -> &RevisionFacts {
        &self.facts
    }

    /// The pin another revision or evidence record uses to depend on exactly this one.
    pub fn pin(&self) -> InputPin {
        InputPin {
            artifact: self.artifact,
            revision: self.revision,
            digest: self.facts.content,
        }
    }
}

fn refuse_self_input(artifact: ArtifactId, facts: &RevisionFacts) -> Result<(), RevisionError> {
    if facts.inputs.contains_artifact(artifact) {
        return Err(RevisionError::SelfInput);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    pub fn producer() -> Provenance {
        Provenance::HostAssisted {
            producer: ProducerLabel::parse("AI host session").unwrap(),
        }
    }

    pub fn facts(bytes: &[u8], payload: ArtifactPayload, inputs: InputPins) -> RevisionFacts {
        RevisionFacts {
            content: Sha256Digest::of(bytes),
            payload,
            provenance: producer(),
            rights: RightsClaim::OriginalWork,
            inputs,
        }
    }

    pub fn register(
        artifact: ArtifactId,
        head: Option<&ArtifactRevision>,
        facts: RevisionFacts,
    ) -> ArtifactRevision {
        match decide_registration(artifact, head, facts) {
            Ok(Registration::Created(revision)) => revision,
            other => panic!("expected a new revision, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::*, *};

    const STORY: ArtifactPayload = ArtifactPayload::Manuscript {
        origin: ManuscriptOrigin::Original,
    };

    fn id(bits: u128) -> ArtifactId {
        ArtifactId::from_bits(bits)
    }

    #[test]
    fn revision_numbers_start_at_one() {
        assert_eq!(RevisionNumber::new(0), Err(RevisionNumberError::Zero));
        assert_eq!(RevisionNumber::new(1), Ok(RevisionNumber::FIRST));
        assert_eq!(RevisionNumber::new(u32::MAX).unwrap().get(), u32::MAX);
    }

    #[test]
    fn first_registration_creates_revision_one_with_exact_facts() {
        let facts = facts(b"Mot loi hen", STORY, InputPins::default());
        let revision = register(id(1), None, facts.clone());
        assert_eq!(revision.artifact(), id(1));
        assert_eq!(revision.revision(), RevisionNumber::FIRST);
        assert_eq!(revision.facts(), &facts);
        assert_eq!(revision.pin().digest, Sha256Digest::of(b"Mot loi hen"));
    }

    #[test]
    fn replaying_identical_facts_creates_nothing() {
        let facts = facts(b"draft", STORY, InputPins::default());
        let head = register(id(1), None, facts.clone());
        assert_eq!(
            decide_registration(id(1), Some(&head), facts),
            Ok(Registration::AlreadyCurrent(RevisionNumber::FIRST))
        );
    }

    #[test]
    fn changed_facts_create_the_next_revision_and_leave_the_head_untouched() {
        let first = register(id(1), None, facts(b"draft", STORY, InputPins::default()));
        let before = first.clone();
        let second = register(
            id(1),
            Some(&first),
            facts(b"draft 2", STORY, InputPins::default()),
        );
        assert_eq!(second.revision().get(), 2);
        assert_eq!(first, before);
        // Reverting to earlier bytes is a new revision, not a rewrite of history.
        let third = register(
            id(1),
            Some(&second),
            facts(b"draft", STORY, InputPins::default()),
        );
        assert_eq!(third.revision().get(), 3);
        assert_eq!(third.facts().content, first.facts().content);
    }

    #[test]
    fn a_different_rights_claim_or_producer_is_a_new_revision() {
        let first = register(id(1), None, facts(b"draft", STORY, InputPins::default()));
        let mut licensed = first.facts().clone();
        licensed.rights = RightsClaim::Licensed {
            reference: ClaimReference::parse("licence 2026-07").unwrap(),
        };
        assert_eq!(register(id(1), Some(&first), licensed).revision().get(), 2);
    }

    #[test]
    fn standalone_audio_and_text_without_script_are_valid_lineages() {
        // A recording with no script and narration derived straight from a manuscript.
        let recording = register(
            id(1),
            None,
            facts(b"RIFF", ArtifactPayload::Audio, InputPins::default()),
        );
        assert!(recording.facts().inputs.is_empty());
        let manuscript = register(id(2), None, facts(b"chuong 1", STORY, InputPins::default()));
        let narration_inputs = InputPins::new([manuscript.pin()]).unwrap();
        let narration = register(
            id(3),
            None,
            facts(b"OggS", ArtifactPayload::Audio, narration_inputs),
        );
        assert_eq!(
            narration.facts().inputs.iter().collect::<Vec<_>>(),
            [manuscript.pin()]
        );
    }

    #[test]
    fn refusals_carry_the_exact_reason() {
        let first = register(id(1), None, facts(b"draft", STORY, InputPins::default()));
        let self_pinned = InputPins::new([first.pin()]).unwrap();
        assert_eq!(
            decide_registration(
                id(1),
                Some(&first),
                facts(b"draft 2", STORY, self_pinned.clone())
            ),
            Err(RevisionError::SelfInput)
        );
        assert_eq!(
            decide_registration(id(1), None, facts(b"draft", STORY, self_pinned)),
            Err(RevisionError::SelfInput)
        );
        assert_eq!(
            decide_registration(
                id(1),
                Some(&first),
                facts(b"OggS", ArtifactPayload::Audio, InputPins::default())
            ),
            Err(RevisionError::KindChanged {
                from: ArtifactKind::Manuscript,
                to: ArtifactKind::Audio
            })
        );
        let collected = ArtifactPayload::Manuscript {
            origin: ManuscriptOrigin::Collected,
        };
        assert_eq!(
            decide_registration(
                id(1),
                Some(&first),
                facts(b"draft", collected, InputPins::default())
            ),
            Err(RevisionError::ManuscriptOriginChanged)
        );
    }

    #[test]
    fn the_last_revision_number_cannot_be_exceeded() {
        let head = ArtifactRevision {
            artifact: id(1),
            revision: RevisionNumber::new(u32::MAX).unwrap(),
            facts: facts(b"a", ArtifactPayload::Report, InputPins::default()),
        };
        assert_eq!(
            decide_registration(
                id(1),
                Some(&head),
                facts(b"b", ArtifactPayload::Report, InputPins::default())
            ),
            Err(RevisionError::Exhausted)
        );
    }

    #[test]
    fn an_artifact_can_be_pinned_only_once() {
        let digest = Sha256Digest::of(b"x");
        let pin = |revision| InputPin {
            artifact: id(9),
            revision: RevisionNumber::new(revision).unwrap(),
            digest,
        };
        assert_eq!(
            InputPins::new([pin(1), pin(2)]),
            Err(InputPinsError::RepeatedArtifact { artifact: id(9) })
        );
        assert_eq!(
            InputPins::new([pin(1), pin(1)]),
            Err(InputPinsError::RepeatedArtifact { artifact: id(9) })
        );
    }

    #[test]
    fn replay_reproduces_exactly_what_registration_created() {
        let first = facts(b"v1", STORY, InputPins::default());
        let second = facts(b"v2", STORY, InputPins::default());
        let created = {
            let v1 = register(id(1), None, first.clone());
            let v2 = register(id(1), Some(&v1), second.clone());
            vec![v1, v2]
        };
        assert_eq!(replay(id(1), [first, second]), Ok(created));
        assert_eq!(replay(id(1), []), Ok(Vec::new()));
    }

    #[test]
    fn replay_refuses_a_history_registration_would_refuse() {
        let story = facts(b"v1", STORY, InputPins::default());
        let audio = facts(b"v2", ArtifactPayload::Audio, InputPins::default());
        assert_eq!(
            replay(id(1), [story.clone(), audio]),
            Err(ReplayError::Refused {
                position: 1,
                error: RevisionError::KindChanged {
                    from: ArtifactKind::Manuscript,
                    to: ArtifactKind::Audio
                }
            })
        );
        assert_eq!(
            replay(id(1), [story.clone(), story]),
            Err(ReplayError::RepeatsPrevious { position: 1 })
        );
        let self_pinned = {
            let first = register(id(2), None, facts(b"x", STORY, InputPins::default()));
            let pins = InputPins::new([first.pin()]).unwrap();
            facts(b"y", STORY, pins)
        };
        assert_eq!(
            replay(id(2), [self_pinned]),
            Err(ReplayError::Refused {
                position: 0,
                error: RevisionError::SelfInput
            })
        );
    }
}
