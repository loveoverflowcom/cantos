// Draft #334/#335 executable expectations for turning a native Identity authentication fact
// into ONE Server session and re-admitting it against a fresh security-state read.
// No HTTP, cookie, CSRF, browser, SQL lock, token hash, rotation or RPC transport is modeled.
// Subjects, epochs and versions are equality/order classes, not ids or real counters.
module native_session_admission {
  type Purpose = Theatre | Studio
  // Fact age at the Server instant: inside [-5 s, +30 s], older, or further ahead.
  type Age = Fresh | Stale | Future
  // `echoed` = the fact echoes this login's operation; `purpose` = the purpose it carries.
  type Fact = { subject: int, purpose: Purpose, echoed: bool, age: Age, epoch: int, version: int }
  // How the Identity read for the asked subject ends. Malformed is a reply outside the wire
  // contract (protocol failure, not a security fact). A well-formed reply naming ANOTHER
  // subject is deliberately unmodeled: the contract and the implementation disagree on it.
  type Read = Answer | Unreachable | Malformed
  // Identity's record for subject 1 (the only registered subject). Subject 2 is never present.
  type Identity = { read: Read, present: bool, active: bool, epoch: int, version: int }
  type Observed = { subject: int, epoch: int, version: int }
  type Session = { exists: bool, generation: int, purpose: Purpose, observed: Observed, revoked: bool }
  type RevokeReason = SubjectMissing | Disabled | EpochChanged | VersionAdvanced | VersionRegressed
  // A requested revocation is a shell intent to write; only a committed write revokes.
  type Pending = NoPending | Requested(RevokeReason)
  type State = { identity: Identity, rostered: bool, session: Session, pending: Pending }
  type Reason = FactMisbound | NotOnRoster | IdentityUnavailable | SubjectUnknown | FactNotFresh
    | Inactive | EpochMismatch | VersionMismatch | NoSession | SessionRevoked | SecurityChanged
    | NothingRequested | RevokeWriteFailed
  type NoOpKind = Admitted | AlreadyEnded
  type TransitionKind = Issued | Revoked | LoggedOut
  type Outcome = NoOutcome | Rejected(Reason) | NoOp(NoOpKind) | Transition(TransitionKind) | External
  type Event = NoEvent | Assertion | Login({ purpose: Purpose, fact: Fact }) | Admit
    | CommitRevoke(bool) | Logout | SetIdentity(Identity) | SetRoster(bool)

  pure val SUBJECT = 1
  pure val noSession: Session = { exists: false, generation: 0, purpose: Theatre,
    observed: { subject: 0, epoch: 0, version: 0 }, revoked: false }
  pure val initial: State = {
    identity: { read: Answer, present: true, active: true, epoch: 1, version: 1 },
    rostered: true, session: noSession, pending: NoPending
  }
  var state: State
  var before: State
  var last: Event
  var outcome: Outcome

  action init = all { state' = initial, before' = initial, last' = NoEvent, outcome' = NoOutcome }
  action record(successor: State, event: Event, result: Outcome): bool = all {
    state' = successor, before' = state, last' = event, outcome' = result
  }

  // One login attempt with one fact. A new session replaces the slot; retirement of the previous
  // family on a new login is native-tested, not modeled.
  action login(purpose: Purpose, fact: Fact): bool = {
    val id = state.identity
    val event = Login({ purpose: purpose, fact: fact })
    if (not(fact.echoed) or fact.purpose != purpose) record(state, event, Rejected(FactMisbound))
    else if (purpose == Studio and not(fact.subject == SUBJECT and state.rostered))
      record(state, event, Rejected(NotOnRoster))
    else if (id.read != Answer) record(state, event, Rejected(IdentityUnavailable))
    else if (fact.subject != SUBJECT or not(id.present)) record(state, event, Rejected(SubjectUnknown))
    else if (fact.age != Fresh) record(state, event, Rejected(FactNotFresh))
    else if (not(id.active)) record(state, event, Rejected(Inactive))
    else if (id.epoch != fact.epoch) record(state, event, Rejected(EpochMismatch))
    else if (id.version != fact.version) record(state, event, Rejected(VersionMismatch))
    else record({ ...state, pending: NoPending, session: { exists: true,
      generation: state.session.generation + 1, purpose: purpose,
      observed: { subject: fact.subject, epoch: fact.epoch, version: fact.version }, revoked: false } },
      event, Transition(Issued))
  }

