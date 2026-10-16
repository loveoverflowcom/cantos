// Draft executable expectations for #199's explicit one-app deployment boundary.
// Authority: ADR 0012 and the reviewed #199 request, not live Hosting acceptance.
// One run/one abstract selected bundle; file integrity, CLI parsing, Firebase permissions,
// URLs, clocks and actual remote effects remain native tests/operator acceptance.
module hosting_deploy {
  type Remote = NotStarted | Pending | Confirmed | Rejected | Unknown
  type Smoke = NotRun | Passed | Failed
  type Event = NoEvent | Assertion | Prepare | Intent | Fire | Finish | SmokeCheck | Persist
  type State = {
    ready: bool, intent: bool, stopped: bool, attempts: int,
    remote: Remote, smoke: Smoke
  }
  pure val initial: State = {
    ready: false, intent: false, stopped: false, attempts: 0,
    remote: NotStarted, smoke: NotRun
  }
  var state: State
  var before: State
  var last: Event
  action init = all { state' = initial, before' = initial, last' = NoEvent }
  action record(successor: State, event: Event): bool = all {
    state' = successor, before' = state, last' = event
  }
  action prepare(valid: bool): bool = all {
    state.attempts == 0 and not(state.stopped),
    record({ ...state, ready: valid, stopped: not(valid) }, Prepare)
  }
  action persistIntent(success: bool): bool = all {
    state.ready and state.attempts == 0 and not(state.stopped),
    record({ ...state, intent: success, stopped: not(success) }, Intent)
  }
  action fire = all {
    state.ready and state.intent and not(state.stopped) and state.attempts == 0,
    record({ ...state, attempts: state.attempts + 1, remote: Pending }, Fire)
  }
  action finish(outcome: Remote): bool = all {
    state.remote == Pending,
    Set(Confirmed, Rejected, Unknown).contains(outcome),
    record({ ...state, remote: outcome }, Finish)
  }
  action smoke(success: bool): bool = all {
    state.remote == Confirmed and state.smoke == NotRun,
    record({ ...state, smoke: if (success) Passed else Failed }, SmokeCheck)
  }
  // A failed local outcome receipt cannot erase a known remote effect or authorize retry.
  action persistOutcome = record(state, Persist)
  action step = any {
    prepare(true), prepare(false), persistIntent(true), persistIntent(false), fire,
    finish(Confirmed), finish(Rejected), finish(Unknown), smoke(true), smoke(false), persistOutcome
  }
  val writeOnlyPrepared = state.attempts == 0 or (state.ready and state.intent and not(state.stopped))
  val atMostOnce = state.attempts <= 1
  val confirmedSticky = before.remote != Confirmed or state.remote == Confirmed
  val preparedWrite = state.attempts == 1 and state.remote == Pending
  val unknownOutcome = state.remote == Unknown
  val confirmedSmokeFailure = state.remote == Confirmed and state.smoke == Failed
}
