// Draft executable expectation for the PROPOSED Work-level Listener Library projection
// (docs/architecture/listener-library-projection.md, decisions D-L02..D-L08). It is not the
// implemented Library v1 behavior: Saved presentation and Continue membership differ on purpose.
// One account, one Work and four Units in published order. Catalog/Media facts are inputs.
// Registers abstract the implemented reconcile kernel: no revisions, device sequences, receipts,
// cursors, clocks, bytes, sessions or clients. A Unit holds at most one progress register here.
module library_projection {
  type Listen = Playable | NoAudioYet
  type Presentation = NullPresentation | Metadata(Listen)
  type SavedRow = SavedAbsent | SavedHidden | SavedShown(Presentation)
  type LibraryAction = NoActivity | Hidden | Resume(int) | Replaced(int) | AudioUnavailable(int)
    | StartNext(int) | WaitingAudio(int) | CaughtUp
  type Event = NoEvent | Assertion | Save(bool) | Start(int) | Checkpoint(int) | Finish(int)
    | Complete(int) | ClearHistory | Publish(int) | Hide(int) | Attach(int) | Withdraw(int)
    | Expose(bool)
  // release 0 means no current eligible audio association for the published Unit.
  type UnitFact = { published: bool, release: int }
  // release 0 means no register; order is the account's accepted order of the last change.
  type Progress = { release: int, atEnd: bool, completed: bool, order: int }
  type State = { workPublic: bool, units: int -> UnitFact, saved: bool,
    progress: int -> Progress, historyUnit: int, epoch: int, clock: int, releases: int }

  pure val UNITS = Set(1, 2, 3, 4)
  pure val noProgress: Progress = { release: 0, atEnd: false, completed: false, order: 0 }
  // Chapter 2 is published as metadata only; chapter 3 already has audio; chapter 4 is unpublished.
  pure val initial: State = { workPublic: true,
    units: Map(1 -> { published: true, release: 1 }, 2 -> { published: true, release: 0 },
      3 -> { published: true, release: 2 }, 4 -> { published: false, release: 0 }),
    saved: false, progress: Map(1 -> noProgress, 2 -> noProgress, 3 -> noProgress, 4 -> noProgress),
    historyUnit: 0, epoch: 0, clock: 0, releases: 2 }

  pure def visibleUnit(s: State, u: int): bool = s.workPublic and s.units.get(u).published
  pure def listenable(s: State, u: int): bool = visibleUnit(s, u) and s.units.get(u).release > 0
  pure def anyListenable(s: State): bool = UNITS.exists(u => listenable(s, u))
  // D-L02: Saved presentation comes from the published Work metadata; audio is a separate fact.
  pure def presentationOf(s: State): Presentation =
    Metadata(if (anyListenable(s)) Playable else NoAudioYet)
  pure def savedRow(s: State): SavedRow =
    if (not(s.saved)) SavedAbsent else if (not(s.workPublic)) SavedHidden
    else SavedShown(presentationOf(s))
  // D-L03: the anchor is the latest register of the Work, completed or not.
  pure def anchor(s: State): int = UNITS.fold(0, (a, u) =>
    if (s.progress.get(u).release > 0 and (a == 0 or s.progress.get(u).order > s.progress.get(a).order)) u else a)
  // The immediately following published Unit: the smallest published position after the anchor.
  pure def followingUnit(s: State, a: int): int = UNITS.fold(0, (n, u) =>
    if (u > a and s.units.get(u).published and (n == 0 or u < n)) u else n)
  pure def libraryAction(s: State): LibraryAction = {
    val a = anchor(s)
    if (a == 0) NoActivity
    else if (not(visibleUnit(s, a))) Hidden
    else {
      val p = s.progress.get(a)
      val current = s.units.get(a).release
      if (not(p.completed)) {
        if (current == p.release) Resume(a)
        else if (current > 0) Replaced(a)
        else AudioUnavailable(a)
      } else {
        val n = followingUnit(s, a)
        if (n == 0) CaughtUp
        else if (listenable(s, n)) StartNext(n)
        else WaitingAudio(n)
      }
    }
  }
  // D-L04: Continue lists only Works with an actionable or explainable resume.
  pure def inContinue(s: State): bool = match libraryAction(s) {
    | Resume(_) => true
    | Replaced(_) => true
    | AudioUnavailable(_) => true
    | StartNext(_) => true | _ => false
  }
  pure def historyShown(s: State): bool = s.historyUnit > 0 and visibleUnit(s, s.historyUnit)
  // Independent of anchor(): the latest register exists and its Unit is no longer visible.
  pure def latestHidden(s: State): bool = UNITS.exists(u => s.progress.get(u).release > 0
    and UNITS.forall(v => s.progress.get(v).order <= s.progress.get(u).order)
    and not(visibleUnit(s, u)))

