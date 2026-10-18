// Draft #253 owner-selected rotation expectations. One API family, finite generations/time.
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

  // Native shell history is a separate bounded projection of the SAME lifecycle capability.
  // Actor 1/2 and intent 0..3 are equality classes, not account IDs or actual persisted counters.
  // Journal writes/server results are supplied facts. The model does not perform platform I/O.
  type NativeJournal = NativeActive | NativeRefreshing | NativeCleared
  type NativeState = { actor: int, intent: int, admitted: bool, journal: NativeJournal,
    barrier: bool, cleanup: bool, inFlight: bool, pendingPair: bool, calls: int }
  type NativeEvent = NoNativeEvent | NativeRestore | NativeConfirm({ actor: int, intent: int, accepted: bool })
    | NativeStart(bool) | NativeResult({ actor: int, intent: int, known: bool }) | NativePersist(bool)
    | NativeRestart | NativeLogout | NativeCleanup(bool) | NativeClearBarrier(bool) | NativeAuthorize(int)
  type NativeOutcome = NoNativeOutcome | RestoreCandidate | NativeEstablished | RefreshStarted | JoinedFlight
    | JournalRefused | PairReceived | UnknownRefresh | PairRefused | CleanupPending | CleanupAcknowledged
    | BarrierCleared | StaleIgnored | NativeDenied
  pure val nativeInitial: NativeState = { actor: 1, intent: 0, admitted: false, journal: NativeActive,
    barrier: false, cleanup: false, inFlight: false, pendingPair: false, calls: 0 }
  var native: NativeState
  var beforeNative: NativeState
  var nativeLast: NativeEvent
  var nativeOutcome: NativeOutcome

  action init = all { state' = initial, before' = initial, last' = NoEvent, outcome' = NoOutcome,
    native' = nativeInitial, beforeNative' = nativeInitial, nativeLast' = NoNativeEvent, nativeOutcome' = NoNativeOutcome }
  action record(successor: State, event: Event, result: Outcome): bool = all {
    state' = successor, before' = state, last' = event, outcome' = result,
    native' = native, beforeNative' = native, nativeLast' = NoNativeEvent, nativeOutcome' = NoNativeOutcome
  }
  action refresh(presented: int): bool =
    if (state.revoked or not(state.unexpired)) record(state, Refresh(presented), Denied)
    else if (presented < state.generation) record({ ...state, revoked: true }, Refresh(presented), ReuseRevoked)
    else if (presented == state.generation and state.generation < 3)
      record({ ...state, generation: state.generation + 1 }, Refresh(presented), Rotated)
    else record(state, Refresh(presented), Denied)
  action logout = record({ ...state, revoked: true }, Logout, Changed)
  action expire = record({ ...state, unexpired: false }, Expire, Changed)
  action nativeRecord(successor: NativeState, event: NativeEvent, result: NativeOutcome): bool = all {
    native' = successor, beforeNative' = native, nativeLast' = event, nativeOutcome' = result,
    state' = state, before' = state, last' = NoEvent, outcome' = NoOutcome
  }
  def nativeMatches(actor: int, intent: int): bool = actor == native.actor and intent == native.intent
  action nativeRestore =
    if (native.admitted or native.inFlight or native.pendingPair) nativeRecord(native, NativeRestore, NativeDenied)
    else if (native.barrier or native.journal == NativeRefreshing)
      nativeRecord({ ...native, admitted: false, cleanup: true, barrier: true }, NativeRestore, CleanupPending)
    else nativeRecord({ ...native, admitted: false }, NativeRestore, RestoreCandidate)
  action nativeConfirm(actor: int, intent: int, accepted: bool): bool = {
    val event = NativeConfirm({ actor: actor, intent: intent, accepted: accepted })
    if (not(nativeMatches(actor, intent))) nativeRecord(native, event, StaleIgnored)
    else if (native.journal == NativeActive and not(native.barrier) and not(native.cleanup))
      if (accepted) nativeRecord({ ...native, admitted: true }, event, NativeEstablished)
      else nativeRecord({ ...native, admitted: false, cleanup: true, barrier: true }, event, CleanupPending)
    else nativeRecord(native, event, NativeDenied)
  }
  action nativeStart(written: bool): bool =
    if (native.inFlight) nativeRecord(native, NativeStart(written), JoinedFlight)
    else if (native.admitted and not(native.cleanup) and not(native.pendingPair) and native.calls < 2)
      if (written) nativeRecord({ ...native, journal: NativeRefreshing, barrier: true,
        inFlight: true, calls: native.calls + 1 }, NativeStart(written), RefreshStarted)
      else nativeRecord({ ...native, admitted: false, cleanup: true, barrier: true }, NativeStart(written), JournalRefused)
    else nativeRecord(native, NativeStart(written), NativeDenied)
  action nativeResult(actor: int, intent: int, known: bool): bool = {
    val event = NativeResult({ actor: actor, intent: intent, known: known })
    if (not(nativeMatches(actor, intent))) nativeRecord(native, event, StaleIgnored)
    else if (not(native.inFlight)) nativeRecord(native, event, NativeDenied)
    else if (known) nativeRecord({ ...native, inFlight: false, pendingPair: true }, event, PairReceived)
    else nativeRecord({ ...native, admitted: false, inFlight: false, pendingPair: false,
      cleanup: true, barrier: true }, event, UnknownRefresh)
  }
  action nativePersist(written: bool): bool =
    if (native.pendingPair)
      if (written) nativeRecord({ ...native, admitted: true, journal: NativeActive, barrier: false,
        pendingPair: false }, NativePersist(written), NativeEstablished)
      else nativeRecord({ ...native, admitted: false, cleanup: true, barrier: true,
        pendingPair: false }, NativePersist(written), PairRefused)
    else nativeRecord(native, NativePersist(written), NativeDenied)
  action nativeRestart = {
    val intent = if (native.intent < 3) native.intent + 1 else 3
    nativeRecord({ ...native, intent: intent, admitted: false, inFlight: false, pendingPair: false,
      cleanup: native.barrier or native.journal == NativeRefreshing }, NativeRestart, RestoreCandidate)
  }
  action nativeLogout = {
    val intent = if (native.intent < 3) native.intent + 1 else 3
    nativeRecord({ ...native, intent: intent, admitted: false,
      barrier: true, cleanup: true, inFlight: false, pendingPair: false }, NativeLogout, CleanupPending)
  }
  action nativeCleanup(accepted: bool): bool =
    if (native.cleanup and native.barrier)
      if (accepted) nativeRecord({ ...native, journal: NativeCleared }, NativeCleanup(accepted), CleanupAcknowledged)
      else nativeRecord(native, NativeCleanup(accepted), CleanupPending)
    else nativeRecord(native, NativeCleanup(accepted), NativeDenied)
  action nativeClearBarrier(written: bool): bool =
    if (native.cleanup and native.journal == NativeCleared)
      if (written) nativeRecord({ ...native, barrier: false, cleanup: false }, NativeClearBarrier(written), BarrierCleared)
      else nativeRecord(native, NativeClearBarrier(written), CleanupPending)
    else nativeRecord(native, NativeClearBarrier(written), NativeDenied)
  // A fresh API-confirmed exchange + durable full-pair write is one supplied external fact.
  // Its HTTP/PKCE/expiry/byte validation and actual write are independently native-tested.
  action nativeAuthorize(actor: int): bool =
    if (native.journal == NativeCleared and not(native.barrier) and not(native.cleanup) and native.intent < 3)
      nativeRecord({ ...native, actor: actor, intent: native.intent + 1, admitted: true,
        journal: NativeActive }, NativeAuthorize(actor), NativeEstablished)
    else nativeRecord(native, NativeAuthorize(actor), NativeDenied)
  // Bias finite simulation toward non-vacuous journal/late-result states. This is not a
  // fairness or liveness assumption: arbitrary rejected/interrupted events remain in step.
  action nativeGuided = {
    nondet accepted = Set(false, true).oneOf()
    if (native.cleanup and native.journal != NativeCleared) nativeCleanup(true)
    else if (native.cleanup) nativeClearBarrier(true)
    else if (native.journal == NativeCleared) nativeAuthorize(2)
    else if (not(native.admitted)) nativeConfirm(native.actor, native.intent, true)
    else if (native.inFlight) any { nativeStart(true), nativeResult(native.actor, native.intent, true),
      nativeResult(native.actor, native.intent, false), nativeRestart, nativeLogout }
    else if (native.pendingPair) nativePersist(accepted)
    else any { nativeStart(true), nativeLogout, nativeResult(1, 0, true) }
  }
  action step = {
    nondet presented = 0.to(3).oneOf()
    nondet actor = Set(1, 2).oneOf()
    nondet accepted = Set(false, true).oneOf()
    any { refresh(presented), logout, expire, nativeGuided,
      any { nativeRestore, nativeConfirm(native.actor, native.intent, accepted), nativeConfirm(actor, presented, accepted),
        nativeStart(accepted), nativeResult(native.actor, native.intent, accepted), nativeResult(actor, presented, accepted),
        nativePersist(accepted), nativeRestart, nativeLogout, nativeCleanup(accepted), nativeClearBarrier(accepted), nativeAuthorize(actor) } }
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

  val nativeJournalBeforeRefresh = match nativeLast {
    | NativeStart(_) => native.calls == beforeNative.calls or
      (native.journal == NativeRefreshing and native.barrier and native.inFlight)
    | _ => true
  }
  val nativeSingleFlight = match nativeLast {
    | NativeStart(_) => not(beforeNative.inFlight) or (native == beforeNative and nativeOutcome == JoinedFlight)
    | _ => true
  }
  val nativeUnknownCleanupOnly = match nativeLast {
    | NativeResult(response) => response.known or not(beforeNative.inFlight) or
      not(response.actor == beforeNative.actor and response.intent == beforeNative.intent) or
      (native.calls == beforeNative.calls and not(native.admitted) and native.cleanup and native.barrier)
    | _ => true
  }
  val nativePersistBeforeAdmission = match nativeLast {
    | NativePersist(written) => not(beforeNative.pendingPair) or written or
      (not(native.admitted) and native.cleanup and native.barrier)
    | _ => true
  }
  val nativeRestartNeedsConfirmation = match nativeLast {
    | NativeRestart => not(native.admitted) and not(native.inFlight) and
      (not(beforeNative.barrier or beforeNative.journal == NativeRefreshing) or native.cleanup)
    | _ => true
  }
  val nativeLateResultFenced = match nativeLast {
    | NativeResult(response) => (response.actor == beforeNative.actor and response.intent == beforeNative.intent) or
      (native == beforeNative and nativeOutcome == StaleIgnored)
    | NativeConfirm(response) => (response.actor == beforeNative.actor and response.intent == beforeNative.intent) or
      (native == beforeNative and nativeOutcome == StaleIgnored)
    | _ => true
  }
  val nativeMarkerBeforeBarrierClear = match nativeLast {
    | NativeClearBarrier(_) => native.barrier or not(beforeNative.barrier) or beforeNative.journal == NativeCleared
    | _ => true
  }
  val nativeRefreshDispatched = nativeOutcome == RefreshStarted and native.calls > 0 and native.journal == NativeRefreshing
  val nativeJoined = nativeOutcome == JoinedFlight and native.calls == beforeNative.calls and native.inFlight
  val nativeUnknownStopped = nativeOutcome == UnknownRefresh and native.cleanup and not(native.admitted)
  val nativePairWriteRefused = nativeOutcome == PairRefused and native.cleanup and not(native.admitted)
  val nativeInterrupted = nativeLast == NativeRestart and native.cleanup and native.journal == NativeRefreshing and not(native.inFlight)
  val nativeLateIgnored = nativeOutcome == StaleIgnored and native.admitted and native.actor == 2
  val nativeDurableCleanup = nativeOutcome == BarrierCleared and native.journal == NativeCleared and not(native.barrier)
}
