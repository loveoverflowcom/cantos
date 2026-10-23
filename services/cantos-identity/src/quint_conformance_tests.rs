//! Event-only test observer of the real mailbox-proof core, not a reference reducer.
//!
//! Scope `pure-core-synthetic-shell`. Production symbols called here: `ProofId::from_bytes`,
//! `ProofTime::new`, `ProofBinding::new`, `MailboxProof::{issue, plan_confirmation,
//! plan_cancellation, plan_replacement}` and the `ProofTransition` accessors. Everything a
//! store would own is a small SYNTHETIC SHELL below and is labelled as such in every
//! observation: the epoch/receipt/secret/locator-uniqueness checks, the held plan, the
//! compare-and-replace and the commit fate. It performs no SQL, entropy, digest, mail or RPC
//! and never claims a real commit. Expected states and outcomes never enter this file.
//!
//! Aliases: locator/command/operation/epoch n -> nonzero 128-bit big-endian value n;
//! clock class n -> Unix second 1000 + n (order, equality and one-second cooldown kept).
use crate::domain::{
    admission::Purpose,
    ids::{InstanceEpoch, OperationId, SubjectId},
    mailbox_proof::{
        MailboxProof, ProofAction, ProofBinding, ProofDenied, ProofId, ProofIntent, ProofState,
        ProofTime, ProofTransition,
    },
    security_version::SecurityVersion,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const OBSERVATION_SCHEMA: &str = "cantos-native-mailbox-proof-observation-v1";
const TIME_ORIGIN: u64 = 1000;
const MAX_TIME_ALIAS: u64 = 16;
// Policy inputs of the model, passed to the real replacement planner.
const STORE_EPOCH: u8 = 1;
const OPERATION: u8 = 1;
const TTL_SECONDS: u64 = 2;
const COOLDOWN_SECONDS: u32 = 1;
// Recovery intents are only wrong-binding inputs; they carry one fixed subject/version.
const RECOVERY_SUBJECT: u128 = 1;
const RECOVERY_VERSION: u64 = 1;

/// A model integer class with a closed domain, rejected at decode time when outside it.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
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

type Command = Alias<1, 2>;
type Operation = Alias<1, 2>;
type Epoch = Alias<1, 2>;
type Locator = Alias<1, 3>;
type SecretClass = Alias<0, 3>;
type Clock = Alias<0, 5>;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
enum IntentAlias {
    TheatreSignup,
    TheatreRecovery,
    StudioRecovery,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
enum Fate {
    Commit,
    Reject,
    UnknownApplied,
    UnknownLost,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestAlias {
    operation: Operation,
    epoch: Epoch,
    intent: IntentAlias,
    loc: Locator,
    secret: SecretClass,
    explicit: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PrepareAlias {
    command: Command,
    request: RequestAlias,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ReplaceAlias {
    loc: Locator,
    fresh: Locator,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "tag", content = "value", deny_unknown_fields)]
enum ReplayEvent {
    NoEvent(()),
    Assertion(()),
    Inspect(()),
    Prepare(PrepareAlias),
    Settle(Fate),
    Cancel(Locator),
    Replace(ReplaceAlias),
    Clock(Clock),
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
    EpochChanged,
    OperationReused,
    NotExplicit,
    UnknownOperation,
    BindingChanged,
    ProofChanged,
    InvalidSecret,
    AlreadyConsumed,
    AlreadyCancelled,
    BeforeIssue,
    Expired,
    NothingPlanned,
    StaleSnapshot,
    CommitRejected,
    ReusedLocator,
    LocatorNotUnique,
    Cooldown,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum NoOpKind {
    Inspected,
    PlanPrepared,
    Replayed,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
enum TransitionKind {
    ConsumedWithAccount,
    CancelledProof,
    ReplacedProof,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "tag", content = "value")]
enum ObservedOutcome {
    NoOutcome(()),
    Rejected(Reason),
    NoOp(NoOpKind),
    Transition(TransitionKind),
    UnknownEffect(()),
    External(()),
}

/// Which side produced this checkpoint's outcome. Reported so no report can read a synthetic
/// shell decision as production evidence.
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

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ObservedProof {
    loc: u8,
    state: &'static str,
    issued_at: u8,
    expires_at: u8,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ObservedReceipt {
    command: u8,
    request: RequestAlias,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ObservedDurable {
    proof: ObservedProof,
    issued: Vec<u8>,
    receipts: Vec<ObservedReceipt>,
    accounts: u64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ObservedPlan {
    command: u8,
    request: RequestAlias,
    expected: ObservedProof,
    next: ObservedProof,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ObservedState {
    durable: ObservedDurable,
    now: u8,
    plan: Option<ObservedPlan>,
    // The pure core exposes no authority value at all. These are harness constants, compared
    // only so the model's scope declaration is not silently dropped.
    session: bool,
    studio: bool,
    admin: bool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct ObservedBinding {
    operation: u8,
    epoch: u8,
    intent: IntentAlias,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct PlanBinding {
    expected: ObservedBinding,
    next: ObservedBinding,
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Calls {
    plan_confirmation: u64,
    plan_cancellation: u64,
    plan_replacement: u64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Evidence {
    decided_by: Origin,
    calls: Calls,
    binding: ObservedBinding,
    plan_binding: Option<PlanBinding>,
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

/// Explicit test-only adapter defects. Each passes a wrong fact to, or skips, a real seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObserverFault {
    None,
    /// A client `explicit=false` flag is presented to the core as an explicit confirmation.
    InspectAsExplicit,
    /// Commit applies the held plan without asking the real planner at the commit instant.
    SkipCommitRecheck,
}

impl ObserverFault {
    fn from_environment() -> Result<Self, String> {
        match std::env::var("CANTOS_QUINT_OBSERVER_FAULT") {
            Err(std::env::VarError::NotPresent) => Ok(Self::None),
            Ok(value) if value == "inspect-as-explicit" => Ok(Self::InspectAsExplicit),
            Ok(value) if value == "skip-commit-recheck" => Ok(Self::SkipCommitRecheck),
            _ => Err("unsupported CANTOS_QUINT_OBSERVER_FAULT".into()),
        }
    }
}

fn locator(alias: u8) -> Result<ProofId, String> {
    ProofId::from_bytes(u128::from(alias).to_be_bytes()).map_err(|e| format!("locator: {e}"))
}

fn proof_time(alias: u8) -> Result<ProofTime, String> {
    ProofTime::new(TIME_ORIGIN + u64::from(alias)).map_err(|e| format!("time: {e}"))
}

fn presented_binding(
    operation: u8,
    epoch: u8,
    intent: IntentAlias,
) -> Result<ProofBinding, String> {
    let intent = match intent {
        IntentAlias::TheatreSignup => ProofIntent::TheatreSignup,
        IntentAlias::TheatreRecovery | IntentAlias::StudioRecovery => {
            ProofIntent::CredentialRecovery {
                subject: SubjectId::from_bytes(RECOVERY_SUBJECT.to_be_bytes())
                    .map_err(|e| format!("recovery subject: {e}"))?,
                version: SecurityVersion::new(RECOVERY_VERSION)
                    .map_err(|e| format!("recovery version: {e}"))?,
                purpose: if intent == IntentAlias::StudioRecovery {
                    Purpose::Studio
                } else {
                    Purpose::Theatre
                },
            }
        }
    };
    Ok(ProofBinding::new(
        OperationId::from_bytes(u128::from(operation).to_be_bytes())
            .map_err(|e| format!("operation: {e}"))?,
        InstanceEpoch::from_bytes(u128::from(epoch).to_be_bytes())
            .map_err(|e| format!("epoch: {e}"))?,
        intent,
    ))
}

fn small(bytes: [u8; 16], what: &str) -> Result<u8, String> {
    u8::try_from(u128::from_be_bytes(bytes)).map_err(|_| format!("{what} outside alias domain"))
}

fn binding_alias(binding: ProofBinding) -> Result<ObservedBinding, String> {
    let intent = match binding.intent() {
        ProofIntent::TheatreSignup => IntentAlias::TheatreSignup,
        ProofIntent::CredentialRecovery {
            subject,
            version,
            purpose,
        } => {
            if u128::from_be_bytes(subject.to_bytes()) != RECOVERY_SUBJECT
                || version.get() != RECOVERY_VERSION
            {
                return Err("recovery binding outside alias domain".into());
            }
            match purpose {
                Purpose::Theatre => IntentAlias::TheatreRecovery,
                Purpose::Studio => IntentAlias::StudioRecovery,
            }
        }
    };
    Ok(ObservedBinding {
        operation: small(binding.operation().to_bytes(), "operation")?,
        epoch: small(binding.epoch().to_bytes(), "epoch")?,
        intent,
    })
}

fn second_alias(time: ProofTime) -> Result<u8, String> {
    let delta = time
        .seconds()
        .checked_sub(TIME_ORIGIN)
        .filter(|delta| *delta <= MAX_TIME_ALIAS)
        .ok_or("actual time outside alias domain")?;
    u8::try_from(delta).map_err(|e| e.to_string())
}

fn proof_alias(proof: MailboxProof) -> Result<ObservedProof, String> {
    Ok(ObservedProof {
        loc: small(proof.id().to_bytes(), "locator")?,
        state: match proof.state() {
            ProofState::Pending => "Pending",
            ProofState::Consumed => "Consumed",
            ProofState::Cancelled => "Cancelled",
        },
        issued_at: second_alias(proof.issued_at())?,
        expires_at: second_alias(proof.expires_at())?,
    })
}

fn reason(denied: ProofDenied) -> Result<Reason, String> {
    Ok(match denied {
        ProofDenied::NotExplicitConfirmation => Reason::NotExplicit,
        ProofDenied::BindingChanged => Reason::BindingChanged,
        ProofDenied::ProofChanged => Reason::ProofChanged,
        ProofDenied::BeforeIssue => Reason::BeforeIssue,
        ProofDenied::Expired => Reason::Expired,
        ProofDenied::Consumed => Reason::AlreadyConsumed,
        ProofDenied::Cancelled => Reason::AlreadyCancelled,
        ProofDenied::ReusedProofId => Reason::ReusedLocator,
        ProofDenied::Cooldown => Reason::Cooldown,
        // The model has no outcome for these; reaching one is an adapter/fixture error.
        ProofDenied::InvalidWindow | ProofDenied::InvalidCooldown => {
            return Err(format!("unmodeled production denial {denied:?}"))
        }
    })
}

type Decided = (ObservedOutcome, Origin);

fn rejected(reason: Reason, origin: Origin) -> Decided {
    (ObservedOutcome::Rejected(reason), origin)
}

/// The shell's held plan: the real planner's value plus the inputs it was planned from.
struct HeldPlan {
    command: u8,
    request: RequestAlias,
    matched: ProofId,
    presented: ProofBinding,
    transition: ProofTransition,
}

struct Observer {
    fault: ObserverFault,
    initial_binding: ProofBinding,
    proof: MailboxProof,
    issued: BTreeSet<u8>,
    receipts: BTreeMap<u8, RequestAlias>,
    accounts: u64,
    now: u8,
    plan: Option<HeldPlan>,
    calls: Calls,
}

impl Observer {
    fn new(fault: ObserverFault) -> Result<Self, String> {
        let binding = presented_binding(OPERATION, STORE_EPOCH, IntentAlias::TheatreSignup)?;
        let proof = MailboxProof::issue(locator(1)?, binding, proof_time(1)?, proof_time(3)?)
            .map_err(|e| format!("initial proof fixture: {e}"))?;
        Ok(Self {
            fault,
            initial_binding: binding,
            proof,
            issued: BTreeSet::from([1]),
            receipts: BTreeMap::new(),
            accounts: 0,
            now: 1,
            plan: None,
            calls: Calls::default(),
        })
    }

    fn confirm(
        &mut self,
        matched: ProofId,
        presented: ProofBinding,
        action: ProofAction,
    ) -> Result<ProofTransition, ProofDenied> {
        self.calls.plan_confirmation += 1;
        self.proof
            .plan_confirmation(matched, presented, action, proof_time(self.now).unwrap())
    }

    /// Compare-and-replace in the synthetic shell: only a plan made from the current snapshot.
    fn apply(&mut self, transition: ProofTransition) -> Result<(), String> {
        if transition.expected() != self.proof {
            return Err("synthetic shell was handed a plan for another snapshot".into());
        }
        self.proof = transition.next();
        Ok(())
    }

    fn prepare(&mut self, command: Command, request: RequestAlias) -> Result<Decided, String> {
        if request.epoch.0 != STORE_EPOCH {
            return Ok(rejected(Reason::EpochChanged, Origin::Shell));
        }
        if let Some(previous) = self.receipts.get(&command.0) {
            return Ok(if *previous == request {
                (ObservedOutcome::NoOp(NoOpKind::Replayed), Origin::Shell)
            } else {
                rejected(Reason::OperationReused, Origin::Shell)
            });
        }
        let presented = presented_binding(request.operation.0, request.epoch.0, request.intent)?;
        let matched = locator(request.loc.0)?;
        // The store refuses a non-explicit action before it looks anything else up; the real
        // core owns the actual refusal.
        if !request.explicit && self.fault != ObserverFault::InspectAsExplicit {
            return match self.confirm(matched, presented, ProofAction::Inspect) {
                Err(ProofDenied::NotExplicitConfirmation) => {
                    Ok(rejected(Reason::NotExplicit, Origin::Core))
                }
                other => Err(format!(
                    "core did not refuse a non-explicit action: {other:?}"
                )),
            };
        }
        if request.operation.0 != OPERATION {
            // The store finds no workflow for another operation; the core is never reached.
            return Ok(rejected(Reason::UnknownOperation, Origin::Shell));
        }
        let planned = self.confirm(matched, presented, ProofAction::ExplicitConfirmation);
        if let Err(denied @ (ProofDenied::BindingChanged | ProofDenied::ProofChanged)) = planned {
            return Ok(rejected(reason(denied)?, Origin::Core));
        }
        // The stored digest is compared after the binding/locator checks, before the planner's
        // state and window checks. The model's secret class is the locator it was minted for.
        if request.secret.0 != request.loc.0 {
            return Ok(rejected(Reason::InvalidSecret, Origin::CoreAndShell));
        }
        match planned {
            Err(denied) => Ok(rejected(reason(denied)?, Origin::Core)),
            Ok(transition) => {
                self.plan = Some(HeldPlan {
                    command: command.0,
                    request,
                    matched,
                    presented,
                    transition,
                });
                Ok((ObservedOutcome::NoOp(NoOpKind::PlanPrepared), Origin::Core))
            }
        }
    }

    fn settle(&mut self, fate: Fate) -> Result<Decided, String> {
        let Some(plan) = self.plan.take() else {
            return Ok(rejected(Reason::NothingPlanned, Origin::Shell));
        };
        if plan.transition.expected() != self.proof {
            return Ok(rejected(Reason::StaleSnapshot, Origin::Shell));
        }
        // Plan and commit share one owner transaction, so the window is the commit instant's.
        let transition = if self.fault == ObserverFault::SkipCommitRecheck {
            plan.transition
        } else {
            match self.confirm(
                plan.matched,
                plan.presented,
                ProofAction::ExplicitConfirmation,
            ) {
                Ok(transition) => transition,
                Err(denied @ (ProofDenied::BeforeIssue | ProofDenied::Expired)) => {
                    return Ok(rejected(reason(denied)?, Origin::Core))
                }
                Err(other) => {
                    return Err(format!(
                        "commit recheck changed class on one snapshot: {other:?}"
                    ))
                }
            }
        };
        match fate {
            Fate::Commit | Fate::UnknownApplied => {
                self.apply(transition)?;
                self.receipts.insert(plan.command, plan.request);
                self.accounts += 1;
                let outcome = if fate == Fate::Commit {
                    ObservedOutcome::Transition(TransitionKind::ConsumedWithAccount)
                } else {
                    ObservedOutcome::UnknownEffect(())
                };
                Ok((outcome, Origin::CoreAndShell))
            }
            Fate::UnknownLost => Ok((ObservedOutcome::UnknownEffect(()), Origin::CoreAndShell)),
            Fate::Reject => Ok(rejected(Reason::CommitRejected, Origin::CoreAndShell)),
        }
    }

    fn cancel(&mut self, loc: Locator) -> Result<Decided, String> {
        self.calls.plan_cancellation += 1;
        match self
            .proof
            .plan_cancellation(locator(loc.0)?, self.proof.binding())
        {
            Err(denied) => Ok(rejected(reason(denied)?, Origin::Core)),
            Ok(transition) => {
                self.apply(transition)?;
                Ok((
                    ObservedOutcome::Transition(TransitionKind::CancelledProof),
                    Origin::Core,
                ))
            }
        }
    }

    fn replace(&mut self, event: ReplaceAlias) -> Result<Decided, String> {
        self.calls.plan_replacement += 1;
        let now = proof_time(self.now)?;
        let expires = ProofTime::new(now.seconds() + TTL_SECONDS).map_err(|e| e.to_string())?;
        match self.proof.plan_replacement(
            locator(event.loc.0)?,
            self.proof.binding(),
            locator(event.fresh.0)?,
            now,
            expires,
            COOLDOWN_SECONDS,
        ) {
            Err(denied) => Ok(rejected(reason(denied)?, Origin::Core)),
            // Lifetime uniqueness of any earlier locator is a store constraint, not core.
            Ok(_) if self.issued.contains(&event.fresh.0) => {
                Ok(rejected(Reason::LocatorNotUnique, Origin::CoreAndShell))
            }
            Ok(transition) => {
                self.apply(transition)?;
                self.issued.insert(event.fresh.0);
                Ok((
                    ObservedOutcome::Transition(TransitionKind::ReplacedProof),
                    Origin::Core,
                ))
            }
        }
    }

    /// The origin label follows the call counters: `shell` means no production planner ran in
    /// this checkpoint and `core+shell` means one ran and a synthetic shell rule decided.
    fn apply_event(&mut self, event: &ReplayEvent) -> Result<Decided, String> {
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
            // Opening the link, a prefetch or a poll presents an inspect action, never a flag.
            ReplayEvent::Inspect(()) => {
                let matched = self.proof.id();
                let binding = self.proof.binding();
                match self.confirm(matched, binding, ProofAction::Inspect) {
                    Err(ProofDenied::NotExplicitConfirmation) => {
                        Ok((ObservedOutcome::NoOp(NoOpKind::Inspected), Origin::Core))
                    }
                    other => Err(format!("core planned from an inspect action: {other:?}")),
                }
            }
            ReplayEvent::Prepare(event) => self.prepare(event.command, event.request),
            ReplayEvent::Settle(fate) => self.settle(*fate),
            ReplayEvent::Cancel(loc) => self.cancel(*loc),
            ReplayEvent::Replace(event) => self.replace(*event),
            ReplayEvent::Clock(now) => {
                self.now = now.0;
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
        let plan = self
            .plan
            .as_ref()
            .map(|plan| -> Result<ObservedPlan, String> {
                Ok(ObservedPlan {
                    command: plan.command,
                    request: plan.request,
                    expected: proof_alias(plan.transition.expected())?,
                    next: proof_alias(plan.transition.next())?,
                })
            })
            .transpose()?;
        let plan_binding = self
            .plan
            .as_ref()
            .map(|plan| -> Result<PlanBinding, String> {
                Ok(PlanBinding {
                    expected: binding_alias(plan.transition.expected().binding())?,
                    next: binding_alias(plan.transition.next().binding())?,
                })
            })
            .transpose()?;
        let binding = binding_alias(self.proof.binding())?;
        if binding != binding_alias(self.initial_binding)? {
            return Err("actual proof binding changed".into());
        }
        Ok(ObservedCheckpoint {
            index,
            state: ObservedState {
                durable: ObservedDurable {
                    proof: proof_alias(self.proof)?,
                    issued: self.issued.iter().copied().collect(),
                    receipts: self
                        .receipts
                        .iter()
                        .map(|(command, request)| ObservedReceipt {
                            command: *command,
                            request: *request,
                        })
                        .collect(),
                    accounts: self.accounts,
                },
                now: self.now,
                plan,
                session: false,
                studio: false,
                admin: false,
            },
            outcome,
            evidence: Evidence {
                decided_by,
                calls: self.calls,
                binding,
                plan_binding,
            },
        })
    }
}

fn replay(input: &ReplayInput, fault: ObserverFault) -> Result<ReplayOutput, String> {
    input.validate()?;
    let mut traces = Vec::with_capacity(input.traces.len());
    for trace in &input.traces {
        let mut observer = Observer::new(fault)?;
        let mut checkpoints = Vec::with_capacity(trace.checkpoints.len());
        for checkpoint in &trace.checkpoints {
            let (outcome, origin) = observer.apply_event(&checkpoint.event).map_err(|error| {
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
fn replay_quint_native_mailbox_proof() {
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

fn alias<const LOW: u8, const HIGH: u8>(value: u8) -> Alias<LOW, HIGH> {
    Alias::try_from(value).unwrap()
}

fn exact_request(explicit: bool) -> RequestAlias {
    RequestAlias {
        operation: alias(1),
        epoch: alias(1),
        intent: IntentAlias::TheatreSignup,
        loc: alias(1),
        secret: alias(1),
        explicit,
    }
}

fn prepare(command: u8, request: RequestAlias) -> ReplayEvent {
    ReplayEvent::Prepare(PrepareAlias {
        command: alias(command),
        request,
    })
}

fn events(rest: Vec<ReplayEvent>) -> Vec<ReplayEvent> {
    let mut all = vec![ReplayEvent::NoEvent(())];
    all.extend(rest);
    all
}

#[test]
fn event_schema_rejects_unknown_fields_variants_and_out_of_domain_aliases() {
    for malformed in [
        r#"{"tag":"Assertion","value":null,"outcome":"Rejected"}"#,
        r#"{"tag":"Assertion"}"#,
        r#"{"tag":"Assertion","value":true}"#,
        r#"{"tag":"Unknown","value":null}"#,
        r#"{"tag":"Settle","value":"Retry"}"#,
        r#"{"tag":"Settle","value":0}"#,
        r#"{"tag":"Cancel","value":0}"#,
        r#"{"tag":"Cancel","value":4}"#,
        r#"{"tag":"Clock","value":6}"#,
        r#"{"tag":"Clock","value":-1}"#,
        r#"{"tag":"Clock","value":1.5}"#,
        r#"{"tag":"Replace","value":{"loc":1,"fresh":0}}"#,
        r#"{"tag":"Replace","value":{"loc":1,"fresh":2,"state":{}}}"#,
        r#"{"tag":"Prepare","value":{"command":3,"request":{"operation":1,"epoch":1,"intent":"TheatreSignup","loc":1,"secret":1,"explicit":true}}}"#,
        r#"{"tag":"Prepare","value":{"command":1,"request":{"operation":1,"epoch":1,"intent":"Admin","loc":1,"secret":1,"explicit":true}}}"#,
        r#"{"tag":"Prepare","value":{"command":1,"request":{"operation":1,"epoch":1,"intent":"TheatreSignup","loc":1,"secret":4,"explicit":true}}}"#,
        r#"{"tag":"Prepare","value":{"command":1,"request":{"operation":1,"epoch":1,"intent":"TheatreSignup","loc":1,"secret":1,"explicit":1}}}"#,
        r#"{"tag":"Prepare","value":{"command":1,"request":{"operation":1,"epoch":1,"intent":"TheatreSignup","loc":1,"secret":1}}}"#,
    ] {
        assert!(
            serde_json::from_str::<ReplayEvent>(malformed).is_err(),
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
        prepare(1, exact_request(true)),
        ReplayEvent::Assertion(()),
        ReplayEvent::Settle(Fate::Commit),
        ReplayEvent::Assertion(()),
        prepare(1, exact_request(true)),
        ReplayEvent::Assertion(()),
    ]);
    let output = replay(&input(steps), ObserverFault::None).unwrap();
    let checkpoints = &output.traces[0].checkpoints;
    for index in [1, 3, 5, 7] {
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
        ObservedOutcome::NoOp(NoOpKind::PlanPrepared)
    );
    assert_eq!(
        checkpoints[4].outcome,
        ObservedOutcome::Transition(TransitionKind::ConsumedWithAccount)
    );
    assert_eq!(
        checkpoints[6].outcome,
        ObservedOutcome::NoOp(NoOpKind::Replayed)
    );
    let last = checkpoints.last().unwrap();
    assert_eq!(last.state.durable.accounts, 1);
    // Prepare, commit recheck: two real planner calls; the exact replay never reaches the core.
    assert_eq!(last.evidence.calls.plan_confirmation, 2);
    assert_eq!(last.evidence.decided_by, Origin::NoDecision);
}

#[test]
fn every_production_seam_is_called_and_inputs_without_a_decision_call_none() {
    let steps = events(vec![
        ReplayEvent::Clock(alias(2)),
        ReplayEvent::Inspect(()),
        ReplayEvent::Replace(ReplaceAlias {
            loc: alias(1),
            fresh: alias(2),
        }),
        ReplayEvent::Cancel(alias(2)),
    ]);
    let output = replay(&input(steps), ObserverFault::None).unwrap();
    let checkpoints = &output.traces[0].checkpoints;
    assert_eq!(checkpoints[1].outcome, ObservedOutcome::External(()));
    assert_eq!(checkpoints[1].evidence.calls, Calls::default());
    assert_eq!(
        checkpoints[2].outcome,
        ObservedOutcome::NoOp(NoOpKind::Inspected)
    );
    assert_eq!(checkpoints[2].evidence.calls.plan_confirmation, 1);
    assert_eq!(
        checkpoints[3].outcome,
        ObservedOutcome::Transition(TransitionKind::ReplacedProof)
    );
    assert_eq!(checkpoints[3].state.durable.proof.loc, 2);
    assert_eq!(checkpoints[3].state.durable.issued, vec![1, 2]);
    assert_eq!(
        checkpoints[4].outcome,
        ObservedOutcome::Transition(TransitionKind::CancelledProof)
    );
    let calls = checkpoints[4].evidence.calls;
    assert_eq!(
        (
            calls.plan_confirmation,
            calls.plan_cancellation,
            calls.plan_replacement
        ),
        (1, 1, 1)
    );
}

#[test]
fn event_only_replay_is_deterministic_and_each_fault_changes_the_actual_observation() {
    let confirmation = input(events(vec![
        prepare(1, exact_request(false)),
        ReplayEvent::Assertion(()),
    ]));
    let correct = replay(&confirmation, ObserverFault::None).unwrap();
    assert_eq!(correct, replay(&confirmation, ObserverFault::None).unwrap());
    assert_eq!(
        correct.traces[0].checkpoints[1].outcome,
        ObservedOutcome::Rejected(Reason::NotExplicit)
    );
    let faulty = replay(&confirmation, ObserverFault::InspectAsExplicit).unwrap();
    assert_ne!(correct, faulty);
    assert_eq!(
        faulty.traces[0].checkpoints[1].outcome,
        ObservedOutcome::NoOp(NoOpKind::PlanPrepared)
    );

    let window = input(events(vec![
        prepare(1, exact_request(true)),
        ReplayEvent::Clock(alias(3)),
        ReplayEvent::Settle(Fate::Commit),
    ]));
    let correct = replay(&window, ObserverFault::None).unwrap();
    assert_eq!(
        correct.traces[0].checkpoints[3].outcome,
        ObservedOutcome::Rejected(Reason::Expired)
    );
    let faulty = replay(&window, ObserverFault::SkipCommitRecheck).unwrap();
    assert_eq!(
        faulty.traces[0].checkpoints[3].outcome,
        ObservedOutcome::Transition(TransitionKind::ConsumedWithAccount)
    );
    assert_eq!(faulty.traces[0].checkpoints[3].state.durable.accounts, 1);
}

#[test]
fn shell_owned_decisions_are_labelled_as_shell_not_core() {
    let steps = events(vec![
        prepare(
            1,
            RequestAlias {
                epoch: alias(2),
                ..exact_request(true)
            },
        ),
        prepare(
            1,
            RequestAlias {
                operation: alias(2),
                ..exact_request(true)
            },
        ),
        prepare(
            1,
            RequestAlias {
                secret: alias(0),
                ..exact_request(true)
            },
        ),
        ReplayEvent::Settle(Fate::Commit),
    ]);
    let output = replay(&input(steps), ObserverFault::None).unwrap();
    let checkpoints = &output.traces[0].checkpoints;
    assert_eq!(
        checkpoints[1].outcome,
        ObservedOutcome::Rejected(Reason::EpochChanged)
    );
    assert_eq!(checkpoints[1].evidence.decided_by, Origin::Shell);
    assert_eq!(
        checkpoints[2].outcome,
        ObservedOutcome::Rejected(Reason::UnknownOperation)
    );
    assert_eq!(checkpoints[2].evidence.decided_by, Origin::Shell);
    assert_eq!(checkpoints[2].evidence.calls, Calls::default());
    assert_eq!(
        checkpoints[3].outcome,
        ObservedOutcome::Rejected(Reason::InvalidSecret)
    );
    assert_eq!(checkpoints[3].evidence.decided_by, Origin::CoreAndShell);
    assert_eq!(
        checkpoints[4].outcome,
        ObservedOutcome::Rejected(Reason::NothingPlanned)
    );
    assert_eq!(checkpoints[4].evidence.decided_by, Origin::Shell);
}

#[test]
fn recovery_classes_are_only_wrong_bindings_and_the_proof_binding_never_changes() {
    let recovery = |intent| RequestAlias {
        intent,
        ..exact_request(true)
    };
    let steps = events(vec![
        prepare(1, recovery(IntentAlias::TheatreRecovery)),
        prepare(1, recovery(IntentAlias::StudioRecovery)),
        ReplayEvent::Clock(alias(2)),
        ReplayEvent::Replace(ReplaceAlias {
            loc: alias(1),
            fresh: alias(2),
        }),
    ]);
    let output = replay(&input(steps), ObserverFault::None).unwrap();
    let checkpoints = &output.traces[0].checkpoints;
    for index in [1, 2] {
        assert_eq!(
            checkpoints[index].outcome,
            ObservedOutcome::Rejected(Reason::BindingChanged)
        );
        assert_eq!(checkpoints[index].evidence.decided_by, Origin::Core);
    }
    let initial = checkpoints[0].evidence.binding;
    assert_eq!(initial.intent, IntentAlias::TheatreSignup);
    assert!(checkpoints.iter().all(|c| c.evidence.binding == initial));
    assert_eq!(checkpoints[4].state.durable.proof.loc, 2);
}
