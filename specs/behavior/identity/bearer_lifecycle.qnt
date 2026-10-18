// Draft #253 owner-selected rotation expectations. One family, finite generations/time.
// No cryptography, SQL isolation, provider, HTTP, browser or secure-storage conformance claim.
module bearer_lifecycle {
  type Event = NoEvent | Assertion | Refresh(int) | Logout | Expire
  type Outcome = NoOutcome | Rotated | ReuseRevoked | Denied | Changed
  type State = { generation: int, revoked: bool, unexpired: bool, absolute: int, authTime: int }
  pure val initial: State = { generation: 0, revoked: false, unexpired: true, absolute: 3, authTime: 0 }
  var state: State
  var before: State
  var last: Event
  var outcome: Outcome
  action init = all { state' = initial, before' = initial, last' = NoEvent, outcome' = NoOutcome }
  action record(successor: State, event: Event, result: Outcome): bool = all {
    state' = successor, before' = state, last' = event, outcome' = result
  }
  action refresh(presented: int): bool =
    if (state.revoked or not(state.unexpired)) record(state, Refresh(presented), Denied)
    else if (presented < state.generation) record({ ...state, revoked: true }, Refresh(presented), ReuseRevoked)
    else if (presented == state.generation and state.generation < 3)
      record({ ...state, generation: state.generation + 1 }, Refresh(presented), Rotated)
    else record(state, Refresh(presented), Denied)
  action logout = record({ ...state, revoked: true }, Logout, Changed)
  action expire = record({ ...state, unexpired: false }, Expire, Changed)
  action step = { nondet presented = 0.to(3).oneOf()
    any { refresh(presented), logout, expire }
  }
  val revocationSticky = match last {
    | Refresh(_) => not(before.revoked) or (state == before and outcome == Denied)
    | _ => true
  }
  val singleUse = match last {
    | Refresh(presented) => before.revoked or not(before.unexpired) or presented >= before.generation or state.revoked
    | _ => true
  }
  val boundedAuthority = state.absolute == before.absolute
  val authTimeUnchanged = state.authTime == before.authTime
  val rotated = outcome == Rotated and state.generation > 0
  val replayed = outcome == ReuseRevoked and state.revoked
  val logoutRefused = before.revoked and last == Refresh(0) and outcome == Denied
  val expiryRefused = not(before.unexpired) and last == Refresh(0) and outcome == Denied
}
