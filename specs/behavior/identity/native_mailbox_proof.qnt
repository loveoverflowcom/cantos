// Draft #334/#335 executable expectations for ONE native Identity Theatre-signup mailbox proof.
// Pure plan/commit lifecycle only: no secret entropy, digest, SQL isolation, mail, RPC, HTTP or
// front end. Integers are equality/time classes, not locator bytes, Unix seconds or row counts.
module native_mailbox_proof {
  type ProofState = Pending | Consumed | Cancelled
  // The stored workflow is a Theatre signup. Recovery classes exist in the pure core's binding
  // type but the recovery flow is deferred (ADR 0045): here they are only wrong-binding inputs.
  type Intent = TheatreSignup | TheatreRecovery | StudioRecovery
  type Proof = { loc: int, state: ProofState, issuedAt: int, expiresAt: int }
  // The complete request fingerprint a receipt binds. `secret` names the locator the presented
  // secret was minted for (0 = forged); `explicit` is the adapter-established action, never a
  // client flag. Inspect/prefetch/polling map to explicit=false or to the `inspect` event.
  type Request = { operation: int, epoch: int, intent: Intent, loc: int, secret: int, explicit: bool }
  // A plan is a compare-and-replace value. It has no effect until the owner commits it.
  type Plan = NoPlan | Planned({ command: int, request: Request, expected: Proof, next: Proof })
  type Durable = { proof: Proof, issued: Set[int], receipts: int -> Request, accounts: int }
  type State = { durable: Durable, now: int, plan: Plan, session: bool, studio: bool, admin: bool }
  type Reason = EpochChanged | OperationReused | NotExplicit | UnknownOperation | BindingChanged
    | ProofChanged | InvalidSecret | AlreadyConsumed | AlreadyCancelled | BeforeIssue | Expired
    | NothingPlanned | StaleSnapshot | CommitRejected | ReusedLocator | Cooldown
  type NoOpKind = Inspected | PlanPrepared | Replayed
  type TransitionKind = ConsumedWithAccount | CancelledProof | ReplacedProof
  // Rejected and UnknownEffect are distinct commit faults: unknown is never rollback proof.
  type Outcome = NoOutcome | Rejected(Reason) | NoOp(NoOpKind) | Transition(TransitionKind)
    | UnknownEffect | External
  type Fate = Commit | Reject | UnknownApplied | UnknownLost
  type Event = NoEvent | Assertion | Inspect | Prepare({ command: int, request: Request })
    | Settle(Fate) | Cancel(int) | Replace({ loc: int, fresh: int }) | Clock(int)

  pure val STORE_EPOCH = 1
  pure val OPERATION = 1
  pure val TTL = 2
  pure val COOLDOWN = 1
  pure val initial: State = {
    durable: { proof: { loc: 1, state: Pending, issuedAt: 1, expiresAt: 3 }, issued: Set(1),
      receipts: Map(), accounts: 0 },
    now: 1, plan: NoPlan, session: false, studio: false, admin: false
  }
  var state: State
  var before: State
  var last: Event
  var outcome: Outcome

  action init = all { state' = initial, before' = initial, last' = NoEvent, outcome' = NoOutcome }
  action record(successor: State, event: Event, result: Outcome): bool = all {
    state' = successor, before' = state, last' = event, outcome' = result
  }
  // Opening the mailed link, a GET, a prefetch or a receipt poll: nothing is presented to the owner.
  action inspect = record(state, Inspect, NoOp(Inspected))
  // The owner clock is an input fact; it may move backwards. Expiry is derived, not a state.
  action clock(t: int): bool = record({ ...state, now: t }, Clock(t), External)

  // Plan a confirmation from the authoritative snapshot. Epoch is checked even before replay.
  action prepare(command: int, p: Request): bool = {
    val d = state.durable
    val event = Prepare({ command: command, request: p })
    if (p.epoch != STORE_EPOCH) record(state, event, Rejected(EpochChanged))
    else if (d.receipts.keys().contains(command))
      if (d.receipts.get(command) == p) record(state, event, NoOp(Replayed))
      else record(state, event, Rejected(OperationReused))
    else if (not(p.explicit)) record(state, event, Rejected(NotExplicit))
    else if (p.operation != OPERATION) record(state, event, Rejected(UnknownOperation))
    else if (p.intent != TheatreSignup) record(state, event, Rejected(BindingChanged))
    else if (p.loc != d.proof.loc) record(state, event, Rejected(ProofChanged))
    else if (p.secret != p.loc) record(state, event, Rejected(InvalidSecret))
    else if (d.proof.state == Consumed) record(state, event, Rejected(AlreadyConsumed))
    else if (d.proof.state == Cancelled) record(state, event, Rejected(AlreadyCancelled))
    else if (state.now < d.proof.issuedAt) record(state, event, Rejected(BeforeIssue))
    else if (state.now >= d.proof.expiresAt) record(state, event, Rejected(Expired))
    else record({ ...state, plan: Planned({ command: command, request: p, expected: d.proof,
      next: { ...d.proof, state: Consumed } }) }, event, NoOp(PlanPrepared))
  }

