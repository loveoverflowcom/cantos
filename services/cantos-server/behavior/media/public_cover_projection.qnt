// Accepted #157 P0 public Work cover policy (ADR 0033); model lifecycle is draft.
// Authority: the public_cover kernel and artwork-v1.md revision-bound history.
// Images 1 and 2 have distinct byte SHA identities; requests name those equality
// classes. Separate assets sharing bytes are outside this instance. Asset binding is
// exact (malformed hashes, binding/corruption and actual bytes remain native partitions).
module public_cover_projection {
  type Art = Img(int) | Cleared
  type Pin = Unpublished | Pinned(int)
  type Decision = Undecided | Disclosed(int) | Revoked
  type Reason = NotPublished | NoArtwork | Undisclosed | RevokedCover | Expired
  type View = Shown(int) | Fallback(Reason)
  type Outcome = NoOutcome | Changed | Served(int) | FetchDenied
  type Event = NoEvent | Assertion | SetPrivate(int) | ClearPrivate | EditMetadata | Publish
    | Withdraw | Disclose(int) | Revoke(int) | Clock(int) | Fetch(int)
  type State = {
    now: int, revision: int, changes: int -> Art, pin: Pin, eligible: bool,
    decisions: int -> Decision, view: View
  }

  pure val images = Set(1, 2)
  pure def max2(a: int, b: int): int = if (a > b) a else b
  // artwork::artwork_at: the newest change at or before the pinned revision; an
  // explicit clear is a tombstone that prevents resurrecting an older image.
  pure def effectiveAt(changes: int -> Art, revision: int): Art = {
    val atOrBefore = changes.keys().filter(k => k <= revision)
    if (atOrBefore == Set()) Cleared
    else changes.get(atOrBefore.fold(0, (a, b) => max2(a, b)))
  }
  // public_cover::cover_outcome after ListenerPublication eligibility.
  pure def disclosure(d: Decision, image: int, now: int): View = match d {
    | Undecided => Fallback(Undisclosed)
    | Revoked => Fallback(RevokedCover)
    | Disclosed(until) => if (now < until) Shown(image) else Fallback(Expired)
  }
  pure def coverView(s: State): View = match s.pin {
    | Unpublished => Fallback(NotPublished)
    | Pinned(r) =>
      if (not(s.eligible)) Fallback(NotPublished)
      else match effectiveAt(s.changes, r) {
        | Cleared => Fallback(NoArtwork)
        | Img(i) => disclosure(s.decisions.get(i), i, s.now)
      }
  }
  pure def project(s: State): State = { ...s, view: coverView(s) }
  // Every request re-resolves today's eligible release, disclosure and clock. The
  // requested identity may be a URL retained before a replacement or revocation.
  pure def fetchOutcome(s: State, requested: int): Outcome = match coverView(s) {
    | Shown(i) => if (i == requested) Served(i) else FetchDenied
    | Fallback(_) => FetchDenied
  }

  pure val initial: State = {
    now: 1, revision: 1, changes: Map(), pin: Unpublished, eligible: false,
    decisions: Map(1 -> Undecided, 2 -> Undecided), view: Fallback(NotPublished)
  }
  var state: State
  var before: State
  var last: Event
  var outcome: Outcome

  action init = all {
    state' = initial, before' = initial, last' = NoEvent, outcome' = NoOutcome
  }
  action checkpoint(successor: State, event: Event): bool = all {
    state' = project(successor), before' = state, last' = event, outcome' = Changed
  }
  // Owner-only private head changes: each advances the Catalog metadata revision.
  action setPrivate(i: int): bool = {
    val successor = state.revision + 1
    checkpoint({ ...state, revision: successor, changes: state.changes.put(successor, Img(i)) }, SetPrivate(i))
  }
  action clearPrivate = {
    val successor = state.revision + 1
    checkpoint({ ...state, revision: successor, changes: state.changes.put(successor, Cleared) }, ClearPrivate)
  }
  action editMetadata = checkpoint({ ...state, revision: state.revision + 1 }, EditMetadata)
  // A new approved representative release pins the current head revision.
  action publish = checkpoint({ ...state, pin: Pinned(state.revision), eligible: true }, Publish)
  // Withdrawal, supersession without replacement or visibility loss of the release.
  action withdraw = checkpoint({ ...state, eligible: false }, Withdraw)
  // Accepted #157 public-display decision for one exact image; private admission is not it.
  action disclose(i: int, until: int): bool =
    checkpoint({ ...state, decisions: state.decisions.put(i, Disclosed(until)) }, Disclose(i))
  action revoke(i: int): bool =
    checkpoint({ ...state, decisions: state.decisions.put(i, Revoked) }, Revoke(i))
  action clock(now: int): bool = all {
    0 <= now and now <= 6,
    checkpoint({ ...state, now: now }, Clock(now))
  }
  action fetch(requested: int): bool = all {
    state' = project(state), before' = state, last' = Fetch(requested),
    outcome' = fetchOutcome(state, requested)
  }
  action sampledInputs = {
    nondet i = images.oneOf()
    nondet until = 0.to(6).oneOf()
    nondet now = 0.to(6).oneOf()
    any {
      setPrivate(i), clearPrivate, editMetadata, publish, withdraw,
      disclose(i, until), revoke(i), clock(now), fetch(i)
    }
  }
  // Bias toward cover A disclosed and published before later private replacements.
  action step = any { setPrivate(1), disclose(1, 6), publish, fetch(1), sampledInputs }

