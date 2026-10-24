module discovery_retired_language_tests {
  import discovery_retired_language.* from "./discovery_retired_language"
  action check(predicate: bool): bool = all {
    assert(predicate), state' = state, before' = state, last' = Assertion, outcome' = NoOutcome
  }
  pure val searched: Link = { search: Searched, language: NoLang }
  run retiredDeepLinkTest = init.then(arrive({ search: Searched, language: Vi }))
    .then(check(state.history == [searched] and state.applied == searched
      and noHiddenFilter and addressShowsApplied and rewriteInPlace))
    .then(arrive({ search: Searched, language: En })).then(check(retiredLinkSettled))
  run languageOnlyTest = init.then(arrive({ search: Unfiltered, language: En }))
    .then(check(state.history == [home] and state.applied == home and rewriteInPlace))
  run backForwardTest = init.then(follow({ search: Searched, language: Vi }))
    .then(check(state.history == [home, searched] and state.index == 1 and rewriteInPlace))
    .then(submit(Unfiltered)).then(check(state.history == [home, searched, home] and state.applied == home))
    .then(back).then(check(backToRewritten and rewriteInPlace and addressShowsApplied and state.index == 1))
    .then(forward).then(check(state.applied == home and state.index == 2 and rewriteInPlace))
    .then(submit(Unfiltered)).then(check(state.history.length() == 3 and rewriteInPlace))
  run localeIsolatedTest = init.then(arrive({ search: Searched, language: Vi }))
    .then(setLocale(English)).then(check(localeAfterSearch and localeIsolated and state.applied == searched))
    .then(submit(Searched)).then(check(localeIsolated and state.applied == searched and state.history == [searched]))
    .then(submit(Unfiltered)).then(check(localeIsolated and noHiddenFilter and state.applied == home))
}
