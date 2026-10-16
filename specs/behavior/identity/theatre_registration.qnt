// Draft executable #174/ADR0034 expectations of one registration operation.
// Abstract SQL/provider/mail facts are NOT runtime or least-privilege acceptance.
// No policy values, email canonicalization, retention, credentials or session issuance.
module theatre_registration {
  type Phase = Awaiting | Cancelled | Creating | Created | SetupStarted | Delivering | Complete | Blocked
  type Outcome = NoOutcome | Changed
  type Event = NoEvent | Assertion | Transition
  type State = {
    phase: Phase, epoch: int, unexpired: bool, claimedEpoch: int, claimedValid: bool,
    person: bool, createCalls: int, setupCalls: int, mailCalls: int,
    session: bool, studio: bool, admin: bool
  }
  pure val initial: State = {
    phase: Awaiting, epoch: 0, unexpired: true, claimedEpoch: 0, claimedValid: false,
    person: false, createCalls: 0, setupCalls: 0, mailCalls: 0,
    session: false, studio: false, admin: false
  }
  var state: State
  var before: State
  var last: Event
  var outcome: Outcome
  action init = all { state' = initial, before' = initial, last' = NoEvent, outcome' = NoOutcome }
  action record(successor: State): bool = all { state' = successor, before' = state, last' = Transition, outcome' = Changed }
  // Proof epochs stand for distinct random hashes. There is no hash collision or numeric TTL claim.
  action rotate = all {
    Set(Awaiting, Cancelled).contains(state.phase) and state.epoch < 3,
    record({ ...state, phase: Awaiting, epoch: state.epoch + 1, unexpired: true })
  }
  action cancel = all { state.phase == Awaiting, record({ ...state, phase: Cancelled }) }
  action expire = record({ ...state, unexpired: false })
  action claim(proof: int): bool = {
    val allowed = state.phase == Awaiting and proof == state.epoch and state.unexpired
    record(if (allowed) { ...state, phase: Creating, claimedEpoch: proof, claimedValid: state.unexpired }
      else state)
  }
  // Creating/SetupStarted/Delivering stand for committed fences before each external effect.
  action create = all {
    state.phase == Creating and state.createCalls == 0,
    record({ ...state, createCalls: state.createCalls + 1 })
  }
  // ack means exact positive acknowledgement of the preselected NEW person UUID.
  // Conflict, malformed/mismatched acknowledgement and every uncertain result all pass false.
  action created(ack: bool): bool = all {
    state.phase == Creating and state.createCalls == 1,
    record({ ...state, phase: if (ack) Created else Blocked, person: ack })
  }
  action beginSetup = all {
    state.phase == Created and state.person,
    record({ ...state, phase: SetupStarted })
  }
  action setup = all {
    state.phase == SetupStarted and state.setupCalls == 0,
    record({ ...state, setupCalls: state.setupCalls + 1 })
  }
  action setupResult(known: bool): bool = all {
    state.phase == SetupStarted and state.setupCalls == 1,
    record({ ...state, phase: if (known) Delivering else Blocked })
  }
  action mail = all {
    state.phase == Delivering and state.mailCalls == 0,
    record({ ...state, mailCalls: state.mailCalls + 1 })
  }
  action mailResult(accepted: bool): bool = all {
    state.phase == Delivering and state.mailCalls == 1,
    record({ ...state, phase: if (accepted) Complete else Blocked })
  }
  action step = {
    nondet proof = 0.to(3).oneOf()
    any {
      rotate, cancel, expire, claim(proof), create, created(true), created(false),
      beginSetup, setup, setupResult(true), setupResult(false), mail, mailResult(true), mailResult(false)
    }
  }
  val proofBeforeCreate = state.createCalls == 0 or (state.claimedEpoch == state.epoch and state.claimedValid)
  val setupOnlyNewPerson = state.setupCalls == 0 or (state.person and state.createCalls == 1)
  val atMostOnce = state.createCalls <= 1 and state.setupCalls <= 1 and state.mailCalls <= 1
  val noAuthorityIssued = not(state.session) and not(state.studio) and not(state.admin)
  val blockedSticky = before.phase != Blocked or state.phase == Blocked
  val claimed = state.phase == Creating and state.createCalls == 1
  val setupDispatched = state.phase == SetupStarted and state.setupCalls == 1
  val completeWithoutSession = state.phase == Complete and not(state.session)
  val stoppedAfterUnknown = state.phase == Blocked and state.createCalls == 1
}
