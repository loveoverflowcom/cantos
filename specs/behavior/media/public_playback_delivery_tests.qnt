module public_playback_delivery_tests {
  import public_playback_delivery.* from "./public_playback_delivery"

  action check(predicate: bool): bool = all {
    // A named assertion is a no-effect harness checkpoint, not another fetch.
    assert(predicate), state' = state, before' = state, last' = Assertion, outcome' = NoOutcome
  }

  run servedTest = init.then(authorize)
    .then(check(outcome == Issued and heldGrant(state.held).expires == 3))
    .then(beginFetch).then(check(outcome == Opened))
    .then(completeFetch).then(check(served and state.served == 1))
  run expiredRefreshTest = init.then(authorize).then(clock(3)).then(beginFetch)
    .then(check(expiredDenied and state.pending == Idle))
    .then(authorize).then(check(refreshed and heldGrant(state.held).expires == 5))
    .then(beginFetch).then(completeFetch).then(check(served))
  run withdrawBeforeFetchTest = init.then(authorize).then(withdraw).then(beginFetch)
    .then(check(withdrawnDenied)).then(authorize).then(check(authorizationDenied))
  run withdrawDuringOpenTest = init.then(authorize).then(beginFetch).then(withdraw)
    .then(completeFetch).then(check(recheckDenied and state.served == 0))
  run expiryDuringOpenTest = init.then(authorize).then(beginFetch).then(clock(3))
    .then(completeFetch).then(check(outcome == Denied(Expired) and state.served == 0))
  run forgedGrantTest = init.then(present({ expires: 2, authentic: false })).then(beginFetch)
    .then(check(forgedDenied))
  run unboundedGrantTest = init.then(present({ expires: 8, authentic: true })).then(beginFetch)
    .then(check(unboundedDenied))
  run outsideRightsGrantTest = init.then(clock(4)).then(present({ expires: 6, authentic: true }))
    .then(beginFetch).then(check(outsideRightsDenied))
  run rightsBoundedIssueTest = init.then(clock(4)).then(authorize)
    .then(check(outcome == Issued and heldGrant(state.held).expires == 5))
  run rightsExpiredIssueTest = init.then(clock(5)).then(authorize).then(check(authorizationDenied))
  run noSilentSwapTest = init.then(authorize).then(publishOther).then(beginFetch).then(completeFetch)
    .then(check(servedAfterRepresentativeChange and noSilentSwap))
  run clockRollbackTest = init.then(authorize).then(clock(0)).then(beginFetch)
    .then(check(unboundedDenied)).then(authorize).then(check(authorizationDenied))
}
