// Draft sample-only disclosure model. Release eligibility is an external fact; no audio is made.
module sample_metadata {
  type State = { scope: bool, ownerActive: bool, public: bool, work: bool, unit: bool,
    released: bool, visible: bool, playable: bool, seedOwned: bool, seedIdentity: bool,
    seedSource: bool, seedRights: bool, seedPublished: bool, seedWithdrawn: bool }
  pure def project(s: State): State = { ...s,
    visible: s.released or (s.scope and s.ownerActive and s.public and s.work and s.unit),
    playable: s.released }
  pure val initial: State = { scope: false, ownerActive: true, public: true,
    work: false, unit: false, released: false, visible: false, playable: false,
    seedOwned: false, seedIdentity: false, seedSource: false, seedRights: false,
    seedPublished: false, seedWithdrawn: false }
  var state: State
  action init = state' = initial
  action scope(b: bool): bool = state' = project({ ...state, scope: b })
  action active(b: bool): bool = state' = project({ ...state, ownerActive: b })
  action visibility(b: bool): bool = state' = project({ ...state, public: b })
  action work(b: bool): bool = state' = project({ ...state, work: b })
  action unit(b: bool): bool = state' = project({ ...state, unit: b })
  action release(b: bool): bool = state' = project({ ...state, released: b })
  // These flags stand for accepted inputs, not proof of the source or the claim's truth.
  action confirmSource = state' = project({ ...state, seedSource: true })
  action confirmRights = state' = project({ ...state, seedRights: true })
  action attachIdentity = all { not(state.seedOwned), state' = project({ ...state, seedIdentity: true }) }
  action seedPublish = all { not(state.seedOwned), not(state.seedIdentity),
    state.seedSource and state.seedRights,
    state' = project({ ...state, seedOwned: true, seedPublished: true }) }
  action seedWithdraw = all { state.seedPublished,
    state' = project({ ...state, seedPublished: false, seedWithdrawn: true }) }
  action step = any { scope(true), scope(false), active(true), active(false),
    visibility(true), visibility(false), work(true), work(false), unit(true), unit(false),
    release(true), release(false), confirmSource, confirmRights, attachIdentity,
    seedPublish, seedWithdraw }
  val scoped = state.released or not(state.visible) or state.scope
  val separateDecisions = state.released or not(state.visible) or (state.work and state.unit)
  val liveGates = state.released or not(state.visible) or (state.ownerActive and state.public)
  val playbackNeedsRelease = not(state.playable) or state.released
  val seedCannotLogin = not(state.seedOwned) or not(state.seedIdentity)
  val seedAttested = not(state.seedPublished) or (state.seedOwned and state.seedSource and state.seedRights)
  val seedWithdrawalPermanent = not(state.seedWithdrawn) or not(state.seedPublished)
  val sampleShown = state.visible and not(state.released)
  val noUnitHidden = state.scope and state.work and not(state.unit) and not(state.released) and not(state.visible)
  val inactiveHidden = state.scope and state.work and state.unit and not(state.ownerActive) and not(state.released) and not(state.visible)
  val releasePlayable = state.released and state.playable
  val seedRevoked = state.seedOwned and state.seedWithdrawn and not(state.seedPublished)
}
