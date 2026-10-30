module viewer_access_tests {
 import viewer_access.* from "./viewer_access"
 action check(p:bool):bool = all {assert(p),state'=state,before'=state,last'=Assert}
 run workAndUnitTest = init.then(claim).then(childOnly).then(check(not(state.shown)))
   .then(publish).then(check(admitted and workAndUnit)).then(expire).then(check(not(state.shown)))
 run bindingStableTest = init.then(claim).then(publish).then(reuseMailbox).then(claim)
   .then(check(reusedBound and bindingStable and not(state.shown)))
 run revokedStickyTest = init.then(claim).then(publish).then(revoke).then(replay)
   .then(check(replayAfterRevoke and revokedSticky and not(state.shown)))
 run groupIsolationTest = init.then(foreignGroup).then(check(foreignDenied and groupIsolation))
   .then(ownGroup).then(check(state.shown)).then(disable).then(check(not(state.shown)))
}
