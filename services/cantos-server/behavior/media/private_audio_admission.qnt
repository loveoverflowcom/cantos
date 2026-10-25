// One already-prepared, intrinsically valid private AudioSubmission.
// Authority: audio-lifecycle-v1.md, "State, reviews and candidate blockers".
// Synthetic byte aliases are equality classes, not real hashes, lengths or media.
module private_audio_admission {
  type Request = { digest: int, length: int }
  type AdmissionError = PermissionRevoked | PermissionExpired | InvalidExpiry
    | StaleBrief | DigestMismatch | OperationReused
  type Outcome = NoOutcome | Inspect | Replay | Admitted | Rejected(AdmissionError)
    | MediaRejected | Cancelled | FactsChanged
  type OwnerState = AwaitingBytes | OwnerAdmitted | Expired | Revoked | Stale
  type Receipt = Unsettled | Settled({ id: int, admittedAt: int, digest: int, length: int })
  // Pending is simulator/shell history. It adds no persisted submission state.
  type Pending = Idle | Inspecting(Request)
  type Facts = { now: int, revoked: bool, current: bool }
  type State = {
    facts: Facts, receipt: Receipt, pending: Pending,
    inspections: int, admissions: int, ownerView: OwnerState
  }
  type Event = NoEvent | Assertion | Begin(Request) | Complete(bool) | Abandon
    | Revoke | Invalidate | RestoreCurrent | Clock(int)

  pure val createdAt = 1
  pure val expiresAt = 3
  pure val exact: Request = { digest: 0, length: 0 }
  pure def settled(r: Receipt): bool = match r { Unsettled => false | Settled(_) => true }
  pure def admittedAt(r: Receipt): int = match r { Unsettled => createdAt | Settled(x) => x.admittedAt }
  pure def pendingRequest(p: Pending): Request = match p { Idle => exact | Inspecting(q) => q }
  pure def ownerProjection(f: Facts, r: Receipt): OwnerState =
    if (not(f.current)) Stale
    else if (f.revoked) Revoked
    else if (settled(r)) OwnerAdmitted
    else if (f.now >= expiresAt) Expired
    else AwaitingBytes
  pure def project(s: State): State = { ...s, ownerView: ownerProjection(s.facts, s.receipt) }

  // Exact pure seam: lifecycle::decide_admission. Intrinsic/foreign-snapshot
  // construction errors are outside this already-valid one-submission instance.
  pure def admission(s: State, q: Request): Outcome =
    if (s.facts.revoked) Rejected(PermissionRevoked)
    else if (s.facts.now >= expiresAt) Rejected(PermissionExpired)
    else if (s.facts.now < createdAt) Rejected(InvalidExpiry)
    else if (not(s.facts.current)) Rejected(StaleBrief)
    else if (q.digest != exact.digest or q.length != exact.length)
      Rejected(if (settled(s.receipt)) OperationReused else DigestMismatch)
    else if (settled(s.receipt) and s.facts.now < admittedAt(s.receipt)) Rejected(InvalidExpiry)
    else if (settled(s.receipt)) Replay
    else Inspect

  pure val initial: State = {
    facts: { now: createdAt, revoked: false, current: true }, receipt: Unsettled,
    pending: Idle, inspections: 0, admissions: 0, ownerView: AwaitingBytes
  }
  var state: State
  var before: State
  var last: Event
  var outcome: Outcome

