//! Whether one audio revision may be exported as a local handoff bundle.
//!
//! Export prepares files for a later authorized submission; it is not that submission. A bundle
//! says nothing about server admission, review or publication, and an export decision never
//! turns producer-side measurements into approval.

use super::{
    artifact::{ArtifactKind, ArtifactPayload, ArtifactRevision, InputPin, RightsClaim},
    audio_profile::AudioProfile,
    evaluation::{self, CheckOutcome, EvaluationSummary, Evidence, Finding},
    identity::EvidenceId,
    label::CheckName,
    lineage::{self, Freshness, PinViolation, RevisionHistory, SupersededInput},
};

/// Everything the shell needs to write a bundle for exactly one revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportPlan {
    pub revision: ArtifactRevision,
    pub profile: AudioProfile,
    /// The passing deterministic evidence that opened the gate.
    pub gate_evidence: EvidenceId,
    /// All three layers for the revision, as recorded. AI and human layers are reported, never
    /// required here, and a missing layer stays `NotRun`.
    pub layers: EvaluationSummary,
    pub inputs: Vec<InputPin>,
}

/// One reason an export is refused. All applicable reasons are reported together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportRefusal {
    /// Only audio can be exported in this slice; nothing else is a submission candidate yet.
    NotAudio {
        kind: ArtifactKind,
    },
    /// Export applies to the head revision; a newer revision exists.
    SupersededRevision {
        head: u32,
    },
    /// A handoff must state rights; `Unstated` is not a claim.
    RightsUnstated,
    /// An input this revision was made from has a newer revision.
    InputsStale(Vec<SupersededInput>),
    /// A pin or evidence record contradicts recorded history; nothing can be trusted.
    HistoryViolations(Vec<PinViolation>),
    /// The profile check has not been run for exactly these bytes.
    CheckNotRun,
    /// The profile check exists only for another revision of this artifact.
    CheckStale,
    CheckFailed,
}

/// Decides whether `head` (the artifact's newest revision) may be exported for `profile`.
///
/// `gate` is the name of the profile check; only its evidence can open the gate.
pub fn plan_export(
    head: &ArtifactRevision,
    history: &RevisionHistory,
    evidence: &[Evidence],
    gate: &CheckName,
    profile: AudioProfile,
) -> Result<ExportPlan, Vec<ExportRefusal>> {
    let facts = head.facts();
    if facts.payload != ArtifactPayload::Audio {
        return Err(vec![ExportRefusal::NotAudio {
            kind: facts.payload.kind(),
        }]);
    }
    let mut refusals = Vec::new();
    if history.head(head.artifact()) != Some(head.revision()) {
        let current = history
            .head(head.artifact())
            .map_or(0, |number| number.get());
        refusals.push(ExportRefusal::SupersededRevision { head: current });
    }
    if facts.rights == RightsClaim::Unstated {
        refusals.push(ExportRefusal::RightsUnstated);
    }
    match lineage::assess(&facts.inputs, history) {
        Ok(Freshness::Current) => {}
        Ok(Freshness::Stale(superseded)) => refusals.push(ExportRefusal::InputsStale(superseded)),
        Err(violations) => refusals.push(ExportRefusal::HistoryViolations(violations)),
    }
    let subject = head.pin();
    let gate_records: Vec<Evidence> = evidence
        .iter()
        .filter(|record| is_gate_check(record, gate))
        .cloned()
        .collect();
    let gate_evidence = match evaluation::summarize(subject, &gate_records, history) {
        Err(violations) => {
            refusals.push(ExportRefusal::HistoryViolations(violations));
            None
        }
        Ok(summary) => match summary.deterministic {
            evaluation::LayerStatus::NotRun => {
                refusals.push(ExportRefusal::CheckNotRun);
                None
            }
            evaluation::LayerStatus::Stale => {
                refusals.push(ExportRefusal::CheckStale);
                None
            }
            evaluation::LayerStatus::Current(CheckOutcome::Failed) => {
                refusals.push(ExportRefusal::CheckFailed);
                None
            }
            evaluation::LayerStatus::Current(CheckOutcome::Passed) => {
                latest_passing(&gate_records, subject)
            }
        },
    };
    let layers = match evaluation::summarize(subject, evidence, history) {
        Ok(layers) => Some(layers),
        Err(violations) => {
            let reported = refusals
                .iter()
                .any(|refusal| matches!(refusal, ExportRefusal::HistoryViolations(_)));
            if !reported {
                refusals.push(ExportRefusal::HistoryViolations(violations));
            }
            None
        }
    };
    match (gate_evidence, layers, refusals.is_empty()) {
        (Some(gate_evidence), Some(layers), true) => Ok(ExportPlan {
            revision: head.clone(),
            profile,
            gate_evidence,
            layers,
            inputs: facts.inputs.iter().collect(),
        }),
        _ => Err(refusals),
    }
}

