module forge_attempt_tests {
  import forge_attempt.* from "./forge_attempt"

  action check(predicate: bool): bool = all {
    // A named assertion is a no-effect harness checkpoint, not another submit or dispatch.
    assert(predicate), state' = state, before' = state, last' = Assertion, verdict' = NoVerdict, ack' = ack
  }

  run singleAttemptSucceedsTest = init.then(reserveFirst(1, 1)).then(dispatch).then(settle(Succeeded))
    .then(check(succeededOnce and state.calls == 1 and state.attempts == 1 and state.spent == 1))
  run retryAfterLostResponseReplaysTest = init.then(reserveFirst(1, 1)).then(dispatch).then(settle(Succeeded))
    .then(replay(1)).then(check(duplicateReplayed and state.calls == 1 and state.attempts == 1 and state.successes == 1))
  run duplicateWhileOpenTest = init.then(reserveFirst(1, 1)).then(replay(1))
    .then(check(duplicateReplayed and state.phase == Open and state.calls == 0 and state.attempts == 1))
  run conflictingRequestTest = init.then(reserveFirst(1, 1)).then(dispatch).then(settle(Succeeded)).then(conflict(2))
    .then(check(conflictRefused and state.request == 1 and state.attempts == 1 and state.calls == 1))
  // The loser of a different-request reservation race observes this already-open operation.
  // This is the required sequential observation, not a model of a concurrent storage adapter.
  run conflictingRequestWhileOpenTest = init.then(reserveFirst(1, 1)).then(conflict(2))
    .then(check(conflictRefused and state.request == 1 and state.phase == Open and state.calls == 0 and state.attempts == 1))
  run crashBeforeDispatchTest = init.then(reserveFirst(1, 1)).then(recover)
    .then(check(abandonedBeforeDispatch and state.calls == 0))
    .then(replay(1)).then(check(duplicateReplayed and state.calls == 0 and state.attempts == 1))
    .then(refuseUndecided).then(check(verdict == RefusedNeedsDecision and state.attempts == 1 and state.calls == 0))
  run crashAfterDispatchTest = init.then(reserveFirst(1, 1)).then(dispatch).then(recover)
    .then(check(unknownOutcome and state.calls == 1 and state.successes == 0))
    .then(refuseUndecided).then(check(verdict == RefusedNeedsDecision and state.attempts == 1))
  run unknownProviderOutcomeTest = init.then(reserveFirst(1, 1)).then(dispatch).then(settle(Uncertain))
    .then(check(unknownOutcome and state.successes == 0))
    .then(regenerate(1, 1)).then(check(reservedAfterDecision and state.attempts == 2 and state.calls == 1))
    .then(dispatch).then(settle(Succeeded)).then(check(succeededOnce and state.calls == 2 and state.spent == 2))
  run staleDecisionTest = init.then(reserveFirst(1, 1)).then(dispatch).then(settle(Rejected))
    .then(regenerate(0, 1)).then(recover)
    .then(refuseStale(1)).then(check(staleDecisionRefused and state.attempts == 2 and state.phase == Uncertain))
    .then(regenerate(2, 1)).then(check(reservedAfterDecision and state.attempts == 3))
  run regenerationWhileOpenTest = init.then(reserveFirst(1, 1)).then(refuseInFlight)
    .then(check(verdict == RefusedInFlight and state.attempts == 1 and state.calls == 0))
  run explicitRegenerationKeepsResultTest = init.then(reserveFirst(1, 1)).then(dispatch).then(settle(Succeeded))
    .then(regenerate(0, 1)).then(check(last == Regenerate and verdict == Reserved and state.attempts == 2 and state.successes == 1))
  run budgetRefusalTest = init.then(reserveFirst(1, 2)).then(dispatch).then(settle(Succeeded))
    .then(refuseBudget(0, 2)).then(check(budgetRefused and state.attempts == 1 and state.spent == 2))
  run attemptLimitRefusalTest = init.then(reserveFirst(1, 1)).then(dispatch).then(settle(Rejected))
    .then(regenerate(0, 1)).then(dispatch).then(settle(Rejected))
    .then(regenerate(0, 1)).then(dispatch).then(settle(Rejected))
    .then(refuseAttempts(0)).then(check(attemptLimitRefused and state.attempts == 3 and state.calls == 3))
}