  action request(reason: RevokeReason): bool =
    record({ ...state, pending: Requested(reason) }, Admit, Rejected(SecurityChanged))

  // Every protected admission re-reads Identity; there is no cached verdict.
  action admit = {
    val id = state.identity
    val observed = state.session.observed
    if (not(state.session.exists)) record(state, Admit, Rejected(NoSession))
    else if (state.session.revoked) record(state, Admit, Rejected(SessionRevoked))
    else if (state.session.purpose == Studio and not(state.rostered)) record(state, Admit, Rejected(NotOnRoster))
    else if (state.identity.read != Answer) record(state, Admit, Rejected(IdentityUnavailable))
    else if (not(id.present)) request(SubjectMissing)
    else if (not(id.active)) request(Disabled)
    else if (id.epoch != observed.epoch) request(EpochChanged)
    else if (id.version > observed.version) request(VersionAdvanced)
    else if (id.version < observed.version) request(VersionRegressed)
    else record(state, Admit, NoOp(Admitted))
  }

  // The durable revoke write for a requested revocation; it may fail.
  action commitRevoke(written: bool): bool =
    if (state.pending == NoPending) record(state, CommitRevoke(written), Rejected(NothingRequested))
    else if (written) record({ ...state, pending: NoPending, session: { ...state.session, revoked: true } },
      CommitRevoke(written), Transition(Revoked))
    else record({ ...state, pending: NoPending }, CommitRevoke(written), Rejected(RevokeWriteFailed))

  // Local and durable; never consults Identity.
  action logout =
    if (state.session.exists and not(state.session.revoked))
      record({ ...state, session: { ...state.session, revoked: true } }, Logout, Transition(LoggedOut))
    else record(state, Logout, NoOp(AlreadyEnded))

  // External facts: Identity commits a change, a restored file, an outage; the operator roster.
  action setIdentity(id: Identity): bool = record({ ...state, identity: id }, SetIdentity(id), External)
  action setRoster(on: bool): bool = record({ ...state, rostered: on }, SetRoster(on), External)

  // Bias sampling toward exact facts, single-field Identity changes and restoration of the
  // observed snapshot. Not a fairness assumption: arbitrary inputs stay in `step`.
  action guided = {
    nondet purpose = Set(Theatre, Studio).oneOf()
    nondet written = Set(false, true).oneOf()
    nondet on = Set(false, true).oneOf()
    nondet read = Set(Answer, Unreachable, Malformed).oneOf()
    val id = state.identity
    val obs = if (state.session.exists) state.session.observed else { subject: SUBJECT, epoch: 1, version: 2 }
    val exact = { subject: SUBJECT, purpose: purpose, echoed: true, age: Fresh, epoch: id.epoch, version: id.version }
    val restored = { read: Answer, present: true, active: true, epoch: obs.epoch, version: obs.version }
    any {
      login(purpose, exact), admit, admit, commitRevoke(written), logout, setRoster(on),
      setIdentity(restored), setIdentity({ ...restored, read: read }),
      setIdentity({ ...restored, present: false }), setIdentity({ ...restored, active: false }),
      setIdentity({ ...restored, epoch: 2 }), setIdentity({ ...restored, version: obs.version + 1 }),
      setIdentity({ ...restored, version: obs.version - 1 })
    }
  }
  action step = {
    nondet purpose = Set(Theatre, Studio).oneOf()
    nondet factPurpose = Set(Theatre, Studio).oneOf()
    nondet subject = Set(1, 2).oneOf()
    nondet echoed = Set(false, true).oneOf()
    nondet age = Set(Fresh, Stale, Future).oneOf()
    nondet epoch = Set(1, 2).oneOf()
    nondet version = 1.to(3).oneOf()
    nondet read = Set(Answer, Unreachable, Malformed).oneOf()
    nondet present = Set(false, true).oneOf()
    nondet active = Set(false, true).oneOf()
    nondet written = Set(false, true).oneOf()
    nondet on = Set(false, true).oneOf()
    any {
      guided, guided, guided,
      login(purpose, { subject: subject, purpose: factPurpose, echoed: echoed, age: age, epoch: epoch, version: version }),
      admit, commitRevoke(written), logout, setRoster(on),
      setIdentity({ read: read, present: present, active: active, epoch: epoch, version: version })
    }
  }