fn is_gate_check(record: &Evidence, gate: &CheckName) -> bool {
    matches!(&record.finding, Finding::Deterministic { check, .. } if check == gate)
}

/// The newest passing record about exactly `subject`.
fn latest_passing(records: &[Evidence], subject: InputPin) -> Option<EvidenceId> {
    records
        .iter()
        .rev()
        .find(|record| {
            record.subject == subject
                && matches!(
                    record.finding,
                    Finding::Deterministic {
                        outcome: CheckOutcome::Passed,
                        ..
                    }
                )
        })
        .map(|record| record.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        artifact::{testing::*, InputPins, ManuscriptOrigin, RevisionFacts},
        evaluation::{Assessment, LayerStatus},
        identity::ArtifactId,
        label::ModelLabel,
    };

    const GATE: &str = "ogg-opus-profile-v1:opus-mono-24-v1";
    const PROFILE: AudioProfile = AudioProfile::OpusMono24V1;
    const STORY: ArtifactPayload = ArtifactPayload::Manuscript {
        origin: ManuscriptOrigin::Original,
    };

    fn gate() -> CheckName {
        CheckName::parse(GATE).unwrap()
    }

    fn check(n: u128, subject: InputPin, name: &str, outcome: CheckOutcome) -> Evidence {
        Evidence {
            id: EvidenceId::from_bits(n),
            subject,
            basis: InputPins::default(),
            finding: Finding::Deterministic {
                check: CheckName::parse(name).unwrap(),
                outcome,
            },
        }
    }

    struct World {
        story: ArtifactRevision,
        audio: ArtifactRevision,
    }

    fn world() -> World {
        let story = register(
            ArtifactId::from_bits(1),
            None,
            facts(b"story", STORY, InputPins::default()),
        );
        let pins = InputPins::new([story.pin()]).unwrap();
        let audio = register(
            ArtifactId::from_bits(2),
            None,
            facts(b"ogg bytes", ArtifactPayload::Audio, pins),
        );
        World { story, audio }
    }

    fn history(revisions: &[&ArtifactRevision]) -> RevisionHistory {
        RevisionHistory::from_revisions(revisions.iter().copied()).unwrap()
    }

    fn plan(
        head: &ArtifactRevision,
        history: &RevisionHistory,
        evidence: &[Evidence],
    ) -> Result<ExportPlan, Vec<ExportRefusal>> {
        plan_export(head, history, evidence, &gate(), PROFILE)
    }

    #[test]
    fn a_passing_current_check_on_rights_and_fresh_inputs_opens_the_gate() {
        let w = world();
        let passed = check(10, w.audio.pin(), GATE, CheckOutcome::Passed);
        let plan = plan(&w.audio, &history(&[&w.story, &w.audio]), &[passed]).unwrap();
        assert_eq!(plan.gate_evidence, EvidenceId::from_bits(10));
        assert_eq!(plan.profile, PROFILE);
        assert_eq!(plan.inputs, vec![w.story.pin()]);
        assert_eq!(
            plan.layers.deterministic,
            LayerStatus::Current(CheckOutcome::Passed)
        );
        assert_eq!(plan.layers.ai_review, LayerStatus::NotRun);
        assert_eq!(plan.layers.human_review, LayerStatus::NotRun);
    }

    #[test]
    fn only_audio_can_be_exported() {
        let w = world();
        let refusals = plan(&w.story, &history(&[&w.story]), &[]).unwrap_err();
        assert_eq!(
            refusals,
            [ExportRefusal::NotAudio {
                kind: ArtifactKind::Manuscript
            }]
        );
    }

    #[test]
    fn a_missing_check_is_not_run_and_a_failed_check_is_failed() {
        let w = world();
        let h = history(&[&w.story, &w.audio]);
        assert_eq!(
            plan(&w.audio, &h, &[]).unwrap_err(),
            [ExportRefusal::CheckNotRun]
        );
        let failed = check(10, w.audio.pin(), GATE, CheckOutcome::Failed);
        assert_eq!(
            plan(&w.audio, &h, &[failed]).unwrap_err(),
            [ExportRefusal::CheckFailed]
        );
    }

    #[test]
    fn a_pass_for_another_profile_does_not_open_this_profiles_gate() {
        let w = world();
        let other = check(
            10,
            w.audio.pin(),
            "ogg-opus-profile-v1:opus-mono-40-v1",
            CheckOutcome::Passed,
        );
        assert_eq!(
            plan(&w.audio, &history(&[&w.story, &w.audio]), &[other]).unwrap_err(),
            [ExportRefusal::CheckNotRun]
        );
    }

    #[test]
    fn a_failure_for_another_profile_does_not_close_this_profiles_gate() {
        let w = world();
        let passed = check(10, w.audio.pin(), GATE, CheckOutcome::Passed);
        let other_failed = check(
            11,
            w.audio.pin(),
            "ogg-opus-profile-v1:opus-mono-20-v1",
            CheckOutcome::Failed,
        );
        let plan = plan(
            &w.audio,
            &history(&[&w.story, &w.audio]),
            &[passed, other_failed],
        )
        .unwrap();
        assert_eq!(plan.gate_evidence, EvidenceId::from_bits(10));
        // The recorded layer view still shows the unfavorable record honestly.
        assert_eq!(
            plan.layers.deterministic,
            LayerStatus::Current(CheckOutcome::Failed)
        );
    }

    #[test]
    fn a_new_audio_revision_makes_the_old_check_stale_and_the_old_revision_superseded() {
        let w = world();
        let passed = check(10, w.audio.pin(), GATE, CheckOutcome::Passed);
        let pins = InputPins::new([w.story.pin()]).unwrap();
        let audio_v2 = register(
            w.audio.artifact(),
            Some(&w.audio),
            facts(b"new ogg bytes", ArtifactPayload::Audio, pins),
        );
        let h = history(&[&w.story, &w.audio, &audio_v2]);
        // Exporting the head with only evidence about revision 1: the check is stale.
        assert_eq!(
            plan(&audio_v2, &h, std::slice::from_ref(&passed)).unwrap_err(),
            [ExportRefusal::CheckStale]
        );
        // Exporting revision 1 itself is refused as superseded.
        assert_eq!(
            plan(&w.audio, &h, &[passed]).unwrap_err(),
            [ExportRefusal::SupersededRevision { head: 2 }]
        );
    }

    #[test]
    fn unstated_rights_are_refused_together_with_every_other_reason() {
        let w = world();
        let pins = InputPins::new([w.story.pin()]).unwrap();
        let unstated = register(
            ArtifactId::from_bits(3),
            None,
            RevisionFacts {
                rights: RightsClaim::Unstated,
                ..facts(b"ogg", ArtifactPayload::Audio, pins)
            },
        );
        let h = history(&[&w.story, &unstated]);
        assert_eq!(
            plan(&unstated, &h, &[]).unwrap_err(),
            [ExportRefusal::RightsUnstated, ExportRefusal::CheckNotRun]
        );
    }

    #[test]
    fn a_newer_input_revision_marks_the_audio_stale_and_blocks_export() {
        let w = world();
        let story_v2 = register(
            w.story.artifact(),
            Some(&w.story),
            facts(b"story v2", STORY, InputPins::default()),
        );
        let passed = check(10, w.audio.pin(), GATE, CheckOutcome::Passed);
        let h = history(&[&w.story, &story_v2, &w.audio]);
        let refusals = plan(&w.audio, &h, &[passed]).unwrap_err();
        let [ExportRefusal::InputsStale(superseded)] = refusals.as_slice() else {
            panic!("expected only stale inputs, found {refusals:?}");
        };
        assert_eq!(superseded.len(), 1);
        assert_eq!(superseded[0].pin, w.story.pin());
    }

    #[test]
    fn evidence_contradicting_history_fails_closed() {
        let w = world();
        let forged = InputPin {
            digest: crate::domain::digest::Sha256Digest::of(b"other bytes"),
            ..w.audio.pin()
        };
        let evidence = [check(10, forged, GATE, CheckOutcome::Passed)];
        let refusals = plan(&w.audio, &history(&[&w.story, &w.audio]), &evidence).unwrap_err();
        assert!(
            refusals
                .iter()
                .any(|refusal| matches!(refusal, ExportRefusal::HistoryViolations(_))),
            "{refusals:?}"
        );
    }

    #[test]
    fn an_ai_review_is_reported_but_never_required_or_substituted_for_the_check() {
        let w = world();
        let h = history(&[&w.story, &w.audio]);
        let ai = Evidence {
            id: EvidenceId::from_bits(20),
            subject: w.audio.pin(),
            basis: InputPins::default(),
            finding: Finding::AiReview {
                model: ModelLabel::parse("reviewer model").unwrap(),
                assessment: Assessment::NoConcerns,
            },
        };
        // An AI review alone never opens the gate.
        assert_eq!(
            plan(&w.audio, &h, std::slice::from_ref(&ai)).unwrap_err(),
            [ExportRefusal::CheckNotRun]
        );
        let passed = check(10, w.audio.pin(), GATE, CheckOutcome::Passed);
        let plan = plan(&w.audio, &h, &[passed, ai]).unwrap();
        assert_eq!(
            plan.layers.ai_review,
            LayerStatus::Current(Assessment::NoConcerns)
        );
        assert_eq!(plan.layers.human_review, LayerStatus::NotRun);
    }
}