  // Commit compares the ENTIRE expected snapshot. Plan and commit share one owner transaction
  // in the implementation, so the window is evaluated at the commit instant. Consumption and
  // account/credential creation are one commit; the receipt is recorded with them.
  action settle(fate: Fate): bool = {
    val event = Settle(fate)
    val d = state.durable
    match state.plan {
      | NoPlan => record(state, event, Rejected(NothingPlanned))
      | Planned(plan) => {
        val cleared = { ...state, plan: NoPlan }
        val applied = { ...cleared, durable: { ...d, proof: plan.next,
          receipts: d.receipts.put(plan.command, plan.request), accounts: d.accounts + 1 } }
        if (plan.expected != d.proof) record(cleared, event, Rejected(StaleSnapshot))
        else if (state.now < d.proof.issuedAt) record(cleared, event, Rejected(BeforeIssue))
        else if (state.now >= d.proof.expiresAt) record(cleared, event, Rejected(Expired))
        else if (fate == Commit) record(applied, event, Transition(ConsumedWithAccount))
        else if (fate == UnknownApplied) record(applied, event, UnknownEffect)
        else if (fate == UnknownLost) record(cleared, event, UnknownEffect)
        else record(cleared, event, Rejected(CommitRejected))
      }
    }
  }

  // Owner cancellation of the current locator, atomic in one owner transaction.
  action cancel(loc: int): bool = {
    val proof = state.durable.proof
    if (loc != proof.loc) record(state, Cancel(loc), Rejected(ProofChanged))
    else if (proof.state == Consumed) record(state, Cancel(loc), Rejected(AlreadyConsumed))
    else if (proof.state == Cancelled) record(state, Cancel(loc), Rejected(AlreadyCancelled))
    else record({ ...state, durable: { ...state.durable, proof: { ...proof, state: Cancelled } } },
      Cancel(loc), Transition(CancelledProof))
  }

  // Replacement retires the old locator: a lifetime-unique fresh locator and secret, same binding.
  action replace(loc: int, fresh: int): bool = {
    val current = state.durable.proof
    val event = Replace({ loc: loc, fresh: fresh })
    if (loc != current.loc) record(state, event, Rejected(ProofChanged))
    else if (current.state == Consumed) record(state, event, Rejected(AlreadyConsumed))
    else if (current.state == Cancelled) record(state, event, Rejected(AlreadyCancelled))
    else if (state.durable.issued.contains(fresh)) record(state, event, Rejected(ReusedLocator))
    else if (state.now < current.issuedAt) record(state, event, Rejected(BeforeIssue))
    else if (state.now - current.issuedAt < COOLDOWN) record(state, event, Rejected(Cooldown))
    else record({ ...state, durable: { ...state.durable,
      proof: { loc: fresh, state: Pending, issuedAt: state.now, expiresAt: state.now + TTL },
      issued: state.durable.issued.union(Set(fresh)) } }, event, Transition(ReplacedProof))
  }

