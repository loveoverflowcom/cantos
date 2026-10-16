// One immutable public publication delivered in origin mode (proposed ADR 0032).
// Authority: theatre-discovery-v1.md "Origin-mode media" and the playback_grant kernel.
// Synthetic clock/rights values are equality/ordering classes, not real milliseconds.
module public_playback_delivery {
  type Denial = Expired | Unbounded | NotCurrent | Forged | OutsideRights
  type Admission = Admitted | Rejected(Denial)
  // A presented grant: its claimed expiry and whether its MAC authenticates the
  // publication's sealed identity (tampering or another key makes it false).
  type Grant = { expires: int, authentic: bool }
  type Held = NoGrant | Holding(Grant)
  // Opening is shell history between admission and the post-open recheck.
  type Pending = Idle | Opening(Grant)
  type Outcome = NoOutcome | Issued | AuthorizationDenied | Opened | Served(int)
    | Denied(Denial) | FactsChanged
  type Event = NoEvent | Assertion | Authorize | Present(Grant) | BeginFetch | CompleteFetch
    | Withdraw | PublishOther | Clock(int)
  type State = {
    now: int, eligible: bool, representative: int,
    held: Held, pending: Pending, served: int
  }

  // The publication every grant in this instance names; another unit's release is 2.
  pure val pinned = 1
  pure val publishedAt = 1
  pure val rightsUntil = 5
  pure val ttl = 2

  pure def min2(a: int, b: int): int = if (a < b) a else b
  pure def pendingGrant(p: Pending): Grant = match p {
    | Idle => { expires: 0, authentic: false }
    | Opening(g) => g
  }
  pure def heldGrant(h: Held): Grant = match h {
    | NoGrant => { expires: 0, authentic: false }
    | Holding(g) => g
  }
  pure def holding(h: Held): bool = match h { | NoGrant => false | Holding(_) => true }
  // ListenerPublication::resolve: eligible facts and the half-open rights window.
  pure def current(s: State): bool =
    s.eligible and publishedAt <= s.now and s.now < rightsUntil
  // Exact pure seam: playback_grant::grant_expiry.
  pure def issuedExpiry(s: State): int = min2(s.now + ttl, rightsUntil)
  // Exact pure seam: playback_grant::decide_media_fetch over a fresh resolution.
  pure def admit(s: State, g: Grant): Admission =
    if (s.now < 0 or s.now >= g.expires) Rejected(Expired)
    else if (g.expires > s.now + ttl) Rejected(Unbounded)
    else if (not(current(s))) Rejected(NotCurrent)
    else if (not(g.authentic)) Rejected(Forged)
    else if (publishedAt > s.now or g.expires > rightsUntil) Rejected(OutsideRights)
    else Admitted
  pure def denial(a: Admission): Outcome = match a {
    | Admitted => Opened
    | Rejected(d) => Denied(d)
  }

  pure val initial: State = {
    now: publishedAt, eligible: true, representative: pinned,
    held: NoGrant, pending: Idle, served: 0
  }
  var state: State
  var before: State
  var last: Event
  var outcome: Outcome

