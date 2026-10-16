//! The JSON shapes of Forge's persistent records, bundle manifests and CLI results.
//!
//! One definition per concept, used for every direction: the workspace writes and re-reads
//! these records, and the CLI and bundles print them. Reading goes through these raw shapes and
//! then through the domain's own constructors (`TryFrom`), so a record can never forge a value
//! the domain would refuse. Unknown fields are refused, never ignored.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::{
    artifact::{
        ArtifactPayload, InputPin, InputPins, ManuscriptOrigin, Provenance, RevisionFacts,
        RevisionNumber, RightsClaim,
    },
    checks::{Detail, DetailValue},
    digest::Sha256Digest,
    evaluation::{
        Assessment, CheckOutcome, EvaluationSummary, Evidence, Finding, HumanDecision, LayerStatus,
        ReviewMode,
    },
    identity::{ArtifactId, EvidenceId},
    label::{CheckName, ClaimReference, ModelLabel, ProducerLabel, ReviewerName},
};

pub const REVISION_SCHEMA: &str = "cantos-forge.revision/v1";
pub const EVIDENCE_SCHEMA: &str = "cantos-forge.evidence/v1";

/// A record that does not describe a value the domain accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WireError(pub String);

impl WireError {
    fn field(name: &str, reason: impl std::fmt::Debug) -> Self {
        Self(format!("{name}: {reason:?}"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinWire {
    pub artifact: String,
    pub revision: u32,
    pub digest: String,
}

impl From<InputPin> for PinWire {
    fn from(pin: InputPin) -> Self {
        Self {
            artifact: pin.artifact.to_string(),
            revision: pin.revision.get(),
            digest: pin.digest.to_string(),
        }
    }
}

impl TryFrom<&PinWire> for InputPin {
    type Error = WireError;

    fn try_from(wire: &PinWire) -> Result<Self, WireError> {
        Ok(Self {
            artifact: ArtifactId::parse(&wire.artifact)
                .map_err(|error| WireError::field("pin.artifact", error))?,
            revision: RevisionNumber::new(wire.revision)
                .map_err(|error| WireError::field("pin.revision", error))?,
            digest: Sha256Digest::parse(&wire.digest)
                .map_err(|error| WireError::field("pin.digest", error))?,
        })
    }
}

fn pins_from(wires: &[PinWire]) -> Result<InputPins, WireError> {
    let pins = wires
        .iter()
        .map(InputPin::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    InputPins::new(pins).map_err(|error| WireError::field("inputs", error))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginWire {
    Original,
    Collected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PayloadWire {
    Manuscript { origin: OriginWire },
    ScriptIr,
    Audio,
    Report,
    InstructionBundle,
}

impl From<&ArtifactPayload> for PayloadWire {
    fn from(payload: &ArtifactPayload) -> Self {
        match payload {
            ArtifactPayload::Manuscript { origin } => Self::Manuscript {
                origin: match origin {
                    ManuscriptOrigin::Original => OriginWire::Original,
                    ManuscriptOrigin::Collected => OriginWire::Collected,
                },
            },
            ArtifactPayload::ScriptIr => Self::ScriptIr,
            ArtifactPayload::Audio => Self::Audio,
            ArtifactPayload::Report => Self::Report,
            ArtifactPayload::InstructionBundle => Self::InstructionBundle,
        }
    }
}

impl From<&PayloadWire> for ArtifactPayload {
    fn from(wire: &PayloadWire) -> Self {
        match wire {
            PayloadWire::Manuscript { origin } => Self::Manuscript {
                origin: match origin {
                    OriginWire::Original => ManuscriptOrigin::Original,
                    OriginWire::Collected => ManuscriptOrigin::Collected,
                },
            },
            PayloadWire::ScriptIr => Self::ScriptIr,
            PayloadWire::Audio => Self::Audio,
            PayloadWire::Report => Self::Report,
            PayloadWire::InstructionBundle => Self::InstructionBundle,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProvenanceWire {
    HostAssisted { producer: String },
}

impl From<&Provenance> for ProvenanceWire {
    fn from(provenance: &Provenance) -> Self {
        match provenance {
            Provenance::HostAssisted { producer } => Self::HostAssisted {
                producer: producer.to_string(),
            },
        }
    }
}

impl TryFrom<&ProvenanceWire> for Provenance {
    type Error = WireError;

    fn try_from(wire: &ProvenanceWire) -> Result<Self, WireError> {
        match wire {
            ProvenanceWire::HostAssisted { producer } => Ok(Self::HostAssisted {
                producer: ProducerLabel::parse(producer)
                    .map_err(|error| WireError::field("provenance.producer", error))?,
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RightsWire {
    Unstated,
    OriginalWork,
    Licensed { reference: String },
    PermittedSource { reference: String },
}

impl From<&RightsClaim> for RightsWire {
    fn from(claim: &RightsClaim) -> Self {
        match claim {
            RightsClaim::Unstated => Self::Unstated,
            RightsClaim::OriginalWork => Self::OriginalWork,
            RightsClaim::Licensed { reference } => Self::Licensed {
                reference: reference.to_string(),
            },
            RightsClaim::PermittedSource { reference } => Self::PermittedSource {
                reference: reference.to_string(),
            },
        }
    }
}

impl TryFrom<&RightsWire> for RightsClaim {
    type Error = WireError;

    fn try_from(wire: &RightsWire) -> Result<Self, WireError> {
        let reference = |text: &str| {
            ClaimReference::parse(text).map_err(|error| WireError::field("rights.reference", error))
        };
        Ok(match wire {
            RightsWire::Unstated => Self::Unstated,
            RightsWire::OriginalWork => Self::OriginalWork,
            RightsWire::Licensed { reference: text } => Self::Licensed {
                reference: reference(text)?,
            },
            RightsWire::PermittedSource { reference: text } => Self::PermittedSource {
                reference: reference(text)?,
            },
        })
    }
}

/// One immutable revision as stored in the workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionWire {
    pub schema: String,
    pub artifact: String,
    pub revision: u32,
    pub content: String,
    pub payload: PayloadWire,
    pub provenance: ProvenanceWire,
    pub rights: RightsWire,
    pub inputs: Vec<PinWire>,
}

impl RevisionWire {
    pub fn from_facts(
        artifact: ArtifactId,
        revision: RevisionNumber,
        facts: &RevisionFacts,
    ) -> Self {
        Self {
            schema: REVISION_SCHEMA.to_owned(),
            artifact: artifact.to_string(),
            revision: revision.get(),
            content: facts.content.to_string(),
            payload: (&facts.payload).into(),
            provenance: (&facts.provenance).into(),
            rights: (&facts.rights).into(),
            inputs: facts.inputs.iter().map(PinWire::from).collect(),
        }
    }

    /// The identity, the number the record claims, and the facts it declares.
    pub fn into_facts(&self) -> Result<(ArtifactId, u32, RevisionFacts), WireError> {
        if self.schema != REVISION_SCHEMA {
            return Err(WireError::field("schema", &self.schema));
        }
        let artifact = ArtifactId::parse(&self.artifact)
            .map_err(|error| WireError::field("artifact", error))?;
        let facts = RevisionFacts {
            content: Sha256Digest::parse(&self.content)
                .map_err(|error| WireError::field("content", error))?,
            payload: (&self.payload).into(),
            provenance: (&self.provenance).try_into()?,
            rights: (&self.rights).try_into()?,
            inputs: pins_from(&self.inputs)?,
        };
        Ok((artifact, self.revision, facts))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeWire {
    Passed,
    Failed,
}

impl From<CheckOutcome> for OutcomeWire {
    fn from(outcome: CheckOutcome) -> Self {
        match outcome {
            CheckOutcome::Passed => Self::Passed,
            CheckOutcome::Failed => Self::Failed,
        }
    }
}

impl From<OutcomeWire> for CheckOutcome {
    fn from(wire: OutcomeWire) -> Self {
        match wire {
            OutcomeWire::Passed => Self::Passed,
            OutcomeWire::Failed => Self::Failed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FindingWire {
    Deterministic {
        check: String,
        outcome: OutcomeWire,
    },
    AiReview {
        model: String,
        assessment: AssessmentWire,
    },
    HumanReview {
        reviewer: String,
        mode: ModeWire,
        decision: DecisionWire,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentWire {
    NoConcerns,
    Concerns,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeWire {
    Read,
    Listened,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionWire {
    Accepted,
    ChangesRequested,
}

impl From<&Finding> for FindingWire {
    fn from(finding: &Finding) -> Self {
        match finding {
            Finding::Deterministic { check, outcome } => Self::Deterministic {
                check: check.to_string(),
                outcome: (*outcome).into(),
            },
            Finding::AiReview { model, assessment } => Self::AiReview {
                model: model.to_string(),
                assessment: match assessment {
                    Assessment::NoConcerns => AssessmentWire::NoConcerns,
                    Assessment::Concerns => AssessmentWire::Concerns,
                },
            },
            Finding::HumanReview {
                reviewer,
                mode,
                decision,
            } => Self::HumanReview {
                reviewer: reviewer.to_string(),
                mode: match mode {
                    ReviewMode::Read => ModeWire::Read,
                    ReviewMode::Listened => ModeWire::Listened,
                },
                decision: match decision {
                    HumanDecision::Accepted => DecisionWire::Accepted,
                    HumanDecision::ChangesRequested => DecisionWire::ChangesRequested,
                },
            },
        }
    }
}

impl TryFrom<&FindingWire> for Finding {
    type Error = WireError;

    fn try_from(wire: &FindingWire) -> Result<Self, WireError> {
        Ok(match wire {
            FindingWire::Deterministic { check, outcome } => Self::Deterministic {
                check: CheckName::parse(check)
                    .map_err(|error| WireError::field("finding.check", error))?,
                outcome: (*outcome).into(),
            },
            FindingWire::AiReview { model, assessment } => Self::AiReview {
                model: ModelLabel::parse(model)
                    .map_err(|error| WireError::field("finding.model", error))?,
                assessment: match assessment {
                    AssessmentWire::NoConcerns => Assessment::NoConcerns,
                    AssessmentWire::Concerns => Assessment::Concerns,
                },
            },
            FindingWire::HumanReview {
                reviewer,
                mode,
                decision,
            } => Self::HumanReview {
                reviewer: ReviewerName::parse(reviewer)
                    .map_err(|error| WireError::field("finding.reviewer", error))?,
                mode: match mode {
                    ModeWire::Read => ReviewMode::Read,
                    ModeWire::Listened => ReviewMode::Listened,
                },
                decision: match decision {
                    DecisionWire::Accepted => HumanDecision::Accepted,
                    DecisionWire::ChangesRequested => HumanDecision::ChangesRequested,
                },
            },
        })
    }
}

/// A measured value: a number stays a number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DetailWire {
    Count(u64),
    Codes(Vec<String>),
    Text(String),
}

pub fn detail_to_wire(detail: &Detail) -> BTreeMap<String, DetailWire> {
    detail
        .iter()
        .map(|(key, value)| {
            let wire = match value {
                DetailValue::Count(count) => DetailWire::Count(*count),
                DetailValue::Codes(codes) => DetailWire::Codes(codes.clone()),
                DetailValue::Text(text) => DetailWire::Text(text.clone()),
            };
            (key.to_owned(), wire)
        })
        .collect()
}

pub fn detail_from_wire(wire: &BTreeMap<String, DetailWire>) -> Detail {
    Detail::new(wire.iter().map(|(key, value)| {
        let value = match value {
            DetailWire::Count(count) => DetailValue::Count(*count),
            DetailWire::Codes(codes) => DetailValue::Codes(codes.clone()),
            DetailWire::Text(text) => DetailValue::Text(text.clone()),
        };
        (key.clone(), value)
    }))
}

/// One append-only evidence record as stored in the workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceWire {
    pub schema: String,
    pub id: String,
    pub subject: PinWire,
    pub basis: Vec<PinWire>,
    pub finding: FindingWire,
    pub detail: BTreeMap<String, DetailWire>,
}

impl EvidenceWire {
    pub fn from_record(evidence: &Evidence, detail: &Detail) -> Self {
        Self {
            schema: EVIDENCE_SCHEMA.to_owned(),
            id: evidence.id.to_string(),
            subject: evidence.subject.into(),
            basis: evidence.basis.iter().map(PinWire::from).collect(),
            finding: (&evidence.finding).into(),
            detail: detail_to_wire(detail),
        }
    }

    pub fn into_record(&self) -> Result<(Evidence, Detail), WireError> {
        if self.schema != EVIDENCE_SCHEMA {
            return Err(WireError::field("schema", &self.schema));
        }
        let evidence = Evidence {
            id: EvidenceId::parse(&self.id).map_err(|error| WireError::field("id", error))?,
            subject: InputPin::try_from(&self.subject)?,
            basis: pins_from(&self.basis)?,
            finding: (&self.finding).try_into()?,
        };
        Ok((evidence, detail_from_wire(&self.detail)))
    }
}

/// An evaluation layer as the CLI and bundles report it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LayerWire {
    NotRun,
    Stale,
    Current { result: &'static str },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LayersWire {
    pub deterministic: LayerWire,
    pub ai_review: LayerWire,
    pub human_review: LayerWire,
}

fn layer<T: Copy>(status: LayerStatus<T>, name: impl Fn(T) -> &'static str) -> LayerWire {
    match status {
        LayerStatus::NotRun => LayerWire::NotRun,
        LayerStatus::Stale => LayerWire::Stale,
        LayerStatus::Current(result) => LayerWire::Current {
            result: name(result),
        },
    }
}

impl From<&EvaluationSummary> for LayersWire {
    fn from(summary: &EvaluationSummary) -> Self {
        Self {
            deterministic: layer(summary.deterministic, |outcome| match outcome {
                CheckOutcome::Passed => "passed",
                CheckOutcome::Failed => "failed",
            }),
            ai_review: layer(summary.ai_review, |assessment| match assessment {
                Assessment::NoConcerns => "no_concerns",
                Assessment::Concerns => "concerns",
            }),
            human_review: layer(summary.human_review, |decision| match decision {
                HumanDecision::Accepted => "accepted",
                HumanDecision::ChangesRequested => "changes_requested",
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        artifact::{testing::*, InputPins},
        label::CheckName,
    };

    fn revision() -> RevisionFacts {
        let pin = InputPin {
            artifact: ArtifactId::from_bits(5),
            revision: RevisionNumber::new(3).unwrap(),
            digest: Sha256Digest::of(b"pinned"),
        };
        RevisionFacts {
            rights: RightsClaim::Licensed {
                reference: ClaimReference::parse("Giấy phép CC-BY 4.0").unwrap(),
            },
            ..facts(
                "Lời hẹn".as_bytes(),
                ArtifactPayload::Manuscript {
                    origin: ManuscriptOrigin::Collected,
                },
                InputPins::new([pin]).unwrap(),
            )
        }
    }

    #[test]
    fn a_revision_round_trips_through_its_wire_form_exactly() {
        let facts = revision();
        let wire = RevisionWire::from_facts(
            ArtifactId::from_bits(9),
            RevisionNumber::new(2).unwrap(),
            &facts,
        );
        let json = serde_json::to_string(&wire).unwrap();
        let back: RevisionWire = serde_json::from_str(&json).unwrap();
        assert_eq!(back.into_facts(), Ok((ArtifactId::from_bits(9), 2, facts)));
    }

    #[test]
    fn the_revision_json_shape_is_stable() {
        let wire = RevisionWire::from_facts(
            ArtifactId::from_bits(1),
            RevisionNumber::FIRST,
            &facts(b"x", ArtifactPayload::Audio, InputPins::default()),
        );
        assert_eq!(
            serde_json::to_string(&wire).unwrap(),
            format!(
                "{{\"schema\":\"cantos-forge.revision/v1\",\"artifact\":\"art_{:032x}\",\"revision\":1,\
                 \"content\":\"{}\",\"payload\":{{\"type\":\"audio\"}},\
                 \"provenance\":{{\"type\":\"host_assisted\",\"producer\":\"AI host session\"}},\
                 \"rights\":{{\"type\":\"original_work\"}},\"inputs\":[]}}",
                1,
                Sha256Digest::of(b"x")
            )
        );
    }

    #[test]
    fn unknown_fields_wrong_schemas_and_invalid_values_are_refused() {
        let wire = RevisionWire::from_facts(
            ArtifactId::from_bits(1),
            RevisionNumber::FIRST,
            &facts(b"x", ArtifactPayload::Audio, InputPins::default()),
        );
        let mut value = serde_json::to_value(&wire).unwrap();
        value["surprise"] = true.into();
        assert!(serde_json::from_value::<RevisionWire>(value).is_err());

        let mut wrong = wire.clone();
        wrong.schema = "cantos-forge.revision/v2".to_owned();
        assert!(wrong.into_facts().is_err());

        let mut bad_digest = wire.clone();
        bad_digest.content = "sha256:ABC".to_owned();
        assert!(bad_digest.into_facts().is_err());

        let mut bad_producer = wire.clone();
        bad_producer.provenance = ProvenanceWire::HostAssisted {
            producer: " padded ".to_owned(),
        };
        assert!(bad_producer.into_facts().is_err());

        let mut zero_revision = wire;
        zero_revision.inputs = vec![PinWire {
            artifact: ArtifactId::from_bits(2).to_string(),
            revision: 0,
            digest: Sha256Digest::of(b"y").to_string(),
        }];
        assert!(zero_revision.into_facts().is_err());
    }

    #[test]
    fn every_finding_round_trips() {
        let subject = InputPin {
            artifact: ArtifactId::from_bits(1),
            revision: RevisionNumber::FIRST,
            digest: Sha256Digest::of(b"a"),
        };
        let findings = [
            Finding::Deterministic {
                check: CheckName::parse("utf8-text-v1").unwrap(),
                outcome: CheckOutcome::Failed,
            },
            Finding::AiReview {
                model: ModelLabel::parse("reviewer").unwrap(),
                assessment: Assessment::Concerns,
            },
            Finding::HumanReview {
                reviewer: ReviewerName::parse("Nguyễn Thị Hồng").unwrap(),
                mode: ReviewMode::Listened,
                decision: HumanDecision::ChangesRequested,
            },
        ];
        for finding in findings {
            let evidence = Evidence {
                id: EvidenceId::from_bits(7),
                subject,
                basis: InputPins::default(),
                finding,
            };
            let detail = Detail::new([
                ("n".to_owned(), DetailValue::Count(3)),
                ("codes".to_owned(), DetailValue::Codes(vec!["a".to_owned()])),
                ("text".to_owned(), DetailValue::Text("ogg".to_owned())),
            ]);
            let json =
                serde_json::to_string(&EvidenceWire::from_record(&evidence, &detail)).unwrap();
            let back: EvidenceWire = serde_json::from_str(&json).unwrap();
            assert_eq!(back.into_record(), Ok((evidence, detail)));
        }
    }

    #[test]
    fn layers_serialize_as_independent_statuses() {
        let summary = EvaluationSummary {
            deterministic: LayerStatus::Current(CheckOutcome::Passed),
            ai_review: LayerStatus::NotRun,
            human_review: LayerStatus::Stale,
        };
        assert_eq!(
            serde_json::to_string(&LayersWire::from(&summary)).unwrap(),
            "{\"deterministic\":{\"status\":\"current\",\"result\":\"passed\"},\
             \"ai_review\":{\"status\":\"not_run\"},\"human_review\":{\"status\":\"stale\"}}"
        );
    }
}
