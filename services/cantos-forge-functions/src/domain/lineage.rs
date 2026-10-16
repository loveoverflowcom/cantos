//! Whether pinned inputs are still the current revisions.
//!
//! Pins are never rewritten. When an input gains a newer revision, whatever depended on the old
//! one becomes stale and says so; it is neither silently updated nor silently reused. A pin that
//! contradicts recorded history is an integrity violation, never staleness.

use std::collections::{btree_map::Entry, BTreeMap};

use super::{
    artifact::{ArtifactRevision, InputPin, InputPins, RevisionNumber},
    digest::Sha256Digest,
    identity::ArtifactId,
};

/// The digest of every recorded revision of every artifact in one workspace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RevisionHistory(BTreeMap<ArtifactId, Vec<Sha256Digest>>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryError {
    /// Revisions of one artifact must be supplied as 1, 2, 3 … without gaps or repeats.
    OutOfSequence {
        artifact: ArtifactId,
        expected: u32,
        found: RevisionNumber,
    },
}

impl RevisionHistory {
    /// Builds history from revisions supplied in per-artifact revision order.
    pub fn from_revisions<'a>(
        revisions: impl IntoIterator<Item = &'a ArtifactRevision>,
    ) -> Result<Self, HistoryError> {
        let mut by_artifact: BTreeMap<ArtifactId, Vec<Sha256Digest>> = BTreeMap::new();
        for revision in revisions {
            let digests = match by_artifact.entry(revision.artifact()) {
                Entry::Vacant(entry) => entry.insert(Vec::new()),
                Entry::Occupied(entry) => entry.into_mut(),
            };
            let expected = u32::try_from(digests.len())
                .ok()
                .and_then(|count| count.checked_add(1));
            if expected != Some(revision.revision().get()) {
                return Err(HistoryError::OutOfSequence {
                    artifact: revision.artifact(),
                    expected: expected.unwrap_or(u32::MAX),
                    found: revision.revision(),
                });
            }
            digests.push(revision.facts().content);
        }
        Ok(Self(by_artifact))
    }

    pub fn head(&self, artifact: ArtifactId) -> Option<RevisionNumber> {
        let count = self.0.get(&artifact)?.len();
        u32::try_from(count)
            .ok()
            .and_then(|count| RevisionNumber::new(count).ok())
    }

    fn digest_at(&self, artifact: ArtifactId, revision: RevisionNumber) -> Option<Sha256Digest> {
        let index = usize::try_from(revision.get() - 1).ok()?;
        self.0.get(&artifact)?.get(index).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupersededInput {
    pub pin: InputPin,
    pub head: RevisionNumber,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinViolation {
    UnknownArtifact {
        pin: InputPin,
    },
    /// The pin names a revision that history does not contain yet.
    BeyondHead {
        pin: InputPin,
        head: RevisionNumber,
    },
    /// The recorded bytes of that revision differ from the pinned digest.
    DigestConflict {
        pin: InputPin,
        recorded: Sha256Digest,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Freshness {
    Current,
    /// At least one input has a newer revision, in artifact order.
    Stale(Vec<SupersededInput>),
}

/// Assesses every pin against history, reporting all violations (in artifact order) before
/// any staleness: a violation means the record cannot be trusted at all.
pub fn assess(pins: &InputPins, history: &RevisionHistory) -> Result<Freshness, Vec<PinViolation>> {
    let mut violations = Vec::new();
    let mut superseded = Vec::new();
    for pin in pins.iter() {
        match check_pin(pin, history) {
            PinCheck::Current => {}
            PinCheck::Superseded(input) => superseded.push(input),
            PinCheck::Violation(violation) => violations.push(violation),
        }
    }
    if !violations.is_empty() {
        return Err(violations);
    }
    if superseded.is_empty() {
        Ok(Freshness::Current)
    } else {
        Ok(Freshness::Stale(superseded))
    }
}

pub(crate) enum PinCheck {
    Current,
    Superseded(SupersededInput),
    Violation(PinViolation),
}

pub(crate) fn check_pin(pin: InputPin, history: &RevisionHistory) -> PinCheck {
    let Some(head) = history.head(pin.artifact) else {
        return PinCheck::Violation(PinViolation::UnknownArtifact { pin });
    };
    let Some(recorded) = history.digest_at(pin.artifact, pin.revision) else {
        return PinCheck::Violation(PinViolation::BeyondHead { pin, head });
    };
    if recorded != pin.digest {
        return PinCheck::Violation(PinViolation::DigestConflict { pin, recorded });
    }
    if pin.revision < head {
        PinCheck::Superseded(SupersededInput { pin, head })
    } else {
        PinCheck::Current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::artifact::{testing::*, ArtifactPayload};
    use proptest::prelude::*;

    fn id(bits: u128) -> ArtifactId {
        ArtifactId::from_bits(bits)
    }

    /// `versions[i]` revisions of artifact `i`, each with distinct bytes.
    fn workspace(versions: &[u8]) -> Vec<ArtifactRevision> {
        let mut revisions = Vec::new();
        for (index, &count) in versions.iter().enumerate() {
            let artifact = id(index as u128);
            let mut head = None;
            for version in 0..count {
                let bytes = format!("artifact {index} version {version}");
                let facts = facts(
                    bytes.as_bytes(),
                    ArtifactPayload::Report,
                    InputPins::default(),
                );
                let revision = register(artifact, head.as_ref(), facts);
                revisions.push(revision.clone());
                head = Some(revision);
            }
        }
        revisions
    }

    fn heads(revisions: &[ArtifactRevision]) -> Vec<InputPin> {
        let mut latest = BTreeMap::new();
        for revision in revisions {
            latest.insert(revision.artifact(), revision.pin());
        }
        latest.into_values().collect()
    }

    fn successor_of(revisions: &[ArtifactRevision], artifact: ArtifactId) -> ArtifactRevision {
        let head = revisions
            .iter()
            .rev()
            .find(|revision| revision.artifact() == artifact);
        register(
            artifact,
            head,
            facts(b"edited", ArtifactPayload::Report, InputPins::default()),
        )
    }

    #[test]
    fn history_requires_contiguous_revisions_per_artifact() {
        let revisions = workspace(&[2]);
        assert_eq!(
            RevisionHistory::from_revisions([&revisions[1]]),
            Err(HistoryError::OutOfSequence {
                artifact: id(0),
                expected: 1,
                found: RevisionNumber::new(2).unwrap(),
            })
        );
        assert_eq!(
            RevisionHistory::from_revisions([&revisions[0], &revisions[0]]),
            Err(HistoryError::OutOfSequence {
                artifact: id(0),
                expected: 2,
                found: RevisionNumber::FIRST,
            })
        );
    }

    #[test]
    fn no_inputs_is_current() {
        let history = RevisionHistory::from_revisions(&workspace(&[1])).unwrap();
        assert_eq!(
            assess(&InputPins::default(), &history),
            Ok(Freshness::Current)
        );
    }

    #[test]
    fn every_violation_is_reported_before_staleness() {
        let revisions = workspace(&[2, 1]);
        let history = RevisionHistory::from_revisions(&revisions).unwrap();
        let stale = revisions[0].pin();
        let mut tampered = revisions[2].pin();
        tampered.digest = Sha256Digest::of(b"tampered");
        let unknown = InputPin {
            artifact: id(7),
            ..stale
        };
        let pins = InputPins::new([stale, tampered, unknown]).unwrap();
        assert_eq!(
            assess(&pins, &history),
            Err(vec![
                PinViolation::DigestConflict {
                    pin: tampered,
                    recorded: revisions[2].facts().content,
                },
                PinViolation::UnknownArtifact { pin: unknown },
            ])
        );
    }

    #[test]
    fn a_pin_beyond_the_head_is_a_violation() {
        let revisions = workspace(&[1]);
        let history = RevisionHistory::from_revisions(&revisions).unwrap();
        let ahead = InputPin {
            revision: RevisionNumber::new(2).unwrap(),
            ..revisions[0].pin()
        };
        assert_eq!(
            assess(&InputPins::new([ahead]).unwrap(), &history),
            Err(vec![PinViolation::BeyondHead {
                pin: ahead,
                head: RevisionNumber::FIRST,
            }])
        );
    }

    proptest! {
        #[test]
        fn pins_of_every_head_are_current(versions in proptest::collection::vec(1_u8..4, 1..6)) {
            let revisions = workspace(&versions);
            let history = RevisionHistory::from_revisions(&revisions).unwrap();
            let pins = InputPins::new(heads(&revisions)).unwrap();
            prop_assert_eq!(assess(&pins, &history), Ok(Freshness::Current));
        }

        #[test]
        fn a_new_revision_of_one_pinned_input_marks_exactly_that_input_stale(
            versions in proptest::collection::vec(1_u8..4, 1..6),
            choice in any::<prop::sample::Index>(),
        ) {
            let mut revisions = workspace(&versions);
            let pins = InputPins::new(heads(&revisions)).unwrap();
            let changed = id(choice.index(versions.len()) as u128);
            let next = successor_of(&revisions, changed);
            let old_pin = pins.iter().find(|pin| pin.artifact == changed).unwrap();
            revisions.push(next.clone());
            let history = RevisionHistory::from_revisions(&revisions).unwrap();
            prop_assert_eq!(
                assess(&pins, &history),
                Ok(Freshness::Stale(vec![SupersededInput { pin: old_pin, head: next.revision() }]))
            );
        }

        #[test]
        fn a_new_revision_of_an_unpinned_artifact_changes_nothing(
            versions in proptest::collection::vec(1_u8..4, 2..6),
            choice in any::<prop::sample::Index>(),
        ) {
            let mut revisions = workspace(&versions);
            let unpinned = id(choice.index(versions.len()) as u128);
            let pins = InputPins::new(
                heads(&revisions).into_iter().filter(|pin| pin.artifact != unpinned),
            ).unwrap();
            revisions.push(successor_of(&revisions, unpinned));
            let history = RevisionHistory::from_revisions(&revisions).unwrap();
            prop_assert_eq!(assess(&pins, &history), Ok(Freshness::Current));
        }

        #[test]
        fn a_pin_whose_bytes_differ_from_history_is_never_current_or_stale(
            versions in proptest::collection::vec(1_u8..4, 1..6),
            choice in any::<prop::sample::Index>(),
            noise in proptest::collection::vec(any::<u8>(), 1..16),
        ) {
            let revisions = workspace(&versions);
            let history = RevisionHistory::from_revisions(&revisions).unwrap();
            let target = &revisions[choice.index(revisions.len())];
            let forged = InputPin { digest: Sha256Digest::of(&noise), ..target.pin() };
            prop_assume!(forged.digest != target.facts().content);
            prop_assert_eq!(
                assess(&InputPins::new([forged]).unwrap(), &history),
                Err(vec![PinViolation::DigestConflict { pin: forged, recorded: target.facts().content }])
            );
        }
    }
}
