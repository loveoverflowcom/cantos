// Draft executable expectation for the owner-approved #691/#692 guest entry policy.
// One document/process, finite route classes and request epochs. Session facts are inputs:
// this does not model authentication, URLs, renderer scheduling, bytes or native history.
module guest_navigation {
  type Phase = Idle | Checking | Guest | SignedIn | Failed | LogoutPending
  type Route = Landing | Discover | Library | History | Me | Work | Shared | Player | Auth | Recovery
  type Event = NoEvent | Assertion | Observe(Phase) | Navigate(Route) | OpenAuth(Route)
    | Begin | Complete(int) | Cancel | Logout | Listen | Preferences
  type State = { phase: Phase, route: Route, nav: bool, origin: Route, source: Route,
    epoch: int, pending: int, pushes: int, publicPlays: int, preferences: bool }
  pure def privateRoute(route: Route): bool = Set(Library, History, Me).contains(route)
  pure def settle(route: Route, phase: Phase): Route =
    if (phase == Guest and (privateRoute(route) or route == Discover)) Landing
    else if (phase == SignedIn and route == Landing) Discover else route
  pure def returnDestination(route: Route): Route =
    if (privateRoute(route) or Set(Work, Shared).contains(route)) route else Discover
  pure def visible(phase: Phase, route: Route): bool = phase == SignedIn and route != Auth
  pure val initial: State = { phase: Idle, route: Landing, nav: false, origin: Discover,
    source: Landing, epoch: 0, pending: 0, pushes: 0, publicPlays: 0, preferences: false }
  var state: State
  var before: State
  var last: Event
  action init = all { state' = initial, before' = initial, last' = NoEvent }
  action record(successor: State, event: Event): bool = all { state' = successor, before' = state, last' = event }
  action observe(phase: Phase): bool = {
    val route = settle(state.route, phase)
    val origin = if (state.phase == SignedIn and phase != SignedIn) Discover
      else if (phase == Guest and state.phase == LogoutPending) Discover
      else if (phase == Guest and privateRoute(state.route)) state.route else state.origin
    record({ ...state, phase: phase, route: route, nav: visible(phase, route), origin: origin,
      pending: if (Set(Guest, Failed, LogoutPending).contains(phase)) 0 else state.pending }, Observe(phase))
  }
  action navigate(request: Route): bool = all {
    state.pushes < 8,
    val route = settle(request, state.phase)
    val origin = if (state.phase == Guest and privateRoute(request)) request
      else if (route == state.route) state.origin else returnDestination(route)
    record({ ...state, route: route, nav: visible(state.phase, route), pending: 0, origin: origin,
      pushes: state.pushes + (if (route == state.route) 0 else 1) }, Navigate(request))
  }
  action openAuth(origin: Route): bool = all {
    state.phase == Guest, state.route != Auth,
    record({ ...state, route: Auth, nav: false, origin: returnDestination(origin),
      source: state.route, pending: 0 }, OpenAuth(origin))
  }
  action begin = all {
    state.phase == Guest, state.route == Auth, state.pending == 0, state.epoch < 4,
    record({ ...state, phase: Checking, epoch: state.epoch + 1, pending: state.epoch + 1 }, Begin)
  }
  action complete(ticket: int): bool = {
    val accepted = state.pending != 0 and ticket == state.pending and state.route == Auth
    val target = if (state.origin == Landing) Discover else state.origin
    if (accepted) record({ ...state, phase: SignedIn, route: target,
      nav: visible(SignedIn, target), pending: 0 }, Complete(ticket))
    else record(state, Complete(ticket))
  }
  action cancel = all {
    state.route == Auth,
    val route = settle(state.source, state.phase)
    record({ ...state, route: route, nav: visible(state.phase, route), pending: 0 }, Cancel)
  }
  action logout = all {
    state.phase == SignedIn,
    record({ ...state, phase: LogoutPending, nav: false, pending: 0, origin: Discover }, Logout)
  }
  action listen = all {
    state.publicPlays < 2,
    record({ ...state, publicPlays: state.publicPlays + 1 }, Listen)
  }
  action preferences = record({ ...state, preferences: not(state.preferences) }, Preferences)
  action step = {
    nondet phase = Set(Idle, Checking, Guest, SignedIn, Failed, LogoutPending).oneOf()
    nondet route = Set(Landing, Discover, Library, History, Me, Work, Shared, Player, Recovery).oneOf()
    nondet ticket = Set(1, 2, 3, 4).oneOf()
    any { observe(phase), navigate(route), openAuth(route), begin, complete(ticket), cancel, logout, listen, preferences }
  }
  val shellAdmission = state.nav == visible(state.phase, state.route)
  val guestSafe = state.phase != Guest or (not(privateRoute(state.route)) and state.route != Discover)
  val unresolvedIntent = match last {
    | Observe(phase) => not(Set(Idle, Checking, Failed, LogoutPending).contains(phase))
      or (state.route == before.route and not(state.nav))
    | _ => true
  }
  val authReturn = match last {
    | Complete(ticket) => before.pending == 0 or ticket != before.pending or before.route != Auth
      or (state.phase == SignedIn and state.route == (if (before.origin == Landing) Discover else before.origin))
    | Cancel => state.route == settle(before.source, before.phase) and state.pending == 0
    | _ => true
  }
  val staleIgnored = match last {
    | Complete(ticket) => (before.pending != 0 and ticket == before.pending and before.route == Auth)
      or state == before
    | _ => true
  }
  val idempotentNavigation = match last {
    | Navigate(route) => settle(route, before.phase) != before.route or state.pushes == before.pushes
    | Observe(_) => state.pushes == before.pushes
    | _ => true
  }
  val returnIntent = match last {
    | Navigate(route) => if (before.phase == Guest and privateRoute(route)) state.origin == route
      else settle(route, before.phase) != before.route or state.origin == before.origin
    | Observe(phase) => if ((before.phase == SignedIn and phase != SignedIn)
      or (phase == Guest and before.phase == LogoutPending)) state.origin == Discover
      else if (phase == Guest and privateRoute(before.route)) state.origin == before.route
      else state.origin == before.origin
    | Logout => state.origin == Discover
    | _ => true
  }
  val retainedPrivateIntent = last == Navigate(Landing) and before.phase == Guest
    and before.route == Landing and before.origin == Library and state.origin == Library
  val publicOwnersSurvive = (last == Listen or state.publicPlays == before.publicPlays)
    and (last == Preferences or state.preferences == before.preferences)
  val guestRoot = last == Observe(Guest) and before.route == Library and state.route == Landing
  val unresolvedPrivate = last == Observe(Failed) and state.route == Library and not(state.nav)
  val returnedPrivate = last == Complete(1) and state.route == Library and state.phase == SignedIn
  val cancelledLate = last == Complete(1) and before.pending == 0 and before.epoch == 1 and state == before
  val sameDestination = last == Navigate(Work) and before.route == Work and state.pushes == before.pushes
  val playingGuest = last == Observe(Guest) and state.publicPlays > 0 and state.preferences
  val signedOutPrivate = last == Observe(Guest) and before.phase == LogoutPending and before.route == Library
    and state.route == Landing and not(state.nav)
}
