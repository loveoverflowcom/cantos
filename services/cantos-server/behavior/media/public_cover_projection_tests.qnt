module public_cover_projection_tests {
  import public_cover_projection.* from "./public_cover_projection"

  action check(predicate: bool): bool = all {
    // A named assertion is a no-effect harness checkpoint, not another edit.
    assert(predicate), state' = state, before' = state, last' = Assertion, outcome' = NoOutcome
  }

  run notPublishedTest = init.then(setPrivate(1)).then(disclose(1, 6))
    .then(check(state.view == Fallback(NotPublished)))
  run pinnedReplacementTest = init.then(setPrivate(1)).then(disclose(1, 6)).then(publish)
    .then(check(state.view == Shown(1))).then(setPrivate(2)).then(disclose(2, 6))
    .then(check(pinnedDespiteReplacement and state.view == Shown(1)))
    .then(publish).then(check(republishedReplacement and state.view == Shown(2)))
  run undisclosedTest = init.then(setPrivate(1)).then(publish).then(check(undisclosedFallback))
  run otherImageDisclosureTest = init.then(setPrivate(1)).then(disclose(2, 6)).then(publish)
    .then(check(undisclosedFallback))
  run revokeTest = init.then(setPrivate(1)).then(disclose(1, 6)).then(publish).then(revoke(1))
    .then(check(revokedFallback))
  run expiryTest = init.then(setPrivate(1)).then(disclose(1, 3)).then(publish)
    .then(check(state.view == Shown(1))).then(clock(3)).then(check(expiredFallback))
    .then(clock(2)).then(check(state.view == Shown(1)))
  run withdrawTest = init.then(setPrivate(1)).then(disclose(1, 6)).then(publish).then(withdraw)
    .then(check(withdrawnFallback and state.view == Fallback(NotPublished)))
  run clearTombstoneTest = init.then(setPrivate(1)).then(disclose(1, 6)).then(clearPrivate)
    .then(publish).then(check(clearedFallback))
  run metadataEditKeepsCoverTest = init.then(setPrivate(1)).then(disclose(1, 6)).then(editMetadata)
    .then(publish).then(check(state.view == Shown(1)))
  run coverBeforeArtworkTest = init.then(publish).then(check(state.view == Fallback(NoArtwork)))
    .then(setPrivate(1)).then(disclose(1, 6)).then(check(state.view == Fallback(NoArtwork)))

  run fetchCurrentShownTest = init.then(setPrivate(1)).then(disclose(1, 6)).then(publish)
    .then(fetch(1)).then(check(fetchServed and outcome == Served(1)))
    .then(fetch(2)).then(check(fetchMismatchDenied and outcome == FetchDenied))
  run fetchPrivateReplacementTest = init.then(setPrivate(1)).then(disclose(1, 6)).then(publish)
    .then(setPrivate(2)).then(disclose(2, 6)).then(fetch(1))
    .then(check(fetchPinnedDespiteReplacement and outcome == Served(1)))
    .then(fetch(2)).then(check(fetchMismatchDenied and outcome == FetchDenied))
  run fetchStaleRepublishTest = init.then(setPrivate(1)).then(disclose(1, 6)).then(publish)
    .then(fetch(1)).then(check(outcome == Served(1)))
    .then(setPrivate(2)).then(disclose(2, 6)).then(publish)
    .then(fetch(1)).then(check(fetchMismatchDenied and outcome == FetchDenied))
    .then(fetch(2)).then(check(fetchServed and outcome == Served(2)))
  run fetchRevokedTest = init.then(setPrivate(1)).then(disclose(1, 6)).then(publish)
    .then(fetch(1)).then(check(outcome == Served(1))).then(revoke(1))
    .then(fetch(1)).then(check(fetchFallbackDenied and revokedFallback and outcome == FetchDenied))
  run fetchExpiredTest = init.then(setPrivate(1)).then(disclose(1, 3)).then(publish)
    .then(fetch(1)).then(check(outcome == Served(1))).then(clock(3))
    .then(fetch(1)).then(check(fetchFallbackDenied and expiredFallback and outcome == FetchDenied))
  run fetchWithdrawnTest = init.then(setPrivate(1)).then(disclose(1, 6)).then(publish)
    .then(fetch(1)).then(check(outcome == Served(1))).then(withdraw)
    .then(fetch(1)).then(check(fetchFallbackDenied and withdrawnFallback and outcome == FetchDenied))
}
