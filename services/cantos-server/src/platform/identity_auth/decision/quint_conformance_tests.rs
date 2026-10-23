//! Event-only test observer of the real session-issue/admission decisions, not a reference reducer.
//!
//! Scope `pure-core-synthetic-shell`. Production symbols called here: `validate_fact`,
//! `SecurityState::from_response`, `decide_issue` and `decide_admission`. The Server shell is a
//! SYNTHETIC SHELL below and is labelled as such in every observation: the Studio roster, the
//! single session slot, a refused/absent Identity read, the requested-vs-committed revocation
//! and logout. It performs no HTTP, cookie, CSRF, SQL, RPC or clock read and never claims a
//! committed session. Expected states and outcomes never enter this file.
//!
//! Aliases: subject/epoch n -> sixteen bytes of n; version n -> n; operation 1/other 2;
//! fact age Fresh/Stale/Future -> 1 s old / one microsecond past the 30 s window / one past the
//! 5 s tolerated skew (exact second boundaries stay in the native decision tests).
use super::*;
use crate::platform::identity_auth::rpc::{
    validate_fact, AuthenticationFact, FactRejected, Purpose as RpcPurpose,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const OBSERVATION_SCHEMA: &str = "cantos-native-session-admission-observation-v1";
const NOW_MICROS: i64 = 1_000_000_000_000;
const FRESH_AGE_MICROS: i64 = 1_000_000;
const OPERATION: [u8; ID_BYTES] = [0x0a; ID_BYTES];
const OTHER_OPERATION: [u8; ID_BYTES] = [0x0b; ID_BYTES];
// The only registered subject; subject 2 is never present in Identity.
const SUBJECT: u8 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(try_from = "u8", into = "u8")]
struct Alias<const LOW: u8, const HIGH: u8>(u8);

impl<const LOW: u8, const HIGH: u8> TryFrom<u8> for Alias<LOW, HIGH> {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if (LOW..=HIGH).contains(&value) {
            Ok(Self(value))
        } else {
            Err(format!("alias {value} outside {LOW}..={HIGH}"))
        }
    }
}

impl<const LOW: u8, const HIGH: u8> From<Alias<LOW, HIGH>> for u8 {
    fn from(value: Alias<LOW, HIGH>) -> Self {
        value.0
    }
}

