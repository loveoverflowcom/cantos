module hosting_deploy_tests {
  import hosting_deploy.* from "./hosting_deploy"
  action check(predicate: bool): bool = all {
    assert(predicate), state' = state, before' = state, last' = Assertion
  }
  run missingBundleTest = init.then(prepare(false))
    .then(check(state.stopped and state.attempts == 0 and state.remote == NotStarted))
  run lostIntentTest = init.then(prepare(true)).then(persistIntent(false))
    .then(check(state.stopped and state.attempts == 0 and state.remote == NotStarted))
  run preparedWriteTest = init.then(prepare(true)).then(persistIntent(true)).then(fire)
    .then(check(preparedWrite and writeOnlyPrepared and atMostOnce))
  run unknownOutcomeTest = init.then(prepare(true)).then(persistIntent(true)).then(fire)
    .then(finish(Unknown)).then(persistOutcome).then(check(unknownOutcome and state.attempts == 1))
  run confirmedSmokeFailureTest = init.then(prepare(true)).then(persistIntent(true)).then(fire)
    .then(finish(Confirmed)).then(smoke(false)).then(persistOutcome)
    .then(check(confirmedSmokeFailure and confirmedSticky and state.attempts == 1))
  run rejectedTest = init.then(prepare(true)).then(persistIntent(true)).then(fire)
    .then(finish(Rejected)).then(check(state.remote == Rejected and state.attempts == 1))
}
