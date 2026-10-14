//! ReplayEvent-only test observer of the real private admission core, not a reference reducer.
//! Time aliases map n -> 2000 + 1000*n, preserving order/equality at both window bounds.
//! Synthetic verified metadata exercises constructors; no actual audio inspector runs here.
use super::{brief, request, OP};
use crate::contexts::catalog::domain::OwnerId;
use crate::contexts::media::domain::lifecycle::{
    decide_admission, submission_state, AdmissionDecision, AudioError, AudioRendition,
    AudioRenditionId, AudioSubmission,
};
use cantos_contracts as dto;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const OBSERVATION_SCHEMA: &str = "cantos-private-audio-admission-observation-v1";
const REAL_TIME_ORIGIN: i64 = 2000;
const REAL_TIME_SCALE: i64 = 1000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(try_from = "u8", into = "u8")]
struct BinaryAlias(u8);

impl TryFrom<u8> for BinaryAlias {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value <= 1 {
            Ok(Self(value))
        } else {
            Err("digest and length aliases must be 0 or 1")
        }
    }
}

impl From<BinaryAlias> for u8 {
    fn from(value: BinaryAlias) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RequestAliases {
    digest: BinaryAlias,
    length: BinaryAlias,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "tag", content = "value", deny_unknown_fields)]