type SubjectClass = Alias<1, 2>;
type EpochClass = Alias<1, 2>;
type VersionClass = Alias<1, 8>;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
enum PurposeAlias {
    Theatre,
    Studio,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
enum AgeAlias {
    Fresh,
    Stale,
    Future,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
enum ReadAlias {
    Answer,
    Unreachable,
    Malformed,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct FactAlias {
    subject: SubjectClass,
    purpose: PurposeAlias,
    echoed: bool,
    age: AgeAlias,
    epoch: EpochClass,
    version: VersionClass,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LoginAlias {
    purpose: PurposeAlias,
    fact: FactAlias,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct IdentityAlias {
    read: ReadAlias,
    present: bool,
    active: bool,
    epoch: EpochClass,
    version: VersionClass,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "tag", content = "value", deny_unknown_fields)]
enum ReplayEvent {
    NoEvent(()),
    Assertion(()),
    Login(LoginAlias),
    Admit(()),
    CommitRevoke(bool),
    Logout(()),
    SetIdentity(IdentityAlias),
    SetRoster(bool),
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
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum Reason {
    FactMisbound,
    NotOnRoster,
    IdentityUnavailable,
    SubjectUnknown,
    FactNotFresh,
    Inactive,
    EpochMismatch,
    VersionMismatch,
    NoSession,
    SessionRevoked,
    SecurityChanged,
    NothingRequested,
    RevokeWriteFailed,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum NoOpKind {
    Admitted,
    AlreadyEnded,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum TransitionKind {
    Issued,
    Revoked,
    LoggedOut,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "tag", content = "value")]
enum ObservedOutcome {
    NoOutcome(()),
    Rejected(Reason),
    NoOp(NoOpKind),
    Transition(TransitionKind),
    External(()),
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum Origin {
    #[serde(rename = "none")]
    NoDecision,
    #[serde(rename = "core")]
    Core,
    #[serde(rename = "shell")]
    Shell,
    #[serde(rename = "core+shell")]
    CoreAndShell,
    #[serde(rename = "harness")]
    Harness,
}

/// The model's revoke reasons. `SubjectMismatch` has no model counterpart (the shell refuses
/// a reply for another subject as malformed), so reaching it is an adapter error.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum RevokeName {
    SubjectMissing,
    Disabled,
    EpochChanged,
    VersionAdvanced,
    VersionRegressed,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ObservedSecurityAlias {
    subject: u8,
    epoch: u8,
    version: u8,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ObservedSession {
    exists: bool,
    generation: u64,
    purpose: PurposeAlias,
    observed: ObservedSecurityAlias,
    revoked: bool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ObservedState {
    identity: IdentityAlias,
    rostered: bool,
    session: ObservedSession,
    pending: Option<RevokeName>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Calls {
    validate_fact: u64,
    security_state_from_response: u64,
    decide_issue: u64,
    decide_admission: u64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Evidence {
    decided_by: Origin,
    calls: Calls,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ObservedCheckpoint {
    index: u64,
    state: ObservedState,
    outcome: ObservedOutcome,
    evidence: Evidence,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ObservedTrace {
    id: String,
    checkpoints: Vec<ObservedCheckpoint>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ReplayOutput {
    schema_version: u64,
    observation_schema: &'static str,
    traces: Vec<ObservedTrace>,
}

/// Explicit test-only adapter defects, each a wrong fact handed to a real seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObserverFault {
    None,
    /// Every fact is stamped as fresh, whatever its modeled age class.
    FreshForEveryAge,
    /// The admission decision is asked about the CURRENT state instead of what the session saw.
    AdmitAgainstCurrent,
}

impl ObserverFault {
    fn from_environment() -> Result<Self, String> {
        match std::env::var("CANTOS_QUINT_OBSERVER_FAULT") {
            Err(std::env::VarError::NotPresent) => Ok(Self::None),
            Ok(value) if value == "fresh-for-every-age" => Ok(Self::FreshForEveryAge),
            Ok(value) if value == "admit-against-current" => Ok(Self::AdmitAgainstCurrent),
            _ => Err("unsupported CANTOS_QUINT_OBSERVER_FAULT".into()),
        }
    }
}

fn subject(alias: u8) -> Result<Subject, String> {
    Subject::from_bytes([alias; ID_BYTES]).ok_or_else(|| "subject alias is nil".to_string())
}

fn epoch(alias: u8) -> Result<Epoch, String> {
    Epoch::from_bytes([alias; ID_BYTES]).ok_or_else(|| "epoch alias is nil".to_string())
}

fn uniform(bytes: [u8; ID_BYTES], what: &str) -> Result<u8, String> {
    if bytes.iter().all(|byte| *byte == bytes[0]) {
        Ok(bytes[0])
    } else {
        Err(format!("{what} outside alias domain"))
    }
}

fn version_alias(version: u64) -> Result<u8, String> {
    u8::try_from(version)
        .ok()
        .filter(|alias| (1..=8).contains(alias))
        .ok_or_else(|| "version outside alias domain".to_string())
}

fn security_alias(observed: &ObservedSecurity) -> Result<ObservedSecurityAlias, String> {
    Ok(ObservedSecurityAlias {
        subject: uniform(observed.subject.bytes(), "subject")?,
        epoch: uniform(observed.epoch.bytes(), "epoch")?,
        version: version_alias(observed.version)?,
    })
}

fn rpc_purpose(purpose: PurposeAlias) -> RpcPurpose {
    match purpose {
        PurposeAlias::Theatre => RpcPurpose::Theatre,
        PurposeAlias::Studio => RpcPurpose::Studio,
    }
}

fn authentication_fact(fact: &FactAlias, fault: ObserverFault) -> AuthenticationFact {
    let authenticated_at = match (fault, fact.age) {
        (ObserverFault::FreshForEveryAge, _) | (_, AgeAlias::Fresh) => {
            NOW_MICROS - FRESH_AGE_MICROS
        }
        (_, AgeAlias::Stale) => NOW_MICROS - (FACT_MAX_AGE_MICROS + 1),
        (_, AgeAlias::Future) => NOW_MICROS + FACT_MAX_FUTURE_MICROS + 1,
    };
    AuthenticationFact {
        subject_id: vec![fact.subject.0; ID_BYTES],
        purpose: rpc_purpose(fact.purpose) as i32,
        assurance: crate::platform::identity_auth::rpc::Assurance::Password as i32,
        authenticated_at_unix_micros: authenticated_at,
        security_version: u64::from(fact.version.0),
        instance_epoch: vec![fact.epoch.0; ID_BYTES],
        operation_id: if fact.echoed {
            OPERATION.to_vec()
        } else {
            OTHER_OPERATION.to_vec()
        },
    }
}

fn security_response(
    asked: u8,
    identity: &IdentityAlias,
    malformed: bool,
) -> GetSecurityStateResponse {
    GetSecurityStateResponse {
        subject_id: vec![asked; ID_BYTES],
        state: if identity.active {
            AccountState::Active
        } else {
            AccountState::Disabled
        } as i32,
        // Version zero is outside the wire contract: a reply the shell must not trust.
        security_version: if malformed {
            0
        } else {
            u64::from(identity.version.0)
        },
        instance_epoch: vec![identity.epoch.0; ID_BYTES],
    }
}

type Decided = (ObservedOutcome, Origin);

fn rejected(reason: Reason, origin: Origin) -> Decided {
    (ObservedOutcome::Rejected(reason), origin)
}

enum Read {
    Unavailable(Origin),
    NotFound,
    State(SecurityState),
}

struct Slot {
    generation: u64,
    purpose: PurposeAlias,
    observed: ObservedSecurity,
    revoked: bool,
}

struct Observer {
    fault: ObserverFault,
    identity: IdentityAlias,
    rostered: bool,
    slot: Option<Slot>,
    generation: u64,
    pending: Option<RevokeName>,
    calls: Calls,
}

impl Observer {
    fn new(fault: ObserverFault) -> Self {
        Self {
            fault,
            identity: IdentityAlias {
                read: ReadAlias::Answer,
                present: true,
                active: true,
                epoch: Alias(1),
                version: Alias(1),
            },
            rostered: true,
            slot: None,
            generation: 0,
            pending: None,
            calls: Calls::default(),
        }
    }

    /// One fresh Identity read for `asked`. An outage is a shell fact, a malformed reply is the
    /// real decoder's refusal and an absent subject is the shell's NOT_FOUND.
    fn read(&mut self, asked: u8) -> Result<Read, String> {
        match self.identity.read {
            ReadAlias::Unreachable => Ok(Read::Unavailable(Origin::Shell)),
            ReadAlias::Malformed => {
                self.calls.security_state_from_response += 1;
                match SecurityState::from_response(
                    &security_response(asked, &self.identity, true),
                    subject(asked)?,
                ) {
                    Err(MalformedState) => Ok(Read::Unavailable(Origin::Core)),
                    Ok(_) => Err("a malformed reply was accepted".into()),
                }
            }
            ReadAlias::Answer if asked != SUBJECT || !self.identity.present => Ok(Read::NotFound),
            ReadAlias::Answer => {
                self.calls.security_state_from_response += 1;
                SecurityState::from_response(
                    &security_response(asked, &self.identity, false),
                    subject(asked)?,
                )
                .map(Read::State)
                .map_err(|MalformedState| "a well-formed reply was refused".to_string())
            }
        }
    }

    fn login(&mut self, event: LoginAlias) -> Result<Decided, String> {
        self.calls.validate_fact += 1;
        let fact = match validate_fact(
            &authentication_fact(&event.fact, self.fault),
            OPERATION,
            rpc_purpose(event.purpose),
        ) {
            Ok(fact) => fact,
            Err(FactRejected::WrongOperation | FactRejected::WrongPurpose) => {
                return Ok(rejected(Reason::FactMisbound, Origin::Core))
            }
            Err(other) => return Err(format!("fixture fact refused as {other:?}")),
        };
        let on_roster = event.fact.subject.0 == SUBJECT && self.rostered;
        if event.purpose == PurposeAlias::Studio && !on_roster {
            return Ok(rejected(Reason::NotOnRoster, Origin::CoreAndShell));
        }
        let current = match self.read(event.fact.subject.0)? {
            Read::Unavailable(origin) => return Ok(rejected(Reason::IdentityUnavailable, origin)),
            Read::NotFound => return Ok(rejected(Reason::SubjectUnknown, Origin::CoreAndShell)),
            Read::State(current) => current,
        };
        self.calls.decide_issue += 1;
        match decide_issue(&fact, &current, NOW_MICROS) {
            Err(refusal) => Ok(rejected(
                match refusal {
                    IssueRefusal::FactFromFuture | IssueRefusal::FactStale => Reason::FactNotFresh,
                    IssueRefusal::SubjectChanged => Reason::SubjectUnknown,
                    IssueRefusal::Inactive => Reason::Inactive,
                    IssueRefusal::EpochChanged => Reason::EpochMismatch,
                    IssueRefusal::VersionChanged => Reason::VersionMismatch,
                },
                Origin::Core,
            )),
            Ok(observed) => {
                self.generation += 1;
                self.pending = None;
                self.slot = Some(Slot {
                    generation: self.generation,
                    purpose: event.purpose,
                    observed,
                    revoked: false,
                });
                Ok((
                    ObservedOutcome::Transition(TransitionKind::Issued),
                    Origin::Core,
                ))
            }
        }
    }

    fn admit(&mut self) -> Result<Decided, String> {
        let Some(slot) = &self.slot else {
            return Ok(rejected(Reason::NoSession, Origin::Shell));
        };
        if slot.revoked {
            return Ok(rejected(Reason::SessionRevoked, Origin::Shell));
        }
        let observed = slot.observed;
        if slot.purpose == PurposeAlias::Studio && !self.rostered {
            return Ok(rejected(Reason::NotOnRoster, Origin::Shell));
        }
        let asked = uniform(observed.subject.bytes(), "subject")?;
        let current = match self.read(asked)? {
            Read::Unavailable(origin) => return Ok(rejected(Reason::IdentityUnavailable, origin)),
            Read::NotFound => {
                // `confirm` turns NOT_FOUND into a requested `subject-missing` revocation.
                self.pending = Some(RevokeName::SubjectMissing);
                return Ok(rejected(Reason::SecurityChanged, Origin::Shell));
            }
            Read::State(current) => current,
        };
        self.calls.decide_admission += 1;
        let compared = if self.fault == ObserverFault::AdmitAgainstCurrent {
            ObservedSecurity {
                subject: current.subject,
                epoch: current.epoch,
                version: current.version,
            }
        } else {
            observed
        };
        match decide_admission(&compared, &current) {
            Admission::Admit => Ok((ObservedOutcome::NoOp(NoOpKind::Admitted), Origin::Core)),
            Admission::Revoke(reason) => {
                self.pending = Some(match reason {
                    RevokeReason::Disabled => RevokeName::Disabled,
                    RevokeReason::EpochChanged => RevokeName::EpochChanged,
                    RevokeReason::VersionAdvanced => RevokeName::VersionAdvanced,
                    RevokeReason::VersionRegressed => RevokeName::VersionRegressed,
                    RevokeReason::SubjectMissing | RevokeReason::SubjectMismatch => {
                        return Err(format!("unmodeled production revoke reason {reason:?}"))
                    }
                });
                Ok(rejected(Reason::SecurityChanged, Origin::Core))
            }
        }
    }

    /// The durable write for an outstanding request; a conditional update changes nothing for
    /// an already-ended session.
    fn commit_revoke(&mut self, written: bool) -> Decided {
        if self.pending.take().is_none() {
            return rejected(Reason::NothingRequested, Origin::Shell);
        }
        match &mut self.slot {
            Some(slot) if slot.revoked => {
                (ObservedOutcome::NoOp(NoOpKind::AlreadyEnded), Origin::Shell)
            }
            Some(slot) if written => {
                slot.revoked = true;
                (
                    ObservedOutcome::Transition(TransitionKind::Revoked),
                    Origin::Shell,
                )
            }
            _ => rejected(Reason::RevokeWriteFailed, Origin::Shell),
        }
    }

    fn logout(&mut self) -> Decided {
        match &mut self.slot {
            Some(slot) if !slot.revoked => {
                slot.revoked = true;
                (
                    ObservedOutcome::Transition(TransitionKind::LoggedOut),
                    Origin::Shell,
                )
            }
            _ => (ObservedOutcome::NoOp(NoOpKind::AlreadyEnded), Origin::Shell),
        }
    }

    /// `shell` means no production seam ran in this checkpoint; `core+shell` means one ran and a
    /// synthetic shell rule decided; the label follows the call counters, never a claim.
    fn apply(&mut self, event: &ReplayEvent) -> Result<Decided, String> {
        let before = self.calls;
        let (outcome, origin) = self.dispatch(event)?;
        let consulted = self.calls != before;
        Ok((
            outcome,
            match origin {
                Origin::Shell if consulted => Origin::CoreAndShell,
                Origin::CoreAndShell if !consulted => Origin::Shell,
                other => other,
            },
        ))
    }

    fn dispatch(&mut self, event: &ReplayEvent) -> Result<Decided, String> {
        match event {
            ReplayEvent::NoEvent(()) | ReplayEvent::Assertion(()) => {
                Ok((ObservedOutcome::NoOutcome(()), Origin::NoDecision))
            }
            ReplayEvent::Login(login) => self.login(*login),
            ReplayEvent::Admit(()) => self.admit(),
            ReplayEvent::CommitRevoke(written) => Ok(self.commit_revoke(*written)),
            ReplayEvent::Logout(()) => Ok(self.logout()),
            ReplayEvent::SetIdentity(identity) => {
                self.identity = *identity;
                Ok((ObservedOutcome::External(()), Origin::Harness))
            }
            ReplayEvent::SetRoster(on) => {
                self.rostered = *on;
                Ok((ObservedOutcome::External(()), Origin::Harness))
            }
        }
    }

    fn observe(
        &self,
        index: u64,
        outcome: ObservedOutcome,
        decided_by: Origin,
    ) -> Result<ObservedCheckpoint, String> {
        let session = match &self.slot {
            Some(slot) => ObservedSession {
                exists: true,
                generation: slot.generation,
                purpose: slot.purpose,
                observed: security_alias(&slot.observed)?,
                revoked: slot.revoked,
            },
            None => ObservedSession {
                exists: false,
                generation: 0,
                purpose: PurposeAlias::Theatre,
                observed: ObservedSecurityAlias {
                    subject: 0,
                    epoch: 0,
                    version: 0,
                },
                revoked: false,
            },
        };
        Ok(ObservedCheckpoint {
            index,
            state: ObservedState {
                identity: self.identity,
                rostered: self.rostered,
                session,
                pending: self.pending,
            },
            outcome,
            evidence: Evidence {
                decided_by,
                calls: self.calls,
            },
        })
    }
}

fn replay(input: &ReplayInput, fault: ObserverFault) -> Result<ReplayOutput, String> {
    input.validate()?;
    let mut traces = Vec::with_capacity(input.traces.len());
    for trace in &input.traces {
        let mut observer = Observer::new(fault);
        let mut checkpoints = Vec::with_capacity(trace.checkpoints.len());
        for checkpoint in &trace.checkpoints {
            let (outcome, origin) = observer.apply(&checkpoint.event).map_err(|error| {
                format!(
                    "trace {} checkpoint {}: {error}",
                    trace.id, checkpoint.index
                )
            })?;
            checkpoints.push(observer.observe(checkpoint.index, outcome, origin)?);
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
fn replay_quint_native_session_admission() {
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

fn events(rest: Vec<ReplayEvent>) -> Vec<ReplayEvent> {
    let mut all = vec![ReplayEvent::NoEvent(())];
    all.extend(rest);
    all
}

fn alias<const LOW: u8, const HIGH: u8>(value: u8) -> Alias<LOW, HIGH> {
    Alias::try_from(value).unwrap()
}

fn fact(purpose: PurposeAlias, epoch: u8, version: u8) -> FactAlias {
    FactAlias {
        subject: alias(1),
        purpose,
        echoed: true,
        age: AgeAlias::Fresh,
        epoch: alias(epoch),
        version: alias(version),
    }
}

fn login(purpose: PurposeAlias, fact: FactAlias) -> ReplayEvent {
    ReplayEvent::Login(LoginAlias { purpose, fact })
}

fn identity(read: ReadAlias, present: bool, active: bool, epoch: u8, version: u8) -> ReplayEvent {
    ReplayEvent::SetIdentity(IdentityAlias {
        read,
        present,
        active,
        epoch: alias(epoch),
        version: alias(version),
    })
}

fn base() -> ReplayEvent {
    identity(ReadAlias::Answer, true, true, 1, 1)
}

#[test]
fn event_schema_rejects_unknown_fields_variants_and_out_of_domain_aliases() {
    let fact =
        r#"{"subject":1,"purpose":"Theatre","echoed":true,"age":"Fresh","epoch":1,"version":1}"#;
    for malformed in [
        r#"{"tag":"Admit","value":null,"outcome":"Admitted"}"#.to_string(),
        r#"{"tag":"Admit"}"#.to_string(),
        r#"{"tag":"Admit","value":true}"#.to_string(),
        r#"{"tag":"Refresh","value":null}"#.to_string(),
        r#"{"tag":"CommitRevoke","value":1}"#.to_string(),
        r#"{"tag":"SetRoster","value":null}"#.to_string(),
        r#"{"tag":"SetIdentity","value":{"read":"Down","present":true,"active":true,"epoch":1,"version":1}}"#.to_string(),
        r#"{"tag":"SetIdentity","value":{"read":"Answer","present":true,"active":true,"epoch":3,"version":1}}"#.to_string(),
        r#"{"tag":"SetIdentity","value":{"read":"Answer","present":true,"active":true,"epoch":1,"version":0}}"#.to_string(),
        r#"{"tag":"SetIdentity","value":{"read":"Answer","present":true,"active":true,"epoch":1,"version":1,"state":{}}}"#.to_string(),
        format!(r#"{{"tag":"Login","value":{{"purpose":"Admin","fact":{fact}}}}}"#),
        format!(r#"{{"tag":"Login","value":{{"purpose":"Theatre","fact":{}}}}}"#, fact.replace("Fresh", "Ancient")),
        format!(r#"{{"tag":"Login","value":{{"purpose":"Theatre","fact":{}}}}}"#, fact.replace("\"subject\":1", "\"subject\":3")),
        format!(r#"{{"tag":"Login","value":{{"purpose":"Theatre","fact":{},"result":"Issued"}}}}"#, fact),
    ] {
        assert!(
            serde_json::from_str::<ReplayEvent>(&malformed).is_err(),
            "{malformed}"
        );
    }
    let base = || serde_json::to_value(input(events(vec![]))).unwrap();
    let mut with_state = base();
    with_state["expectedState"] = serde_json::json!({});
    let mut with_outcome = base();
    with_outcome["traces"][0]["expectedOutcome"] = serde_json::json!({});
    let mut with_checkpoint_state = base();
    with_checkpoint_state["traces"][0]["checkpoints"][0]["state"] = serde_json::json!({});
    for value in [with_state, with_outcome, with_checkpoint_state] {
        assert!(serde_json::from_value::<ReplayInput>(value).is_err());
    }
}

#[test]
fn replay_rejects_missing_reordered_and_misidentified_event_checkpoints() {
    for events in [
        vec![],
        vec![ReplayEvent::Assertion(())],
        vec![ReplayEvent::NoEvent(()), ReplayEvent::NoEvent(())],
    ] {
        assert!(replay(&input(events), ObserverFault::None).is_err());
    }
    let mut value = input(events(vec![ReplayEvent::Assertion(())]));
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
fn assertion_checkpoints_never_repeat_a_production_call_or_effect() {
    let steps = events(vec![
        ReplayEvent::Assertion(()),
        login(PurposeAlias::Theatre, fact(PurposeAlias::Theatre, 1, 1)),
        ReplayEvent::Assertion(()),
        ReplayEvent::Admit(()),
        ReplayEvent::Assertion(()),
    ]);
    let output = replay(&input(steps), ObserverFault::None).unwrap();
    let checkpoints = &output.traces[0].checkpoints;
    for index in [1, 3, 5] {
        assert_eq!(checkpoints[index].outcome, ObservedOutcome::NoOutcome(()));
        assert_eq!(checkpoints[index].state, checkpoints[index - 1].state);
        assert_eq!(
            checkpoints[index].evidence.calls,
            checkpoints[index - 1].evidence.calls
        );
        assert_eq!(checkpoints[index].evidence.decided_by, Origin::NoDecision);
    }
    assert_eq!(
        checkpoints[2].outcome,
        ObservedOutcome::Transition(TransitionKind::Issued)
    );
    assert_eq!(
        checkpoints[4].outcome,
        ObservedOutcome::NoOp(NoOpKind::Admitted)
    );
    let calls = checkpoints.last().unwrap().evidence.calls;
    assert_eq!(
        (
            calls.validate_fact,
            calls.security_state_from_response,
            calls.decide_issue,
            calls.decide_admission
        ),
        (1, 2, 1, 1)
    );
}

#[test]
fn fact_age_classes_sit_one_microsecond_outside_the_production_window() {
    let at = |age: AgeAlias| {
        authentication_fact(
            &FactAlias {
                age,
                ..fact(PurposeAlias::Theatre, 1, 1)
            },
            ObserverFault::None,
        )
        .authenticated_at_unix_micros
    };
    assert_eq!(NOW_MICROS - at(AgeAlias::Stale), FACT_MAX_AGE_MICROS + 1);
    assert_eq!(
        at(AgeAlias::Future) - NOW_MICROS,
        FACT_MAX_FUTURE_MICROS + 1
    );
    let current = SecurityState {
        subject: subject(1).unwrap(),
        active: true,
        version: 1,
        epoch: epoch(1).unwrap(),
    };
    let verdict = |micros: i64| {
        let mut message =
            authentication_fact(&fact(PurposeAlias::Theatre, 1, 1), ObserverFault::None);
        message.authenticated_at_unix_micros = micros;
        let validated = validate_fact(&message, OPERATION, RpcPurpose::Theatre).unwrap();
        decide_issue(&validated, &current, NOW_MICROS)
    };
    // The class representatives are on the correct side of the exact production bounds.
    assert!(verdict(NOW_MICROS - FACT_MAX_AGE_MICROS).is_ok());
    assert_eq!(verdict(at(AgeAlias::Stale)), Err(IssueRefusal::FactStale));
    assert!(verdict(NOW_MICROS + FACT_MAX_FUTURE_MICROS).is_ok());
    assert_eq!(
        verdict(at(AgeAlias::Future)),
        Err(IssueRefusal::FactFromFuture)
    );
    assert!(verdict(at(AgeAlias::Fresh)).is_ok());
}

#[test]
fn shell_owned_decisions_are_labelled_and_real_seams_are_called() {
    let steps = events(vec![
        ReplayEvent::Admit(()),
        ReplayEvent::SetRoster(false),
        login(PurposeAlias::Studio, fact(PurposeAlias::Studio, 1, 1)),
        ReplayEvent::SetRoster(true),
        login(PurposeAlias::Theatre, fact(PurposeAlias::Theatre, 1, 1)),
        identity(ReadAlias::Unreachable, true, true, 1, 1),
        ReplayEvent::Admit(()),
        identity(ReadAlias::Malformed, true, true, 1, 1),
        ReplayEvent::Admit(()),
        identity(ReadAlias::Answer, false, true, 1, 1),
        ReplayEvent::Admit(()),
        ReplayEvent::CommitRevoke(true),
        ReplayEvent::Logout(()),
    ]);
    let output = replay(&input(steps), ObserverFault::None).unwrap();
    let checkpoints = &output.traces[0].checkpoints;
    let verdict = |index: usize| {
        (
            checkpoints[index].outcome,
            checkpoints[index].evidence.decided_by,
        )
    };
    assert_eq!(
        verdict(1),
        (ObservedOutcome::Rejected(Reason::NoSession), Origin::Shell)
    );
    assert_eq!(
        verdict(3),
        (
            ObservedOutcome::Rejected(Reason::NotOnRoster),
            Origin::CoreAndShell
        )
    );
    assert_eq!(
        verdict(7),
        (
            ObservedOutcome::Rejected(Reason::IdentityUnavailable),
            Origin::Shell
        )
    );
    assert_eq!(
        verdict(9),
        (
            ObservedOutcome::Rejected(Reason::IdentityUnavailable),
            Origin::Core
        )
    );
    assert_eq!(
        checkpoints[11].outcome,
        ObservedOutcome::Rejected(Reason::SecurityChanged)
    );
    assert_eq!(
        checkpoints[11].state.pending,
        Some(RevokeName::SubjectMissing)
    );
    assert!(!checkpoints[11].state.session.revoked);
    assert_eq!(
        verdict(12),
        (
            ObservedOutcome::Transition(TransitionKind::Revoked),
            Origin::Shell
        )
    );
    assert_eq!(
        checkpoints[13].outcome,
        ObservedOutcome::NoOp(NoOpKind::AlreadyEnded)
    );
}

#[test]
fn event_only_replay_is_deterministic_and_each_fault_changes_the_actual_observation() {
    let aged = input(events(vec![
        login(
            PurposeAlias::Theatre,
            FactAlias {
                age: AgeAlias::Stale,
                ..fact(PurposeAlias::Theatre, 1, 1)
            },
        ),
        ReplayEvent::Assertion(()),
    ]));
    let correct = replay(&aged, ObserverFault::None).unwrap();
    assert_eq!(correct, replay(&aged, ObserverFault::None).unwrap());
    assert_eq!(
        correct.traces[0].checkpoints[1].outcome,
        ObservedOutcome::Rejected(Reason::FactNotFresh)
    );
    let faulty = replay(&aged, ObserverFault::FreshForEveryAge).unwrap();
    assert_ne!(correct, faulty);
    assert_eq!(
        faulty.traces[0].checkpoints[1].outcome,
        ObservedOutcome::Transition(TransitionKind::Issued)
    );

    let advanced = input(events(vec![
        login(PurposeAlias::Theatre, fact(PurposeAlias::Theatre, 1, 1)),
        identity(ReadAlias::Answer, true, true, 1, 2),
        ReplayEvent::Admit(()),
    ]));
    let correct = replay(&advanced, ObserverFault::None).unwrap();
    assert_eq!(
        correct.traces[0].checkpoints[3].outcome,
        ObservedOutcome::Rejected(Reason::SecurityChanged)
    );
    assert_eq!(
        correct.traces[0].checkpoints[3].state.pending,
        Some(RevokeName::VersionAdvanced)
    );
    let faulty = replay(&advanced, ObserverFault::AdmitAgainstCurrent).unwrap();
    assert_eq!(
        faulty.traces[0].checkpoints[3].outcome,
        ObservedOutcome::NoOp(NoOpKind::Admitted)
    );
    assert_eq!(faulty.traces[0].checkpoints[3].state.pending, None);
    assert_eq!(
        faulty.traces[0].checkpoints[3].evidence.calls,
        correct.traces[0].checkpoints[3].evidence.calls
    );
}

#[test]
fn a_regressed_version_is_a_permanent_revocation_request_and_never_a_sticky_reset() {
    let steps = events(vec![
        identity(ReadAlias::Answer, true, true, 1, 2),
        login(PurposeAlias::Theatre, fact(PurposeAlias::Theatre, 1, 2)),
        base(),
        ReplayEvent::Admit(()),
        ReplayEvent::CommitRevoke(true),
        identity(ReadAlias::Answer, true, true, 1, 2),
        ReplayEvent::Admit(()),
    ]);
    let output = replay(&input(steps), ObserverFault::None).unwrap();
    let checkpoints = &output.traces[0].checkpoints;
    assert_eq!(
        checkpoints[4].state.pending,
        Some(RevokeName::VersionRegressed)
    );
    assert_eq!(
        checkpoints[5].outcome,
        ObservedOutcome::Transition(TransitionKind::Revoked)
    );
    assert_eq!(
        checkpoints[7].outcome,
        ObservedOutcome::Rejected(Reason::SessionRevoked)
    );
    assert_eq!(
        checkpoints[7].evidence.calls, checkpoints[6].evidence.calls,
        "a revoked session is refused before Identity is asked"
    );
}
