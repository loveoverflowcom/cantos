module bearer_lifecycle_tests {
  import bearer_lifecycle.* from "./bearer_lifecycle"
  action check(predicate: bool): bool = all { assert(predicate), state'=state, before'=state, last'=Assertion, outcome'=NoOutcome }
  run refreshTest = init.then(refresh(0)).then(check(rotated and boundedAuthority and authTimeUnchanged))
    .then(refresh(1)).then(check(rotated and state.generation == 2 and boundedAuthority and authTimeUnchanged))
  run replayTest = init.then(refresh(0)).then(refresh(0)).then(check(replayed and singleUse))
    .then(refresh(1)).then(check(revocationSticky and outcome == Denied))
  run logoutTest = init.then(logout).then(refresh(0)).then(check(logoutRefused and revocationSticky))
  run expiryTest = init.then(expire).then(refresh(0)).then(check(expiryRefused and boundedAuthority))
}