  var state: State
  var before: State
  var last: Event

  action init = all { state' = initial, before' = initial, last' = NoEvent }
  action record(successor: State, event: Event): bool = all {
    state' = successor, before' = state, last' = event
  }

  // Listener intents; bounded by the account order counter.
  action save(desired: bool): bool = all {
    state.clock < 12,
    val admitted = not(desired) or state.saved or state.workPublic
    record(if (admitted) { ...state, saved: desired, clock: state.clock + 1 } else state, Save(desired))
  }
  // An acknowledged engine start of the current exact release: history plus, for a first, new-release
  // or finished register, a fresh register at position zero (explicit choice or restart).
  action start(u: int): bool = all {
    state.clock < 12, listenable(state, u),
    val p = state.progress.get(u)
    val current = state.units.get(u).release
    val fresh = p.release != current or p.completed
    val progress = if (fresh) state.progress.set(u,
      { release: current, atEnd: false, completed: false, order: state.clock + 1 }) else state.progress
    record({ ...state, progress: progress, historyUnit: u, clock: state.clock + 1 }, Start(u))
  }
  // A checkpoint or seek of an owned register; reaching the end is not completion.
  action checkpoint(u: int, toEnd: bool): bool = all {
    state.clock < 12,
    val p = state.progress.get(u)
    all {
      p.release > 0, not(p.completed),
      val moved = state.progress.set(u, { ...p, atEnd: toEnd, order: state.clock + 1 })
      record({ ...state, progress: moved, clock: state.clock + 1 }, if (toEnd) Finish(u) else Checkpoint(u))
    }
  }
  // D-L05: mark_completed only at the exact end (explicit action or acknowledged natural end).
  action complete(u: int): bool = all {
    state.clock < 12,
    val p = state.progress.get(u)
    all {
      p.release > 0, p.atEnd, not(p.completed),
      record({ ...state, progress: state.progress.set(u, { ...p, completed: true, order: state.clock + 1 }),
        clock: state.clock + 1 }, Complete(u))
    }
  }
  action clearHistory = all {
    state.historyUnit > 0, state.epoch < 2,
    record({ ...state, historyUnit: 0, epoch: state.epoch + 1 }, ClearHistory)
  }

  // Catalog/Media facts. They never edit account registers.
  action publish(u: int): bool =
    record({ ...state, units: state.units.set(u, { ...state.units.get(u), published: true }) }, Publish(u))
  // Leaving the generation also ends the Unit's audio association.
  action hide(u: int): bool =
    record({ ...state, units: state.units.set(u, { published: false, release: 0 }) }, Hide(u))
  // A first association or a replacement release, always a new exact identity.
  action attach(u: int): bool = all {
    state.units.get(u).published, state.releases < 6,
    record({ ...state, units: state.units.set(u, { published: true, release: state.releases + 1 }),
      releases: state.releases + 1 }, Attach(u))
  }
  action withdraw(u: int): bool =
    record({ ...state, units: state.units.set(u, { ...state.units.get(u), release: 0 }) }, Withdraw(u))
  action expose(visible: bool): bool = record({ ...state, workPublic: visible }, Expose(visible))

  action step = {
    nondet u = UNITS.oneOf()
    nondet flag = Set(true, false).oneOf()
    any {
      save(flag), start(u), checkpoint(u, flag), complete(u), clearHistory,
      any { publish(u), hide(u), attach(u), withdraw(u), expose(flag) }
    }
  }

  // Properties.
  val savedPresentable = match savedRow(state) {
    | SavedShown(p) => p != NullPresentation
    | _ => true
  }
  val savedListenHonest = match savedRow(state) {
    | SavedShown(p) => match p {
      | Metadata(l) => (l == Playable) == anyListenable(state)
      | _ => true
    }
    | _ => true
  }
  val savedAdmission = match last {
    | Save(d) => if (d and not(before.saved)) state.saved == before.workPublic else state.saved == d
    | _ => true
  }
  val retainedWhenHidden = match last {
    | Expose(_) => state.saved == before.saved and state.progress == before.progress
      and state.historyUnit == before.historyUnit
    | Hide(_) => state.saved == before.saved and state.progress == before.progress
      and state.historyUnit == before.historyUnit
    | Withdraw(_) => state.saved == before.saved and state.progress == before.progress
    | _ => true
  }
  val exactResume = match libraryAction(state) {
    | Resume(u) => state.progress.get(u).release == state.units.get(u).release
      and not(state.progress.get(u).completed)
    | Replaced(u) => state.units.get(u).release > 0
      and state.units.get(u).release != state.progress.get(u).release
    | _ => true
  }
  val nextInOrder = match libraryAction(state) {
    | StartNext(n) => {
      val a = anchor(state)
      state.progress.get(a).completed and n > a and listenable(state, n)
        and UNITS.forall(v => (v > a and v < n) implies not(state.units.get(v).published))
    }
    | _ => true
  }
  val continueMembership = match libraryAction(state) {
    | StartNext(_) => inContinue(state)
    | WaitingAudio(_) => not(inContinue(state))
    | CaughtUp => not(inContinue(state))
    | Hidden => not(inContinue(state))
    | _ => true
  }
  val hiddenFailsClosed = latestHidden(state) implies not(inContinue(state))
  val noAutoComplete = match last {
    | Checkpoint(u) => state.progress.get(u).completed == before.progress.get(u).completed
    | Finish(u) => state.progress.get(u).completed == before.progress.get(u).completed
    | _ => true
  }
  val registersIndependent = match last {
    | ClearHistory => state.historyUnit == 0 and state.progress == before.progress
      and state.saved == before.saved and libraryAction(state) == libraryAction(before)
    | Save(_) => state.progress == before.progress and state.historyUnit == before.historyUnit
    | _ => true
  }
  val historyFromStart = match last {
    | Start(u) => state.historyUnit == u
    | ClearHistory => state.historyUnit == 0
    | _ => state.historyUnit == before.historyUnit
  }

  // Reachability witnesses.
  val savedNoAudio = savedRow(state) == SavedShown(Metadata(NoAudioYet))
  val metadataOnlySave = match last {
    | Save(d) => d and not(before.saved) and state.saved and not(anyListenable(before))
    | _ => false
  }
  val hiddenRetained = last == Expose(false) and before.saved and savedRow(state) == SavedHidden
    and anchor(state) > 0
  val replacedRelease = match libraryAction(state) {
    | Replaced(_) => true
    | _ => false
  }
  val waitingBeforeLater = match libraryAction(state) {
    | WaitingAudio(n) => UNITS.exists(v => v > n and listenable(state, v))
    | _ => false
  }
  val chapterReentered = match last {
    | Attach(n) => libraryAction(before) == WaitingAudio(n) and libraryAction(state) == StartNext(n)
    | _ => false
  }
  val caughtUp = libraryAction(state) == CaughtUp
  val hiddenLatest = latestHidden(state) and UNITS.exists(v => state.progress.get(v).release > 0
    and not(state.progress.get(v).completed) and visibleUnit(state, v))
  val finishedNotCompleted = match last {
    | Finish(u) => not(state.progress.get(u).completed)
    | _ => false
  }
  val clearedKeepsContinue = last == ClearHistory and inContinue(state)
  val checkpointWithoutHistory = match last {
    | Checkpoint(u) => state.historyUnit != u
    | _ => false
  }
}
