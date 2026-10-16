// Draft executable expectations for #203's producer-side generation attempts.
// Authority: #203 section 5's durable safety and ADR 0022, not live provider, Studio or Functions behavior.
// One operation, two abstract request classes (1 and 2), costs 1 or 2 against a budget of 3 and
// at most 3 attempts. The provider, persistence, clocks, authorization and billing are NOT
// modeled: `dispatch` abstracts "a billable provider call started", and uncertain outcomes are
// nondeterministic facts. The idempotency fingerprint, the real conditional store and the
// provider adapters remain native tests and operator acceptance.
module forge_attempt {
  type Phase = Idle | Open | Succeeded | Rejected | Uncertain
  type Event = NoEvent | Assertion | Submit | Dispatch | Settle | Recover | Regenerate
  type Verdict =
    NoVerdict | Reserved | Replayed | Conflicted | Changed
    | RefusedInFlight | RefusedNeedsDecision | RefusedStaleDecision | RefusedAttempts | RefusedBudget
  type State = {
    phase: Phase, request: int, attempts: int, spent: int,
    dispatched: bool, calls: int, successes: int
  }
  pure val maxAttempts = 3
  pure val budget = 3
  pure val initial: State = {
    phase: Idle, request: 0, attempts: 0, spent: 0, dispatched: false, calls: 0, successes: 0
  }
  var state: State
  var before: State
  var last: Event
  var verdict: Verdict
  var ack: int

  action init = all {
    state' = initial, before' = initial, last' = NoEvent, verdict' = NoVerdict, ack' = 0
  }
  action record(successor: State, event: Event, result: Verdict, presented: int): bool = all {
    state' = successor, before' = state, last' = event, verdict' = result, ack' = presented
  }

  // The only way an operation exists: the first reservation, persisted before any provider call.
  action reserveFirst(request: int, cost: int): bool = all {
    state.attempts == 0,
    cost <= budget,
    record({ ...state, phase: Open, request: request, attempts: 1, spent: cost, dispatched: false }, Submit, Reserved, 0)
  }
  // Same key and the same request: the recorded state is returned and nothing is dispatched.
  action replay(request: int): bool = all {
    state.attempts > 0, request == state.request,
    record(state, Submit, Replayed, 0)
  }
  // Same key and a different request: a conflict, never a new attempt.
  action conflict(request: int): bool = all {
    state.attempts > 0, request != state.request,
    record(state, Submit, Conflicted, 0)
  }
  // Only the invocation that won the reservation starts the one provider call of its attempt.
  action dispatch = all {
    state.phase == Open, not(state.dispatched),
    record({ ...state, dispatched: true, calls: state.calls + 1 }, Dispatch, Changed, 0)
  }
  // A recorded provider outcome. Success needs a started call; a confirmed rejection does not.
  action settle(result: Phase): bool = all {
    state.phase == Open,
    Set(Succeeded, Rejected, Uncertain).contains(result),
    result == Rejected or state.dispatched,
    record({ ...state, phase: result, successes: state.successes + (if (result == Succeeded) 1 else 0) }, Settle, Changed, 0)
  }
  // The attempt's owner is gone before any outcome was recorded: the effect is unknown.
  action recover = all {
    state.phase == Open,
    record({ ...state, phase: Uncertain }, Recover, Changed, 0)
  }

  // A new attempt is an explicit decision. The operator's acknowledgement must name exactly the
  // latest uncertain attempt (0 = none presented; a stale number names an earlier attempt).
  def decided(presented: int): bool = state.attempts > 0 and state.phase != Open and (state.phase != Uncertain or presented == state.attempts)

  // Refusal precedence, mirrored by the Rust core: in flight, undecided uncertainty, attempt limit, budget.
  action refuseInFlight = all {
    state.phase == Open,
    record(state, Regenerate, RefusedInFlight, 0)
  }
  action refuseUndecided = all {
    state.phase == Uncertain,
    record(state, Regenerate, RefusedNeedsDecision, 0)
  }
  action refuseStale(presented: int): bool = all {
    state.phase == Uncertain, presented != 0, presented != state.attempts,
    record(state, Regenerate, RefusedStaleDecision, presented)
  }
  action refuseAttempts(presented: int): bool = all {
    decided(presented), state.attempts >= maxAttempts,
    record(state, Regenerate, RefusedAttempts, presented)
  }
  action refuseBudget(presented: int, cost: int): bool = all {
    decided(presented), state.attempts < maxAttempts, state.spent + cost > budget,
    record(state, Regenerate, RefusedBudget, presented)
  }
  action regenerate(presented: int, cost: int): bool = all {
    decided(presented), state.attempts < maxAttempts, state.spent + cost <= budget,
    record({ ...state, phase: Open, attempts: state.attempts + 1, spent: state.spent + cost, dispatched: false }, Regenerate, Reserved, presented)
  }

  action step = any {
    reserveFirst(1, 1), reserveFirst(1, 2), reserveFirst(2, 1),
    replay(1), replay(2), conflict(1), conflict(2),
    dispatch, settle(Succeeded), settle(Rejected), settle(Uncertain), recover,
    refuseInFlight, refuseUndecided,
    refuseStale(state.attempts - 1), refuseAttempts(state.attempts), refuseAttempts(0),
    refuseBudget(state.attempts, 1), refuseBudget(state.attempts, 2), refuseBudget(0, 2),
    regenerate(state.attempts, 1), regenerate(state.attempts, 2), regenerate(state.attempts - 1, 1),
    regenerate(0, 1), regenerate(0, 2)
  }

  // A provider call never precedes a persisted reservation, and one attempt starts one call.
  val callsNeverExceedReservations = state.calls <= state.attempts
  // Reserved spend and attempts stay inside the server-controlled policy.
  val spendIsBounded = state.spent <= budget and state.attempts <= maxAttempts
  // A new attempt is never reserved while the previous one has no recorded outcome.
  val noRetryWhileOpen = verdict != Reserved or last != Regenerate or before.phase != Open
  // Uncertainty is left only by an explicit regeneration whose acknowledgement names that attempt.
  val uncertaintyNeedsAcknowledgement = before.phase != Uncertain or state.phase == Uncertain or (last == Regenerate and verdict == Reserved and ack == before.attempts)
  // A replay, a conflict or a refusal changes nothing: no reservation, no dispatch, no spend.
  val replayAndRefusalChangeNothing = not(Set(Replayed, Conflicted, RefusedInFlight, RefusedNeedsDecision, RefusedStaleDecision, RefusedAttempts, RefusedBudget).contains(verdict)) or state == before
  // The request pinned by the first reservation is never replaced.
  val requestIsImmutable = before.request == 0 or state.request == before.request
  // A recorded result is never erased, and no more results exist than attempts.
  val resultsAreKept = state.successes >= before.successes and state.successes <= state.attempts

  val succeededOnce = state.phase == Succeeded and state.successes == 1
  val duplicateReplayed = last == Submit and verdict == Replayed
  val conflictRefused = last == Submit and verdict == Conflicted
  val unknownOutcome = state.phase == Uncertain and state.dispatched
  val abandonedBeforeDispatch = state.phase == Uncertain and not(state.dispatched)
  val reservedAfterDecision = last == Regenerate and verdict == Reserved and before.phase == Uncertain
  val staleDecisionRefused = verdict == RefusedStaleDecision
  val attemptLimitRefused = verdict == RefusedAttempts
  val budgetRefused = verdict == RefusedBudget
}
