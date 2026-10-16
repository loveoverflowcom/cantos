//! Evaluation evidence and its three independent layers.
//!
//! Deterministic checks, AI review and named human review are separate layers. None of them
//! implies another, a layer without current evidence stays `NotRun`, and nothing here is a
//! Cantos Studio approval, a server admission or a publication decision.

use super::{
    artifact::{InputPin, InputPins},
    identity::EvidenceId,
    label::{CheckName, ModelLabel, ReviewerName},
    lineage::{self, Freshness, PinCheck, PinViolation, RevisionHistory},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckOutcome {
    Passed,
    Failed,
}

/// An AI reviewer's assessment against a rubric: advisory, never an approval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assessment {
    NoConcerns,
    Concerns,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewMode {
    Read,
    Listened,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HumanDecision {
    Accepted,
    ChangesRequested,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finding {
    /// A validation or measurement whose result does not depend on judgement.
    Deterministic {
        check: CheckName,
        outcome: CheckOutcome,
    },
    AiReview {
        model: ModelLabel,
        assessment: Assessment,
    },
    HumanReview {
        reviewer: ReviewerName,
        mode: ReviewMode,
        decision: HumanDecision,
    },
}

/// One append-only evidence record about exactly one artifact revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evidence {
    pub id: EvidenceId,
    /// The exact revision and bytes that were evaluated.
    pub subject: InputPin,
    /// Everything else the result depended on: rubric, instruction bundle, reference text.
    pub basis: InputPins,
    pub finding: Finding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerStatus<T> {
    /// No evidence about this layer exists for the subject at all.
    NotRun,
    /// Evidence exists, but only for another revision of the subject or a superseded basis.
    Stale,
    /// The combined result of current evidence; any unfavorable record wins.
    Current(T),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationSummary {
    pub deterministic: LayerStatus<CheckOutcome>,
    pub ai_review: LayerStatus<Assessment>,
    pub human_review: LayerStatus<HumanDecision>,
}

/// Summarizes the evidence about `subject`. Evidence about other artifacts is ignored.
///
/// Fails closed with every integrity violation when the subject pin or any relevant
/// evidence record contradicts history.
pub fn summarize(
    subject: InputPin,
    evidence: &[Evidence],
    history: &RevisionHistory,
) -> Result<EvaluationSummary, Vec<PinViolation>> {
    let mut violations = match lineage::check_pin(subject, history) {
        PinCheck::Violation(violation) => vec![violation],
        PinCheck::Current | PinCheck::Superseded(_) => Vec::new(),
    };
    let mut summary = EvaluationSummary {
        deterministic: LayerStatus::NotRun,
        ai_review: LayerStatus::NotRun,
        human_review: LayerStatus::NotRun,
    };
    for record in evidence
        .iter()
        .filter(|record| record.subject.artifact == subject.artifact)
    {
        let current = match currency(record, subject, history) {
            Ok(current) => current,
            Err(mut found) => {
                violations.append(&mut found);
                continue;
            }
        };
        match &record.finding {
            Finding::Deterministic { outcome, .. } => {
                summary.deterministic = combine(
                    summary.deterministic,
                    current,
                    *outcome,
                    CheckOutcome::Failed,
                );
            }
            Finding::AiReview { assessment, .. } => {
                summary.ai_review = combine(
                    summary.ai_review,
                    current,
                    *assessment,
                    Assessment::Concerns,
                );
            }
            Finding::HumanReview { decision, .. } => {
                summary.human_review = combine(
                    summary.human_review,
                    current,
                    *decision,
                    HumanDecision::ChangesRequested,
                );
            }
        }
    }
    if violations.is_empty() {
        Ok(summary)
    } else {
        Err(violations)
    }
}

/// Whether a record about the subject's artifact counts for exactly this subject revision.
fn currency(
    record: &Evidence,
    subject: InputPin,
    history: &RevisionHistory,
) -> Result<bool, Vec<PinViolation>> {
    let mut violations = match lineage::check_pin(record.subject, history) {
        PinCheck::Violation(violation) => vec![violation],
        PinCheck::Current | PinCheck::Superseded(_) => Vec::new(),
    };
    let basis = match lineage::assess(&record.basis, history) {
        Ok(freshness) => Some(freshness),
        Err(mut found) => {
            violations.append(&mut found);
            None
        }
    };
    if !violations.is_empty() {
        return Err(violations);
    }
    Ok(record.subject == subject && basis == Some(Freshness::Current))
}

fn combine<T: Copy + PartialEq>(
    status: LayerStatus<T>,
    current: bool,
    result: T,
    unfavorable: T,
) -> LayerStatus<T> {
    match (status, current) {
        (LayerStatus::Current(existing), true) if existing == unfavorable => status,
        (_, true) => LayerStatus::Current(result),
        (LayerStatus::NotRun, false) => LayerStatus::Stale,
        (status, false) => status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        artifact::{testing::*, ArtifactPayload, ArtifactRevision, ManuscriptOrigin},
        digest::Sha256Digest,
        identity::ArtifactId,
    };

    const STORY: ArtifactPayload = ArtifactPayload::Manuscript {
        origin: ManuscriptOrigin::Original,
    };

    struct Fixture {
        story_v1: ArtifactRevision,
        story_v2: ArtifactRevision,
        rubric_v1: ArtifactRevision,
        rubric_v2: Option<ArtifactRevision>,
        other: ArtifactRevision,
    }

    impl Fixture {
        fn new() -> Self {
            let story_v1 = register(
                ArtifactId::from_bits(1),
                None,
                facts(b"Mot loi hen v1", STORY, InputPins::default()),
            );
            let story_v2 = register(
                ArtifactId::from_bits(1),
                Some(&story_v1),
                facts(b"Mot loi hen v2", STORY, InputPins::default()),
            );
            let rubric_v1 = register(
                ArtifactId::from_bits(2),
                None,
                facts(
                    b"rubric",
                    ArtifactPayload::InstructionBundle,
                    InputPins::default(),
                ),
            );
            let other = register(
                ArtifactId::from_bits(3),
                None,
                facts(b"other", STORY, InputPins::default()),
            );
            Self {
                story_v1,
                story_v2,
                rubric_v1,
                rubric_v2: None,
                other,
            }
        }

        fn revise_rubric(mut self) -> Self {
            let next = register(
                ArtifactId::from_bits(2),
                Some(&self.rubric_v1),
                facts(
                    b"rubric 2",
                    ArtifactPayload::InstructionBundle,
                    InputPins::default(),
                ),
            );
            self.rubric_v2 = Some(next);
            self
        }

        fn history(&self) -> RevisionHistory {
            let mut revisions = vec![&self.story_v1, &self.story_v2, &self.rubric_v1, &self.other];
            revisions.extend(self.rubric_v2.as_ref());
            RevisionHistory::from_revisions(revisions).unwrap()
        }
    }

    fn record(subject: InputPin, basis: &[InputPin], finding: Finding) -> Evidence {
        Evidence {
            id: EvidenceId::from_bits(u128::from(subject.revision.get())),
            subject,
            basis: InputPins::new(basis.iter().copied()).unwrap(),
            finding,
        }
    }

    fn check(outcome: CheckOutcome) -> Finding {
        Finding::Deterministic {
            check: CheckName::parse("utf8-text").unwrap(),
            outcome,
        }
    }

    fn ai(assessment: Assessment) -> Finding {
        Finding::AiReview {
            model: ModelLabel::parse("reviewer-model rubric-v1").unwrap(),
            assessment,
        }
    }

    fn human(decision: HumanDecision) -> Finding {
        Finding::HumanReview {
            reviewer: ReviewerName::parse("Trần Minh Anh").unwrap(),
            mode: ReviewMode::Read,
            decision,
        }
    }

    const NOTHING: EvaluationSummary = EvaluationSummary {
        deterministic: LayerStatus::NotRun,
        ai_review: LayerStatus::NotRun,
        human_review: LayerStatus::NotRun,
    };

    #[test]
    fn without_evidence_every_layer_is_not_run() {
        let fixture = Fixture::new();
        assert_eq!(
            summarize(fixture.story_v2.pin(), &[], &fixture.history()),
            Ok(NOTHING)
        );
    }

    #[test]
    fn no_layer_implies_another() {
        let fixture = Fixture::new();
        let subject = fixture.story_v2.pin();
        let cases = [
            (
                check(CheckOutcome::Passed),
                EvaluationSummary {
                    deterministic: LayerStatus::Current(CheckOutcome::Passed),
                    ..NOTHING
                },
            ),
            (
                ai(Assessment::NoConcerns),
                EvaluationSummary {
                    ai_review: LayerStatus::Current(Assessment::NoConcerns),
                    ..NOTHING
                },
            ),
            (
                human(HumanDecision::Accepted),
                EvaluationSummary {
                    human_review: LayerStatus::Current(HumanDecision::Accepted),
                    ..NOTHING
                },
            ),
        ];
        for (finding, expected) in cases {
            let evidence = [record(subject, &[], finding)];
            assert_eq!(
                summarize(subject, &evidence, &fixture.history()),
                Ok(expected)
            );
        }
    }

    #[test]
    fn evidence_for_another_revision_is_stale_not_current() {
        let fixture = Fixture::new();
        let evidence = [
            record(fixture.story_v1.pin(), &[], check(CheckOutcome::Passed)),
            record(fixture.story_v1.pin(), &[], human(HumanDecision::Accepted)),
        ];
        assert_eq!(
            summarize(fixture.story_v2.pin(), &evidence, &fixture.history()),
            Ok(EvaluationSummary {
                deterministic: LayerStatus::Stale,
                human_review: LayerStatus::Stale,
                ..NOTHING
            })
        );
    }

    #[test]
    fn a_superseded_rubric_makes_its_review_stale() {
        let fixture = Fixture::new();
        let subject = fixture.story_v2.pin();
        let evidence = [record(
            subject,
            &[fixture.rubric_v1.pin()],
            ai(Assessment::NoConcerns),
        )];
        assert_eq!(
            summarize(subject, &evidence, &fixture.history()),
            Ok(EvaluationSummary {
                ai_review: LayerStatus::Current(Assessment::NoConcerns),
                ..NOTHING
            })
        );
        let revised = fixture.revise_rubric();
        assert_eq!(
            summarize(subject, &evidence, &revised.history()),
            Ok(EvaluationSummary {
                ai_review: LayerStatus::Stale,
                ..NOTHING
            })
        );
    }

    #[test]
    fn current_evidence_replaces_stale_and_unfavorable_results_win() {
        let fixture = Fixture::new();
        let subject = fixture.story_v2.pin();
        let evidence = [
            record(fixture.story_v1.pin(), &[], check(CheckOutcome::Failed)),
            record(subject, &[], check(CheckOutcome::Passed)),
            record(subject, &[], check(CheckOutcome::Failed)),
            record(subject, &[], check(CheckOutcome::Passed)),
            record(subject, &[], human(HumanDecision::ChangesRequested)),
            record(subject, &[], human(HumanDecision::Accepted)),
        ];
        assert_eq!(
            summarize(subject, &evidence, &fixture.history()),
            Ok(EvaluationSummary {
                deterministic: LayerStatus::Current(CheckOutcome::Failed),
                human_review: LayerStatus::Current(HumanDecision::ChangesRequested),
                ..NOTHING
            })
        );
    }

    #[test]
    fn evidence_about_other_artifacts_is_ignored() {
        let fixture = Fixture::new();
        let evidence = [record(
            fixture.other.pin(),
            &[],
            human(HumanDecision::Accepted),
        )];
        assert_eq!(
            summarize(fixture.story_v2.pin(), &evidence, &fixture.history()),
            Ok(NOTHING)
        );
    }

    #[test]
    fn tampered_subjects_or_evidence_fail_closed_with_every_violation() {
        let fixture = Fixture::new();
        let mut forged_subject = fixture.story_v2.pin();
        forged_subject.digest = Sha256Digest::of(b"forged");
        let mut forged_basis = fixture.rubric_v1.pin();
        forged_basis.digest = Sha256Digest::of(b"forged rubric");
        let evidence = [
            record(
                fixture.story_v2.pin(),
                &[forged_basis],
                ai(Assessment::NoConcerns),
            ),
            record(forged_subject, &[], check(CheckOutcome::Passed)),
        ];
        assert_eq!(
            summarize(forged_subject, &evidence, &fixture.history()),
            Err(vec![
                PinViolation::DigestConflict {
                    pin: forged_subject,
                    recorded: fixture.story_v2.facts().content
                },
                PinViolation::DigestConflict {
                    pin: forged_basis,
                    recorded: fixture.rubric_v1.facts().content
                },
                PinViolation::DigestConflict {
                    pin: forged_subject,
                    recorded: fixture.story_v2.facts().content
                },
            ])
        );
    }
}