  def matches(id: Identity, observed: Observed): bool =
    id.read == Answer and id.present and id.active and id.epoch == observed.epoch and id.version == observed.version
  def sameSession(a: Session, b: Session): bool = a.exists and b.exists and a.generation == b.generation

  // A session is issued only from an echoed fact of the requested purpose, fresh, for the
  // present Active subject whose epoch/version a fresh read confirms (Studio also rostered),
  // and it observes exactly that fact.
  val issueNeedsFreshBoundFact = match last {
    | Login(e) => outcome != Transition(Issued) or (
      e.fact.echoed and e.fact.purpose == e.purpose and e.fact.age == Fresh and e.fact.subject == SUBJECT
      and matches(before.identity, { subject: e.fact.subject, epoch: e.fact.epoch, version: e.fact.version })
      and (e.purpose == Theatre or before.rostered) and state.session.exists and not(state.session.revoked)
      and state.session.purpose == e.purpose
      and state.session.observed == { subject: e.fact.subject, epoch: e.fact.epoch, version: e.fact.version })
    | _ => true
  }
  // Admission only for an unrevoked session whose fresh read is Active with equal epoch/version.
  val admitOnlyExactActive = outcome == NoOp(Admitted) implies (
    before.session.exists and not(before.session.revoked) and matches(before.identity, before.session.observed)
    and (before.session.purpose == Theatre or before.rostered))
  // Every answered difference is denied and requests revocation with a reason that names an
  // actual difference (missing, disabled, other epoch, higher or LOWER version).
  def reasonIsActual(reason: RevokeReason): bool =
    if (reason == SubjectMissing) not(before.identity.present)
    else if (reason == Disabled) before.identity.present and not(before.identity.active)
    else if (reason == EpochChanged) before.identity.epoch != before.session.observed.epoch
    else if (reason == VersionAdvanced) before.identity.version > before.session.observed.version
    else before.identity.version < before.session.observed.version
  val liveAnsweredMismatch = before.session.exists and not(before.session.revoked) and
    (before.session.purpose == Theatre or before.rostered) and before.identity.read == Answer and
    not(matches(before.identity, before.session.observed))
  val mismatchRequestsRevoke = last == Admit implies (
    if (outcome == Rejected(SecurityChanged))
      before.identity.read == Answer and match state.pending {
        | NoPending => false
        | Requested(reason) => reasonIsActual(reason)
      }
    else state.pending == before.pending and not(liveAnsweredMismatch))
  // A request is not a commit: admission never writes the session; only a committed revoke
  // write (or logout) revokes, and a revoke write needs an outstanding request.
  val requestIsNotCommit = (last == Admit implies state.session == before.session) and
    (outcome == Transition(Revoked) implies before.pending != NoPending)
  // Committed revocation of a session is permanent, including when a later read matches again.
  val revocationSticky = (sameSession(before.session, state.session) and before.session.revoked) implies
    (state.session.revoked and outcome != NoOp(Admitted))
  // An outage or malformed answer denies without requesting or committing any revocation.
  val outageNeverRevokes = match last {
    | Admit => before.identity.read == Answer or (state.session == before.session and state.pending == before.pending)
    | Login(_) => before.identity.read == Answer or (state.session == before.session and state.pending == before.pending)
    | _ => true
  }

  val issued = outcome == Transition(Issued) and state.session.purpose == Studio
  val admitted = outcome == NoOp(Admitted) and state.session.generation >= 1
  val staleFactRefused = outcome == Rejected(FactNotFresh) and matches(before.identity,
    { subject: SUBJECT, epoch: before.identity.epoch, version: before.identity.version })
  val epochChangeRequested = state.pending == Requested(EpochChanged) and
    before.identity.version == before.session.observed.version
  val regressionRequested = state.pending == Requested(VersionRegressed) and outcome == Rejected(SecurityChanged)
  val requestWithoutCommit = outcome == Rejected(RevokeWriteFailed) and not(state.session.revoked)
  val stickyAfterRestore = before.session.revoked and outcome == Rejected(SessionRevoked)
    and matches(before.identity, before.session.observed)
  val outageDenied = last == Admit and outcome == Rejected(IdentityUnavailable) and not(state.session.revoked)
}