enum ReplayEvent {
    NoEvent(()),
    Assertion(()),
    Begin(RequestAliases),
    Complete(bool),
    Abandon(()),
    Revoke(()),
    Invalidate(()),
    RestoreCurrent(()),
    Clock(i64),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReplayInput {
    schema_version: u64,
    observation_schema: String,
    traces: Vec<InputTrace>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InputTrace {
    id: String,
    checkpoints: Vec<InputCheckpoint>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InputCheckpoint {
    index: u64,
    event: ReplayEvent,
}

impl ReplayInput {
    fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.observation_schema != OBSERVATION_SCHEMA {
            return Err("unsupported replay schema or observation schema".into());
        }
        if self.traces.is_empty() {
            return Err("empty replay selection".into());
        }
        let mut ids = BTreeSet::new();
        for trace in &self.traces {
            if trace.id.trim().is_empty() || !ids.insert(&trace.id) {
                return Err("empty or duplicate trace identity".into());
            }
            if trace.checkpoints.is_empty() {
                return Err(format!("trace {} has no checkpoints", trace.id));
            }
            for (position, checkpoint) in trace.checkpoints.iter().enumerate() {
                if checkpoint.index != position as u64 {
                    return Err(format!("trace {} has noncontiguous checkpoints", trace.id));
                }
                if matches!(checkpoint.event, ReplayEvent::NoEvent(())) != (position == 0) {
                    return Err(format!(
                        "trace {} must start with its only NoEvent",
                        trace.id
                    ));
                }
                if let ReplayEvent::Clock(now) = checkpoint.event {
                    real_time(now)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ReplayOutput {
    schema_version: u64,
    observation_schema: &'static str,
    traces: Vec<ObservedTrace>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ObservedTrace {
    id: String,
    checkpoints: Vec<ObservedCheckpoint>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ObservedCheckpoint {
    index: u64,
    state: ObservedState,
    outcome: ObservedOutcome,
    submission: dto::AudioSubmissionDocument,
    rendition: Option<dto::AudioRenditionDocument>,
    decision_calls: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ObservedState {
    facts: ObservedFacts,
    receipt: Option<ObservedReceipt>,
    pending: Option<RequestAliases>,
    inspections: u64,
    admissions: u64,
    owner_view: &'static str,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ObservedFacts {
    now: i64,
    revoked: bool,
    current: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ObservedReceipt {
    id: u8,
    admitted_at: i64,
    digest: BinaryAlias,
    length: BinaryAlias,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "tag", content = "value")]
enum ObservedOutcome {
    NoOutcome(()),
    Inspect(()),
    Replay(()),
    Admitted(()),
    Rejected(String),
    MediaRejected(()),
    Cancelled(()),
    FactsChanged(()),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObserverFault {
    None,
    IgnoreRevocation,
}

impl ObserverFault {
    fn from_environment() -> Result<Self, String> {
        match std::env::var("CANTOS_QUINT_OBSERVER_FAULT") {
            Err(std::env::VarError::NotPresent) => Ok(Self::None),
            Ok(value) if value == "ignore-revocation" => Ok(Self::IgnoreRevocation),
            _ => Err("unsupported CANTOS_QUINT_OBSERVER_FAULT".into()),
        }
    }
}

#[derive(Clone, Debug)]
struct ActualRequest {
    digest: String,
    length: u64,
}

impl ActualRequest {
    fn from_aliases(aliases: RequestAliases) -> Self {
        Self {
            digest: match aliases.digest.0 {
                0 => "a".repeat(64),
                1 => "b".repeat(64),
                _ => unreachable!("BinaryAlias validates its domain"),
            },
            length: 100 + u64::from(aliases.length.0),
        }
    }

    fn aliases(&self) -> Result<RequestAliases, String> {
        Ok(RequestAliases {
            digest: digest_alias(&self.digest)?,
            length: length_alias(self.length)?,
        })
    }
}

fn real_time(alias: i64) -> Result<i64, String> {
    if !(0..=4).contains(&alias) {
        return Err("Clock alias must be in 0..=4".into());
    }
    Ok(REAL_TIME_ORIGIN + REAL_TIME_SCALE * alias)
}

fn time_alias(real: i64) -> Result<i64, String> {
    let delta = real
        .checked_sub(REAL_TIME_ORIGIN)
        .ok_or("actual time is outside alias domain")?;
    if delta % REAL_TIME_SCALE != 0 || !(0..=4).contains(&(delta / REAL_TIME_SCALE)) {
        return Err("actual time is outside alias domain".into());
    }
    Ok(delta / REAL_TIME_SCALE)
}

fn digest_alias(actual: &str) -> Result<BinaryAlias, String> {
    if actual == "a".repeat(64) {
        Ok(BinaryAlias(0))
    } else if actual == "b".repeat(64) {
        Ok(BinaryAlias(1))
    } else {
        Err("actual digest is outside alias domain".into())
    }
}

fn length_alias(actual: u64) -> Result<BinaryAlias, String> {
    match actual {
        100 => Ok(BinaryAlias(0)),
        101 => Ok(BinaryAlias(1)),
        _ => Err("actual length is outside alias domain".into()),
    }
}

struct Observer {
    submission: AudioSubmission,
    original_submission: dto::AudioSubmissionDocument,
    rendition: Option<AudioRendition>,
    original_rendition: Option<dto::AudioRenditionDocument>,
    pending: Option<ActualRequest>,
    now: i64,
    revoked: bool,
    current: bool,
    inspections: u64,
    admissions: u64,
    decision_calls: u64,
    fault: ObserverFault,
}

impl Observer {
    fn new(fault: ObserverFault) -> Result<Self, String> {
        let submission = AudioSubmission::create(
            &request(),
            &brief(),
            &OwnerId::parse("alice").map_err(|error| format!("{error:?}"))?,
            real_time(1)?,
        )
        .map_err(|error| format!("submission fixture: {error:?}"))?;
        if time_alias(submission.document().created_at)? != 1
            || time_alias(submission.document().expires_at)? != 3
            || digest_alias(&submission.document().expected_sha256)? != BinaryAlias(0)
            || length_alias(submission.document().expected_byte_length)? != BinaryAlias(0)
        {
            return Err("actual submission fixture changed its alias domain".into());
        }
        let original_submission = submission.document().clone();
        Ok(Self {
            now: submission.document().created_at,
            submission,
            original_submission,
            rendition: None,
            original_rendition: None,
            pending: None,
            revoked: false,
            current: true,
            inspections: 0,
            admissions: 0,
            decision_calls: 0,
            fault,
        })
    }

    fn decide(&mut self, request: &ActualRequest) -> Result<AdmissionDecision, AudioError> {
        self.decision_calls += 1;
        // Explicit test-only defect: pass an incorrect authority fact to the real seam.
        // The owner projection still observes the actual revocation fact.
        let revoked = self.revoked && self.fault != ObserverFault::IgnoreRevocation;
        decide_admission(
            &self.submission,
            self.now,
            revoked,
            self.current,
            &request.digest,
            request.length,
            self.rendition.as_ref(),
        )
    }

    fn apply(&mut self, event: &ReplayEvent) -> Result<ObservedOutcome, String> {
        match event {
            ReplayEvent::NoEvent(()) | ReplayEvent::Assertion(()) => {
                Ok(ObservedOutcome::NoOutcome(()))
            }
            ReplayEvent::Begin(aliases) => {
                if self.pending.is_some() {
                    return Err("Begin requires Idle".into());
                }
                let request = ActualRequest::from_aliases(*aliases);
                let decision = self.decide(&request);
                if decision == Ok(AdmissionDecision::Inspect) {
                    self.pending = Some(request);
                    self.inspections += 1;
                }
                Ok(decision_outcome(decision))
            }
            ReplayEvent::Complete(verified) => {
                let request = self
                    .pending
                    .take()
                    .ok_or("Complete requires pending inspection")?;
                let decision = self.decide(&request);
                // Even a failed external media check calls the real authority seam above.
                if !verified {
                    return Ok(ObservedOutcome::MediaRejected(()));
                }
                if decision != Ok(AdmissionDecision::Inspect) {
                    return Ok(decision_outcome(decision));
                }
                let rendition = AudioRendition::create(
                    &self.submission,
                    dto::AudioMediaMetadata {
                        sha256: request.digest,
                        byte_length: request.length,
                        container: "ogg".into(),
                        codec: "opus".into(),
                        channels: 1,
                        sample_rate_hz: 48000,
                        duration_ms: 1000,
                        audio_bitrate_bps: 24000,
                        profile_id: "opus-mono-24-v1".into(),
                    },
                    AudioRenditionId::parse(OP).map_err(|error| format!("{error:?}"))?,
                    self.now,
                )
                .map_err(|error| format!("authorized rendition construction: {error:?}"))?;
                if self.rendition.is_some() || self.original_rendition.is_some() {
                    return Err("inspection attempted to replace an immutable rendition".into());
                }
                self.original_rendition = Some(rendition.document().clone());
                self.rendition = Some(rendition);
                self.admissions += 1;
                Ok(ObservedOutcome::Admitted(()))
            }
            ReplayEvent::Abandon(()) => {
                self.pending
                    .take()
                    .ok_or("Abandon requires pending inspection")?;
                Ok(ObservedOutcome::Cancelled(()))
            }
            ReplayEvent::Revoke(()) => {
                self.revoked = true;
                Ok(ObservedOutcome::FactsChanged(()))
            }
            ReplayEvent::Invalidate(()) => {
                self.current = false;
                Ok(ObservedOutcome::FactsChanged(()))
            }
            ReplayEvent::RestoreCurrent(()) => {
                self.current = true;
                Ok(ObservedOutcome::FactsChanged(()))
            }
            ReplayEvent::Clock(now) => {
                self.now = real_time(*now)?;
                Ok(ObservedOutcome::FactsChanged(()))
            }
        }
    }

    fn observe(&self, index: u64, outcome: ObservedOutcome) -> Result<ObservedCheckpoint, String> {
        let submission = self.submission.document().clone();
        let rendition = self.rendition.as_ref().map(|r| r.document().clone());
        if submission != self.original_submission || rendition != self.original_rendition {
            return Err("actual immutable submission or rendition changed".into());
        }
        let receipt = rendition
            .as_ref()
            .map(|document| -> Result<ObservedReceipt, String> {
                if document.id != OP {
                    return Err("actual rendition identity is outside alias domain".into());
                }
                Ok(ObservedReceipt {
                    id: 1,
                    admitted_at: time_alias(document.admitted_at)?,
                    digest: digest_alias(&document.verified_media.sha256)?,
                    length: length_alias(document.verified_media.byte_length)?,
                })
            })
            .transpose()?;
        let owner_view = match submission_state(
            self.now,
            submission.expires_at,
            self.revoked,
            self.current,
            rendition.is_some(),
        ) {
            dto::AudioSubmissionState::AwaitingBytes => "AwaitingBytes",
            dto::AudioSubmissionState::Admitted => "OwnerAdmitted",
            dto::AudioSubmissionState::Expired => "Expired",
            dto::AudioSubmissionState::Revoked => "Revoked",
            dto::AudioSubmissionState::Stale => "Stale",
        };
        Ok(ObservedCheckpoint {
            index,
            state: ObservedState {
                facts: ObservedFacts {
                    now: time_alias(self.now)?,
                    revoked: self.revoked,
                    current: self.current,
                },
                receipt,
                pending: self
                    .pending
                    .as_ref()
                    .map(ActualRequest::aliases)
                    .transpose()?,
                inspections: self.inspections,
                admissions: self.admissions,
                owner_view,
            },
            outcome,
            submission,
            rendition,
            decision_calls: self.decision_calls,
        })
    }
}

fn decision_outcome(decision: Result<AdmissionDecision, AudioError>) -> ObservedOutcome {
    match decision {
        Ok(AdmissionDecision::Inspect) => ObservedOutcome::Inspect(()),
        Ok(AdmissionDecision::Replay) => ObservedOutcome::Replay(()),
        Err(error) => ObservedOutcome::Rejected(format!("{error:?}")),
    }
}

fn replay(input: &ReplayInput, fault: ObserverFault) -> Result<ReplayOutput, String> {
    input.validate()?;
    let mut traces = Vec::with_capacity(input.traces.len());
    for trace in &input.traces {
        let mut observer = Observer::new(fault)?;
        let mut checkpoints = Vec::with_capacity(trace.checkpoints.len());
        for checkpoint in &trace.checkpoints {
            let outcome = observer.apply(&checkpoint.event).map_err(|error| {
                format!(
                    "trace {} checkpoint {}: {error}",
                    trace.id, checkpoint.index
                )
            })?;
            checkpoints.push(observer.observe(checkpoint.index, outcome)?);
        }
        traces.push(ObservedTrace {
            id: trace.id.clone(),
            checkpoints,
        });
    }
    Ok(ReplayOutput {
        schema_version: 1,
        observation_schema: OBSERVATION_SCHEMA,
        traces,
    })
}

#[test]
#[ignore = "event-only observer is invoked by the explicit Quint conformance gate"]
fn replay_quint_private_admission() {
    let input_path = std::env::var_os("CANTOS_QUINT_REPLAY_INPUT")
        .expect("CANTOS_QUINT_REPLAY_INPUT is required");
    let output_path = std::env::var_os("CANTOS_QUINT_REPLAY_OUTPUT")
        .expect("CANTOS_QUINT_REPLAY_OUTPUT is required");
    let bytes = std::fs::read(input_path).expect("read event-only replay input");
    let input: ReplayInput =
        serde_json::from_slice(&bytes).expect("strict event-only replay schema");
    let fault = ObserverFault::from_environment().expect("supported test-only observer fault");
    let output = replay(&input, fault).expect("valid replay events and actual observations");
    let bytes = serde_json::to_vec_pretty(&output).expect("serialize actual observations");
    std::fs::write(output_path, bytes).expect("write actual observations");
}

fn input(events: Vec<ReplayEvent>) -> ReplayInput {
    ReplayInput {
        schema_version: 1,
        observation_schema: OBSERVATION_SCHEMA.into(),
        traces: vec![InputTrace {
            id: "observer-native-regression".into(),
            checkpoints: events
                .into_iter()
                .enumerate()
                .map(|(index, event)| InputCheckpoint {
                    index: index as u64,
                    event,
                })
                .collect(),
        }],
    }
}

fn exact() -> ReplayEvent {
    ReplayEvent::Begin(RequestAliases {
        digest: BinaryAlias(0),
        length: BinaryAlias(0),
    })
}

#[test]
fn event_schema_rejects_unknown_fields_variants_and_invalid_aliases() {
    for malformed in [
        r#"{"tag":"Assertion","value":null,"outcome":"Admitted"}"#,
        r#"{"tag":"Assertion"}"#,
        r#"{"tag":"Assertion","value":true}"#,
        r#"{"tag":"Unknown","value":null}"#,
        r#"{"tag":"Begin","value":{"digest":0,"length":0,"state":{}}}"#,
        r#"{"tag":"Begin","value":{"digest":2,"length":0}}"#,
        r#"{"tag":"Begin","value":{"digest":0,"length":-1}}"#,
        r#"{"tag":"Begin","value":{"digest":true,"length":0}}"#,
        r#"{"tag":"Complete","value":0}"#,
        r#"{"tag":"Clock","value":1.5}"#,
    ] {
        assert!(
            serde_json::from_str::<ReplayEvent>(malformed).is_err(),
            "{malformed}"
        );
    }
    let mut raw = serde_json::to_value(input(vec![ReplayEvent::NoEvent(())])).unwrap();
    raw["expectedState"] = serde_json::json!({});
    assert!(serde_json::from_value::<ReplayInput>(raw).is_err());
    let mut raw = serde_json::to_value(input(vec![ReplayEvent::NoEvent(())])).unwrap();
    raw["traces"][0]["expectedOutcome"] = serde_json::json!({});
    assert!(serde_json::from_value::<ReplayInput>(raw).is_err());
    let mut raw = serde_json::to_value(input(vec![ReplayEvent::NoEvent(())])).unwrap();
    raw["traces"][0]["checkpoints"][0]["state"] = serde_json::json!({});
    assert!(serde_json::from_value::<ReplayInput>(raw).is_err());
}

#[test]
fn replay_rejects_missing_reordered_and_illegal_event_checkpoints() {
    for events in [
        vec![],
        vec![ReplayEvent::Assertion(())],
        vec![ReplayEvent::NoEvent(()), ReplayEvent::NoEvent(())],
        vec![ReplayEvent::NoEvent(()), ReplayEvent::Clock(-1)],
        vec![ReplayEvent::NoEvent(()), ReplayEvent::Clock(5)],
        vec![ReplayEvent::NoEvent(()), ReplayEvent::Complete(true)],
        vec![ReplayEvent::NoEvent(()), ReplayEvent::Abandon(())],
        vec![ReplayEvent::NoEvent(()), exact(), exact()],
    ] {
        assert!(replay(&input(events), ObserverFault::None).is_err());
    }
    let mut value = input(vec![ReplayEvent::NoEvent(()), ReplayEvent::Assertion(())]);
    value.traces[0].checkpoints[1].index = 2;
    assert!(replay(&value, ObserverFault::None).is_err());
    value.traces[0].checkpoints[1].index = 1;
    value.schema_version = 2;
    assert!(replay(&value, ObserverFault::None).is_err());
    value.schema_version = 1;
    value.observation_schema.push_str("-unsupported");
    assert!(replay(&value, ObserverFault::None).is_err());
    value.observation_schema = OBSERVATION_SCHEMA.into();
    value.traces.clear();
    assert!(replay(&value, ObserverFault::None).is_err());
}

#[test]
fn assertion_checkpoints_do_not_repeat_decisions_or_immutable_effects() {
    let output = replay(
        &input(vec![
            ReplayEvent::NoEvent(()),
            ReplayEvent::Assertion(()),
            exact(),
            ReplayEvent::Assertion(()),
            ReplayEvent::Clock(2),
            ReplayEvent::Assertion(()),
            ReplayEvent::Complete(true),
            ReplayEvent::Assertion(()),
            exact(),
            ReplayEvent::Assertion(()),
            ReplayEvent::Clock(3),
            ReplayEvent::Assertion(()),
        ]),
        ObserverFault::None,
    )
    .unwrap();
    let checkpoints = &output.traces[0].checkpoints;
    for pair in checkpoints.chunks_exact(2) {
        assert_eq!(pair[1].outcome, ObservedOutcome::NoOutcome(()));
        assert_eq!(pair[0].state, pair[1].state);
        assert_eq!(pair[0].submission, pair[1].submission);
        assert_eq!(pair[0].rendition, pair[1].rendition);
        assert_eq!(pair[0].decision_calls, pair[1].decision_calls);
    }
    assert_eq!(checkpoints[6].outcome, ObservedOutcome::Admitted(()));
    assert_eq!(checkpoints[8].outcome, ObservedOutcome::Replay(()));
    let last = checkpoints.last().unwrap();
    assert_eq!(last.decision_calls, 3);
    assert_eq!((last.state.inspections, last.state.admissions), (1, 1));
    assert_eq!(last.state.owner_view, "OwnerAdmitted");
    let receipt = last.state.receipt.as_ref().unwrap();
    assert_eq!(receipt.admitted_at, 2);
    assert_eq!(last.rendition.as_ref().unwrap().admitted_at, 4000);
    assert_eq!(last.submission.created_at, 3000);
    assert_eq!(last.submission.expires_at, 5000);
    assert_eq!(last.submission, checkpoints[0].submission);
    assert_eq!(last.rendition, checkpoints[6].rendition);
}

#[test]
fn failed_media_completion_still_rechecks_real_authority_without_admission() {
    let output = replay(
        &input(vec![
            ReplayEvent::NoEvent(()),
            exact(),
            ReplayEvent::Revoke(()),
            ReplayEvent::Complete(false),
            ReplayEvent::Invalidate(()),
            ReplayEvent::RestoreCurrent(()),
            exact(),
        ]),
        ObserverFault::None,
    )
    .unwrap();
    let checkpoints = &output.traces[0].checkpoints;
    assert_eq!(checkpoints[3].outcome, ObservedOutcome::MediaRejected(()));
    assert_eq!(checkpoints[3].decision_calls, 2);
    assert_eq!(checkpoints[3].state.pending, None);
    assert_eq!(checkpoints[3].rendition, None);
    assert_eq!(checkpoints[3].state.admissions, 0);
    assert_eq!(checkpoints[4].state.owner_view, "Stale");
    assert_eq!(checkpoints[5].state.owner_view, "Revoked");
    assert_eq!(
        checkpoints[6].outcome,
        ObservedOutcome::Rejected("PermissionRevoked".into())
    );
}

#[test]
fn event_only_replay_is_deterministic_and_fault_changes_actual_observation() {
    let input = input(vec![
        ReplayEvent::NoEvent(()),
        exact(),
        ReplayEvent::Revoke(()),
        ReplayEvent::Complete(true),
        ReplayEvent::Assertion(()),
    ]);
    let correct = replay(&input, ObserverFault::None).unwrap();
    let repeated = replay(&input, ObserverFault::None).unwrap();
    assert_eq!(correct, repeated);
    assert_eq!(
        serde_json::to_vec(&correct).unwrap(),
        serde_json::to_vec(&repeated).unwrap()
    );
    let faulty = replay(&input, ObserverFault::IgnoreRevocation).unwrap();
    assert_ne!(correct, faulty);
    let correct = &correct.traces[0].checkpoints[3];
    let faulty = &faulty.traces[0].checkpoints[3];
    assert_eq!(
        correct.outcome,
        ObservedOutcome::Rejected("PermissionRevoked".into())
    );
    assert_eq!(faulty.outcome, ObservedOutcome::Admitted(()));
    assert_eq!(correct.decision_calls, faulty.decision_calls);
    assert_eq!(correct.state.owner_view, faulty.state.owner_view);
    assert_eq!(correct.submission, faulty.submission);
    assert_eq!(correct.rendition, None);
    assert!(faulty.rendition.is_some());
}
