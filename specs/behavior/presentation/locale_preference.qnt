// Draft #169 expectations for the bounded existing preference controller contract.
// No catalog/library adoption, OS negotiation, disk durability or UI/device acceptance.
module locale_preference {
  type Locale = Vietnamese | English
  type Theme = Light | Dark
  type Read = Unset | Corrupt | Unavailable | Saved(Locale)
  type Notice = Applied | StoreUnavailable
  type Event = NoEvent | Assertion | Reopen({ read: Read, seed: Locale, theme: Theme })
    | SetLocale({ locale: Locale, accepted: bool }) | SetTheme(Theme) | Reset(bool)
  type State = { locale: Locale, theme: Theme, notice: Notice }
  pure def restore(read: Read, seed: Locale): Locale = match read {
    | Saved(locale) => locale
    | Unset => seed
    | Corrupt => seed
    | Unavailable => seed
  }
  pure val initial: State = { locale: Vietnamese, theme: Light, notice: Applied }
  var state: State
  var before: State
  var last: Event
  action init = all { state' = initial, before' = initial, last' = NoEvent }
  action record(next: State, event: Event): bool = all {
    state' = next, before' = state, last' = event
  }
  // Read results/host seed are input facts. No model action proves a native store persisted.
  action reopen(read: Read, seed: Locale, theme: Theme): bool =
    record({ locale: restore(read, seed), theme: theme, notice: Applied }, Reopen({ read: read, seed: seed, theme: theme }))
  action setLocale(locale: Locale, accepted: bool): bool =
    record(if (accepted) { ...state, locale: locale, notice: Applied }
      else { ...state, notice: StoreUnavailable }, SetLocale({ locale: locale, accepted: accepted }))
  action setTheme(theme: Theme): bool = record({ ...state, theme: theme }, SetTheme(theme))
  action reset(accepted: bool): bool =
    record(if (accepted) initial else { ...state, notice: StoreUnavailable }, Reset(accepted))
  action step = {
    nondet locale = Set(Vietnamese, English).oneOf()
    nondet theme = Set(Light, Dark).oneOf()
    nondet accepted = Set(false, true).oneOf()
    nondet read = Set(Unset, Corrupt, Unavailable, Saved(Vietnamese), Saved(English)).oneOf()
    any { reopen(read, locale, theme), setLocale(locale, accepted), setTheme(theme), reset(accepted) }
  }
  val rejectionAtomic = match last {
    | SetLocale(change) => change.accepted or (state.locale == before.locale and state.theme == before.theme and state.notice == StoreUnavailable)
    | Reset(accepted) => accepted or (state.locale == before.locale and state.theme == before.theme and state.notice == StoreUnavailable)
    | _ => true
  }
  val themeIsolated = match last {
    | SetTheme(_) => state.locale == before.locale and state.notice == before.notice
    | _ => true
  }
  val acceptedRequested = match last {
    | SetLocale(change) => not(change.accepted) or (state.locale == change.locale and state.theme == before.theme and state.notice == Applied)
    | Reset(accepted) => not(accepted) or state == initial
    | _ => true
  }
  val restorationPrecedence = match last {
    | Reopen(boot) => state.theme == boot.theme and (match boot.read {
      | Saved(locale) => state.locale == locale
      | _ => state.locale == boot.seed
    })
    | _ => true
  }
  val restoredSaved = last == Reopen({ read: Saved(English), seed: Vietnamese, theme: Dark }) and state.locale == English
  val refusedEnglish = last == SetLocale({ locale: English, accepted: false }) and state.locale == Vietnamese and state.notice == StoreUnavailable
  val themeAfterFailure = last == SetTheme(Dark) and state.notice == StoreUnavailable
  val resetApplied = last == Reset(true) and state == initial
}