  action init = all {
    state' = initial, before' = initial, last' = NoEvent, outcome' = NoOutcome
  }
  action checkpoint(successor: State, event: Event, result: Outcome): bool = all {
    state' = successor, before' = state, last' = event, outcome' = result
  }
  // POST playback-authorizations: resolve, seal, re-resolve. Refresh is the same call.
  action authorize = {
    val ok = current(state) and issuedExpiry(state) > state.now
    val successor = if (ok)
      { ...state, held: Holding({ expires: issuedExpiry(state), authentic: true }) }
      else state
    checkpoint(successor, Authorize, if (ok) Issued else AuthorizationDenied)
  }
  // Any token a client can present: tampered, foreign-key or over-long grants.
  action present(g: Grant): bool =
    checkpoint({ ...state, held: Holding(g) }, Present(g), FactsChanged)
  action beginFetch = {
    val g = heldGrant(state.held)
    val admission = admit(state, g)
    val successor = if (admission == Admitted) { ...state, pending: Opening(g) } else state
    all {
      state.pending == Idle, holding(state.held),
      checkpoint(successor, BeginFetch, denial(admission))
    }
  }
  // Object-open I/O finished; the same grant is re-admitted from a fresh read.
  action completeFetch = {
    val g = pendingGrant(state.pending)
    val recheck = admit(state, g)
    val result = if (recheck == Admitted) Served(pinned) else denial(recheck)
    val successor = if (recheck == Admitted)
      { ...state, pending: Idle, served: state.served + 1 }
      else { ...state, pending: Idle }
    all { state.pending != Idle, checkpoint(successor, CompleteFetch, result) }
  }
  // Withdrawal, submission revocation, supersession and owner disablement all remove
  // current eligibility; none is reversed for this immutable release.
  action withdraw = checkpoint({ ...state, eligible: false }, Withdraw, FactsChanged)
  // Another unit's release becomes the Work representative; this release stays eligible.
  action publishOther = checkpoint({ ...state, representative: 2 }, PublishOther, FactsChanged)
  // Harness time partitions include rollback; no production clock guarantee.
  action clock(now: int): bool = all {
    0 <= now and now <= 6,
    checkpoint({ ...state, now: now }, Clock(now), FactsChanged)
  }
  action sampledInputs = {
    nondet expires = 0.to(8).oneOf()
    nondet authentic = Set(true, false).oneOf()
    nondet now = 0.to(6).oneOf()
    any {
      present({ expires: expires, authentic: authentic }),
      withdraw, publishOther, clock(now)
    }
  }
  // Bias toward the issue/open/complete path and the rights-end boundary probe, without
  // removing any modeled input.
  action step = any {
    authorize, beginFetch, completeFetch,
    present({ expires: rightsUntil + 1, authentic: true }), clock(rightsUntil - 1), sampledInputs
  }

  val servedGrant = pendingGrant(before.pending)
  val isServed = match outcome { | Served(_) => true | _ => false }
  // Bytes leave only after a post-open recheck saw the release currently eligible.
  val servedOnlyCurrent = not(isServed) or
    (last == CompleteFetch and before.pending != Idle and before.eligible and
      publishedAt <= before.now and before.now < rightsUntil)
  // Every served fetch lies inside its grant: unexpired and never beyond the TTL.
  val servedWithinGrant = not(isServed) or
    (0 <= before.now and before.now < servedGrant.expires and
      servedGrant.expires <= before.now + ttl and servedGrant.expires <= rightsUntil)
  // Issued grants are strictly future and bounded by the TTL and rights window.
  val grantBounded = outcome != Issued or
    (current(before) and heldGrant(state.held).authentic and
      before.now < heldGrant(state.held).expires and
      heldGrant(state.held).expires <= before.now + ttl and
      heldGrant(state.held).expires <= rightsUntil)
  // A grant whose MAC does not authenticate the sealed identity never yields bytes.
  val forgeryRejected = not(isServed) or servedGrant.authentic
  // The served release is exactly the one the grant names: no silent representative swap.
  val noSilentSwap = match outcome { | Served(id) => id == pinned | _ => true }

  val served = outcome == Served(pinned)
  val refreshed = last == Authorize and outcome == Issued and holding(before.held)
  val expiredDenied = outcome == Denied(Expired)
  val unboundedDenied = outcome == Denied(Unbounded)
  val withdrawnDenied = last == BeginFetch and outcome == Denied(NotCurrent) and not(before.eligible)
  val recheckDenied = last == CompleteFetch and outcome == Denied(NotCurrent)
  val forgedDenied = outcome == Denied(Forged)
  val outsideRightsDenied = outcome == Denied(OutsideRights)
  val authorizationDenied = outcome == AuthorizationDenied
  val servedAfterRepresentativeChange = served and before.representative != pinned
}
