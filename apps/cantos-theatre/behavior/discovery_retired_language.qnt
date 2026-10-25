// Draft #424 expectations for Theatre Web discovery links once the content-language filter is
// retired: the interface locale is never a catalogue filter, and an older link's `language` is
// dropped from the applied search and from the address by replacing the current entry in place.
// One document's same-tab history; text and kind are one abstract search class. URL decoding,
// canonical encoding, History API timing, focus/scroll and browser acceptance stay native tests.
module discovery_retired_language {
  type Locale = Vietnamese | English
  type Search = Unfiltered | Searched
  type Lang = NoLang | Vi | En
  type Link = { search: Search, language: Lang }
  type Event = NoEvent | Assertion | Arrive(Link) | Follow(Link) | Submit(Search)
    | Back | Forward | SetLocale(Locale)
  type Outcome = NoOutcome | Changed
  type State = { history: List[Link], index: int, applied: Link, locale: Locale }
  pure val home: Link = { search: Unfiltered, language: NoLang }
  pure val initial: State = { history: [home], index: 0, applied: home, locale: Vietnamese }
  // Bounded history: at most four entries in one document.
  pure val limit = 4
  pure def settle(link: Link): Link = { ...link, language: NoLang }
  // The address after the shell reads the current entry: replaced in place only when it still
  // carried the retired filter, so a settled entry is never rewritten again.
  pure def rewritten(history: List[Link], index: int): List[Link] =
    if (history[index] == settle(history[index])) history
    else history.replaceAt(index, settle(history[index]))
  pure def read(history: List[Link], index: int, locale: Locale): State =
    { history: rewritten(history, index), index: index, applied: settle(history[index]), locale: locale }
  var state: State
  var before: State
  var last: Event
  var outcome: Outcome
  action init = all { state' = initial, before' = initial, last' = NoEvent, outcome' = NoOutcome }
  action record(successor: State, event: Event): bool = all {
    state' = successor, before' = state, last' = event, outcome' = Changed
  }
  // A new document at `link`: a deep link, a reload or a cross-document Back. The persisted
  // interface locale is an input fact that survives it.
  action arrive(link: Link): bool = record(read([link], 0, state.locale), Arrive(link))
  // The browser pushes a pasted or clicked link (hashchange); the shell then reads it.
  action follow(link: Link): bool = all {
    state.index + 2 <= limit,
    record(read(state.history.slice(0, state.index + 1).append(link), state.index + 1, state.locale), Follow(link))
  }
  // Enter, Tìm, a kind chip or Clear (Unfiltered): the form's own search, pushed once unless the
  // address already is that link.
  action submit(search: Search): bool = {
    val link = { search: search, language: NoLang }
    if (state.history[state.index] == link) record({ ...state, applied: link }, Submit(search))
    else all {
      state.index + 2 <= limit,
      record({ ...state, history: state.history.slice(0, state.index + 1).append(link),
        index: state.index + 1, applied: link }, Submit(search))
    }
  }
  action back = all { state.index > 0, record(read(state.history, state.index - 1, state.locale), Back) }
  action forward = all {
    state.index + 1 < state.history.length(),
    record(read(state.history, state.index + 1, state.locale), Forward)
  }
  // The topbar's interface locale.
  action setLocale(locale: Locale): bool = record({ ...state, locale: locale }, SetLocale(locale))
  action step = {
    nondet search = Set(Unfiltered, Searched).oneOf()
    nondet language = Set(NoLang, Vi, En).oneOf()
    nondet locale = Set(Vietnamese, English).oneOf()
    any {
      arrive({ search: search, language: language }), follow({ search: search, language: language }),
      submit(search), back, forward, setLocale(locale)
    }
  }
  // No applied search and no entry of this document's history keeps the retired filter.
  val noHiddenFilter = state.applied.language == NoLang
    and state.history.foldl(true, (settled, link) => settled and link.language == NoLang)
  // The current address says exactly what the page applies, so reading it again changes nothing.
  val addressShowsApplied = state.history[state.index] == state.applied
  // Dropping the filter replaces the entry: only the browser's own push or a new search adds one.
  val rewriteInPlace = match last {
    | Arrive(_) => state.history.length() == 1
    | Follow(_) => state.history.length() == before.index + 2
    | Submit(search) => state.history.length() ==
      (if (before.history[before.index] == { search: search, language: NoLang }) before.history.length()
      else before.index + 2)
    | Back => state.history.length() == before.history.length()
    | Forward => state.history.length() == before.history.length()
    | _ => true
  }
  // The interface locale is not a content-language filter: changing it leaves the search and
  // address alone, and a submitted search carries only the form's own search.
  val localeIsolated = match last {
    | SetLocale(_) => state.applied == before.applied and state.history == before.history
      and state.index == before.index
    | Submit(search) => state.applied == { search: search, language: NoLang }
    | _ => true
  }
  val retiredLinkSettled = last == Arrive({ search: Searched, language: En })
    and state.history == [{ search: Searched, language: NoLang }]
  val backToRewritten = last == Back and before.applied == home
    and state.applied == { search: Searched, language: NoLang }
  val localeAfterSearch = last == SetLocale(English) and before.locale == Vietnamese
    and state.applied.search == Searched
}