  pure def pinnedRevision(p: Pin): int = match p { | Unpublished => 0 | Pinned(r) => r }
  pure def published(p: Pin): bool = match p { | Unpublished => false | Pinned(_) => true }
  pure def shownImage(v: View): int = match v { | Shown(i) => i | Fallback(_) => 0 }
  pure def isShown(v: View): bool = match v { | Shown(_) => true | Fallback(_) => false }
  // The newest change at or before r, stated independently of effectiveAt.
  pure def latestIs(changes: int -> Art, r: int, art: Art): bool =
    changes.keys().exists(k => k <= r and changes.get(k) == art and
      changes.keys().forall(j => j <= r implies j <= k))

  // A shown cover is the image selected at the active release's pinned revision.
  val shownIsPinned = not(isShown(state.view)) or
    (published(state.pin) and state.eligible and
      latestIs(state.changes, pinnedRevision(state.pin), Img(shownImage(state.view))))
  // Owner-only edits after publication never change what listeners see.
  val privateEditIsolated =
    not(match last { | SetPrivate(_) => true | ClearPrivate => true | EditMetadata => true | _ => false })
      or state.view == before.view
  pure def disclosedAt(d: Decision, now: int): bool =
    match d { | Disclosed(until) => now < until | _ => false }
  // Showing requires an explicit current disclosure of that exact image.
  val disclosureRequired =
    if (isShown(state.view)) disclosedAt(state.decisions.get(shownImage(state.view)), state.now)
    else true
  // An ineligible or unpublished release never exposes a cover.
  val withdrawnHidden = (published(state.pin) and state.eligible) or not(isShown(state.view))
  // An explicit clear at the pinned revision is never resurrected as an older image.
  val clearIsTombstone = not(published(state.pin) and state.eligible and
      latestIs(state.changes, pinnedRevision(state.pin), Cleared))
    or state.view == Fallback(NoArtwork)
  // Both directions matter: serve exactly today's shown image, and deny every
  // other request. The projection assertions above independently constrain view.
  val fetchMatchesProjection = match last {
    | Fetch(requested) =>
      if (state.view == Shown(requested)) outcome == Served(requested)
      else outcome == FetchDenied
    | _ => true
  }

  val shown = isShown(state.view)
  val pinnedDespiteReplacement = isShown(state.view) and
    latestIs(state.changes, state.revision, Img(3 - shownImage(state.view)))
  val republishedReplacement = isShown(before.view) and isShown(state.view) and
    shownImage(before.view) != shownImage(state.view)
  val withdrawnFallback = published(state.pin) and not(state.eligible)
  val revokedFallback = state.view == Fallback(RevokedCover)
  val expiredFallback = state.view == Fallback(Expired)
  val undisclosedFallback = state.view == Fallback(Undisclosed)
  val clearedFallback = state.view == Fallback(NoArtwork) and state.changes.keys().size() > 0
  val fetchServed = match last {
    | Fetch(requested) => outcome == Served(requested)
    | _ => false
  }
  val fetchMismatchDenied = match last {
    | Fetch(requested) => isShown(state.view) and shownImage(state.view) != requested and
      outcome == FetchDenied
    | _ => false
  }
  val fetchFallbackDenied = match last {
    | Fetch(_) => not(isShown(state.view)) and outcome == FetchDenied
    | _ => false
  }
  val fetchPinnedDespiteReplacement = fetchServed and pinnedDespiteReplacement
}