  // Bias sampling toward exact current/retired presentations and commits. This is not a
  // fairness assumption: the arbitrary branch keeps every modeled input reachable.
  action guided = {
    nondet command = Set(1, 2).oneOf()
    nondet explicit = Set(false, true).oneOf()
    nondet fresh = 1.to(3).oneOf()
    nondet t = 0.to(5).oneOf()
    nondet fate = Set(Commit, Reject, UnknownApplied, UnknownLost).oneOf()
    val loc = state.durable.proof.loc
    val exact = { operation: OPERATION, epoch: STORE_EPOCH, intent: TheatreSignup, loc: loc,
      secret: loc, explicit: explicit }
    any {
      prepare(command, exact), prepare(command, { ...exact, loc: 1, secret: 1 }), settle(fate),
      replace(loc, fresh), cancel(loc), clock(t), inspect
    }
  }
  action step = {
    nondet command = Set(1, 2).oneOf()
    nondet operation = Set(1, 2).oneOf()
    nondet epoch = Set(1, 2).oneOf()
    nondet intent = Set(TheatreSignup, TheatreRecovery, StudioRecovery).oneOf()
    nondet loc = 1.to(3).oneOf()
    nondet secret = 0.to(3).oneOf()
    nondet explicit = Set(false, true).oneOf()
    nondet fresh = 1.to(3).oneOf()
    nondet t = 0.to(5).oneOf()
    nondet fate = Set(Commit, Reject, UnknownApplied, UnknownLost).oneOf()
    any {
      guided, guided, guided,
      prepare(command, { operation: operation, epoch: epoch, intent: intent, loc: loc,
        secret: secret, explicit: explicit }),
      settle(fate), cancel(loc), replace(loc, fresh), clock(t), inspect
    }
  }

  val consumedNow = state.durable.proof.state == Consumed and before.durable.proof.state != Consumed
  def plannedRequest(predicate: (Request, Proof) => bool): bool = match before.plan {
    | NoPlan => false
    | Planned(plan) => predicate(plan.request, plan.expected)
  }
  def isRejected(result: Outcome): bool = match result {
    | Rejected(_) => true
    | _ => false
  }

  // Only an explicit confirmation consumes; inspection changes nothing at all.
  val explicitOnly = (last == Inspect implies state == before) and
    (consumedNow implies plannedRequest((r, _) => r.explicit))
  // Consumption needs the stored operation/intent/epoch, the current locator, its own secret and
  // the commit instant inside [issuedAt, expiresAt).
  val consumedWithinBoundWindow = consumedNow implies (
    before.now >= before.durable.proof.issuedAt and before.now < before.durable.proof.expiresAt and
    plannedRequest((r, _) => r.operation == OPERATION and r.epoch == STORE_EPOCH and
      r.intent == TheatreSignup and r.loc == before.durable.proof.loc and r.secret == r.loc))
  // Consumed and Cancelled never revive, are never replaced and never change.
  val terminalSticky = before.durable.proof.state != Pending implies state.durable.proof == before.durable.proof
  // A locator other than the current one (retired by replacement, or never issued) has no effect.
  val retiredLocatorRejected = match last {
    | Prepare(e) => e.request.epoch != STORE_EPOCH or before.durable.receipts.keys().contains(e.command)
      or e.request.loc == before.durable.proof.loc or (state == before and isRejected(outcome))
    | Cancel(loc) => loc == before.durable.proof.loc or (state == before and isRejected(outcome))
    | Replace(e) => e.loc == before.durable.proof.loc or (state == before and isRejected(outcome))
    | _ => true
  }
  // Creating a plan never changes authoritative state.
  val planHasNoEffect = match last {
    | Prepare(_) => state.durable == before.durable
    | _ => true
  }
  // A commit applies only a plan whose expected snapshot equals the current one.
  val commitComparesSnapshot = consumedNow implies plannedRequest((_, expected) => expected == before.durable.proof)
  // At most one account per workflow; an exact replay is history, never a second effect.
  val singleAccountEffect = state.durable.accounts <= 1 and (outcome == NoOp(Replayed) implies state == before)
  // A consumed proof and a created account grant no session, Studio or Admin authority.
  val noAuthority = not(state.session) and not(state.studio) and not(state.admin)

  val consumedExplicitly = outcome == Transition(ConsumedWithAccount) and state.durable.accounts == 1
  val inspectedPending = last == Inspect and state.durable.proof.state == Pending and before.plan != NoPlan
  val expiredAtCommit = last == Settle(Commit) and outcome == Rejected(Expired)
  val cancelledReplaceRefused = before.durable.proof.state == Cancelled and outcome == Rejected(AlreadyCancelled)
    and match last { | Replace(_) => true | _ => false }
  val retiredLocatorRefused = outcome == Rejected(ProofChanged) and state.durable.issued.size() >= 2
  val planWithoutEffect = outcome == NoOp(PlanPrepared) and state.durable.proof.state == Pending
  val staleCommitRefused = outcome == Rejected(StaleSnapshot) and state.durable.proof.state == Pending
  val exactRetryReplayed = outcome == NoOp(Replayed) and state.durable.accounts == 1
}
