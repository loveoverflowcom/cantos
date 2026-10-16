module theatre_registration_tests {
  import theatre_registration.* from "./theatre_registration"
  action check(predicate: bool): bool = all { assert(predicate), state' = state, before' = state, last' = Assertion, outcome' = NoOutcome }
  run expiredTest = init.then(expire).then(claim(0)).then(check(state.phase == Awaiting and state.createCalls == 0))
  run rotatedProofTest = init.then(rotate).then(claim(0)).then(check(state.phase == Awaiting))
    .then(claim(1)).then(create).then(check(claimed and proofBeforeCreate))
  run cancelTest = init.then(cancel).then(claim(0)).then(check(state.phase == Cancelled and state.createCalls == 0))
  run singleUseTest = init.then(claim(0)).then(create).then(claim(0)).then(check(claimed and atMostOnce))
  run uncertainCreateTest = init.then(claim(0)).then(create).then(created(false)).then(claim(0))
    .then(check(stoppedAfterUnknown and blockedSticky and state.setupCalls == 0))
  run uncertainSetupTest = init.then(claim(0)).then(create).then(created(true)).then(beginSetup).then(setup)
    .then(check(setupDispatched and setupOnlyNewPerson)).then(setupResult(false))
    .then(check(state.phase == Blocked and state.mailCalls == 0))
  run completeTest = init.then(claim(0)).then(create).then(created(true)).then(beginSetup).then(setup)
    .then(setupResult(true)).then(mail).then(mailResult(true))
    .then(check(completeWithoutSession and noAuthorityIssued and atMostOnce))
  run uncertainMailTest = init.then(claim(0)).then(create).then(created(true)).then(beginSetup).then(setup)
    .then(setupResult(true)).then(mail).then(mailResult(false)).then(claim(0))
    .then(check(state.phase == Blocked and blockedSticky and noAuthorityIssued))
}
