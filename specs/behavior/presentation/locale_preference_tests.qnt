module locale_preference_tests {
  import locale_preference.* from "./locale_preference"
  action check(predicate: bool): bool = all {
    assert(predicate), state' = state, before' = state, last' = Assertion
  }
  run savedWinsTest = init.then(reopen(Saved(English), Vietnamese, Dark)).then(check(restoredSaved))
  run fallbackTest = init.then(reopen(Corrupt, English, Dark)).then(check(state.locale == English and state.theme == Dark))
    .then(reopen(Unavailable, English, Light)).then(check(state.locale == English))
    .then(reopen(Unset, Vietnamese, Light)).then(check(state == initial))
  run refusedRetryTest = init.then(setLocale(English, false)).then(check(refusedEnglish and rejectionAtomic))
    .then(setLocale(English, true)).then(check(state.locale == English and state.notice == Applied))
  run themeAfterFailureTest = init.then(setLocale(English, false)).then(setTheme(Dark))
    .then(check(themeAfterFailure and themeIsolated and state.locale == Vietnamese))
  run resetRefusedTest = init.then(setLocale(English, true)).then(setTheme(Dark)).then(reset(false))
    .then(check(rejectionAtomic and state.locale == English and state.theme == Dark))
  run resetAppliedTest = init.then(setLocale(English, true)).then(setTheme(Dark)).then(reset(true))
    .then(check(resetApplied and acceptedRequested))
}