  action init = all {
    state' = initial, before' = initial, last' = NoEvent, outcome' = NoOutcome
  }
  action checkpoint(successor: State, event: Event, result: Outcome): bool = all {
    state' = project(successor), before' = state, last' = event, outcome' = result
  }
  action begin(q: Request): bool = {
    val result = admission(state, q)
    val successor = if (result == Inspect)
      { ...state, pending: Inspecting(q), inspections: state.inspections + 1 }
      else state
    all { state.pending == Idle, checkpoint(successor, Begin(q), result) }
  }
  action complete(verified: bool): bool = {
    val request = pendingRequest(state.pending)
    // Inspection/storage is an abstract external fact. Recheck today's pure
    // authority before settlement; Inspect was never itself an admitted receipt.
    val decision = admission(state, request)
    val result = if (not(verified)) MediaRejected
      else if (decision == Inspect) Admitted else decision
    val successor = if (result == Admitted) {
      ...state, pending: Idle,
      receipt: Settled({ id: 1, admittedAt: state.facts.now, digest: request.digest, length: request.length }),
      admissions: state.admissions + 1
    } else { ...state, pending: Idle }
    all { state.pending != Idle, checkpoint(successor, Complete(verified), result) }
  }
  action abandon = all {
    state.pending != Idle,
    checkpoint({ ...state, pending: Idle }, Abandon, Cancelled)
  }
  action revoke = checkpoint({ ...state, facts: { ...state.facts, revoked: true } }, Revoke, FactsChanged)
  action invalidate = checkpoint({ ...state, facts: { ...state.facts, current: false } }, Invalidate, FactsChanged)
  // The same immutable binding can become current again after archive/restore.
  // This is an external fact, not a rewrite of changed Content/rights/guidance.
  action restoreCurrent = checkpoint({ ...state, facts: { ...state.facts, current: true } }, RestoreCurrent, FactsChanged)
  // Harness time partitions include clock rollback; no production clock guarantee.
  action clock(now: int): bool = all {
    0 <= now and now <= 4,
    checkpoint({ ...state, facts: { ...state.facts, now: now } }, Clock(now), FactsChanged)
  }
  action sampledInputs = {
    nondet digest = Set(0, 1).oneOf()
    nondet lengthPick = Set(0, 1).oneOf()
    nondet verified = Set(true, false).oneOf()
    nondet now = 0.to(4).oneOf()
    any {
      begin({ digest: digest, length: lengthPick }), complete(verified), abandon,
      revoke, invalidate, restoreCurrent, clock(now)
    }
  }

  // Bias toward exact attempts without removing any legal input/environment action.
  // This keeps settled/replay antecedents observable before authority changes.
  action step = any { begin(exact), complete(true), sampledInputs }

  // Relational properties use the checkpoint, not a second transition reducer.
  val rejectedPreservesReceipt = match outcome {
    Rejected(_) => state.receipt == before.receipt and state.admissions == before.admissions
    | MediaRejected => state.receipt == before.receipt and state.admissions == before.admissions
    | _ => true
  }
  val receiptIsImmutable = not(settled(before.receipt)) or state.receipt == before.receipt
  val producerAuthority =
    not(Set(Inspect, Replay, Admitted).contains(outcome)) or
    (not(before.facts.revoked) and before.facts.current and
      createdAt <= before.facts.now and before.facts.now < expiresAt and
      (outcome != Replay or before.facts.now >= admittedAt(before.receipt)))
  val payloadOwnsAdmission =
    not(Set(Inspect, Replay, Admitted).contains(outcome)) or
    (match last {
      Begin(q) => q == exact
      | Complete(_) => pendingRequest(before.pending) == exact
      | _ => false
    })
  val inspectionRequired = outcome != Admitted or
    (last == Complete(true) and before.pending != Idle and before.receipt == Unsettled)
  val ownerViewPrecedence =
    if (not(state.facts.current)) state.ownerView == Stale
    else if (state.facts.revoked) state.ownerView == Revoked
    else if (settled(state.receipt)) state.ownerView == OwnerAdmitted
    else if (state.facts.now >= expiresAt) state.ownerView == Expired
    else state.ownerView == AwaitingBytes
  val settlementShape =
    state.inspections >= 0 and 0 <= state.admissions and state.admissions <= 1 and
    (match state.receipt {
      Unsettled => state.admissions == 0
      | Settled(r) => state.admissions == 1 and r.id == 1 and
          createdAt <= r.admittedAt and r.admittedAt < expiresAt and
          r.digest == exact.digest and r.length == exact.length
    })

  val inspected = outcome == Inspect and state.receipt == Unsettled
  val admitted = outcome == Admitted and settled(state.receipt)
  val replayed = outcome == Replay and state.admissions == 1
  val digestRejected = outcome == Rejected(DigestMismatch)
  val operationRejected = outcome == Rejected(OperationReused)
  val completionRevoked = last == Complete(true) and outcome == Rejected(PermissionRevoked)
  val mediaRejected = outcome == MediaRejected and state.receipt == Unsettled
  val staleAndRevoked = state.ownerView == Stale and state.facts.revoked
  val expiredOwnerHistory = state.facts.now >= expiresAt and state.ownerView == OwnerAdmitted
  val clockRejected = outcome == Rejected(InvalidExpiry)
  val authorityRestored = not(before.facts.current) and last == RestoreCurrent and state.facts.current
}
