module sample_metadata_tests {
  import sample_metadata.* from "./sample_metadata"
  action check(p: bool): bool = all { assert(p), state' = state }
  run explicitDisclosuresTest = init.then(scope(true)).then(work(true)).then(unit(true))
    .then(check(sampleShown and not(state.playable))).then(unit(false))
    .then(check(noUnitHidden))
  run outsideScopeTest = init.then(work(true)).then(unit(true))
    .then(check(not(state.visible)))
  run inactiveTest = init.then(scope(true)).then(work(true)).then(unit(true))
    .then(active(false)).then(check(inactiveHidden))
  run eligibleReleaseTest = init.then(release(true)).then(check(releasePlayable))
    .then(release(false)).then(check(not(state.playable)))
  run seededWithdrawalTest = init.then(confirmSource).then(confirmRights).then(seedPublish)
    .then(check(seedAttested and seedCannotLogin)).then(seedWithdraw)
    .then(check(seedRevoked and seedWithdrawalPermanent))
}
