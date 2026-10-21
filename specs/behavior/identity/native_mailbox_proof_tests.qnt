module native_mailbox_proof_tests {
  import native_mailbox_proof.* from "./native_mailbox_proof"
  action check(predicate: bool): bool = all { assert(predicate), state' = state, before' = state, last' = Assertion, outcome' = NoOutcome }
  pure val exact: Request = { operation: 1, epoch: 1, intent: TheatreSignup, loc: 1, secret: 1, explicit: true }
  pure def at(loc: int): Request = { ...exact, loc: loc, secret: loc }

  run explicitConfirmationTest = init.then(settle(Commit))
    .then(check(outcome == Rejected(NothingPlanned) and state == before))
    .then(inspect).then(check(outcome == NoOp(Inspected) and state == before and explicitOnly))
    .then(prepare(1, { ...exact, explicit: false })).then(check(outcome == Rejected(NotExplicit) and state == before))
    .then(prepare(1, exact)).then(check(planWithoutEffect and planHasNoEffect and state.plan != NoPlan))
    .then(inspect).then(check(inspectedPending and explicitOnly))
    .then(settle(Commit)).then(check(consumedExplicitly and explicitOnly and consumedWithinBoundWindow
      and commitComparesSnapshot and noAuthority and state.plan == NoPlan))
  run bindingAndWindowTest = init.then(prepare(1, { ...exact, epoch: 2 }))
    .then(check(outcome == Rejected(EpochChanged) and state == before))
    .then(prepare(1, { ...exact, operation: 2 })).then(check(outcome == Rejected(UnknownOperation)))
    .then(prepare(1, { ...exact, intent: StudioRecovery })).then(check(outcome == Rejected(BindingChanged)))
    .then(prepare(1, { ...exact, intent: TheatreRecovery })).then(check(outcome == Rejected(BindingChanged)))
    .then(prepare(1, { ...exact, secret: 0 })).then(check(outcome == Rejected(InvalidSecret)))
    .then(clock(0)).then(prepare(1, exact)).then(check(outcome == Rejected(BeforeIssue)))
    .then(clock(3)).then(prepare(1, exact))
    .then(check(outcome == Rejected(Expired) and state.durable.proof.state == Pending))
    .then(clock(2)).then(prepare(1, exact)).then(check(planWithoutEffect))
    .then(clock(3)).then(settle(Commit)).then(check(expiredAtCommit and state.durable.proof.state == Pending
      and state.durable.accounts == 0 and state.plan == NoPlan and consumedWithinBoundWindow))
  run cancelledTerminalTest = init.then(cancel(2))
    .then(check(outcome == Rejected(ProofChanged) and state == before and retiredLocatorRejected))
    .then(cancel(1)).then(check(outcome == Transition(CancelledProof)))
    .then(prepare(1, exact)).then(check(outcome == Rejected(AlreadyCancelled) and terminalSticky))
    .then(clock(2)).then(replace(1, 2))
    .then(check(cancelledReplaceRefused and terminalSticky and state.durable.proof.state == Cancelled))
    .then(cancel(1)).then(check(outcome == Rejected(AlreadyCancelled) and terminalSticky))
  run consumedTerminalTest = init.then(prepare(1, exact)).then(settle(Commit)).then(cancel(1))
    .then(check(outcome == Rejected(AlreadyConsumed) and terminalSticky))
    .then(clock(2)).then(replace(1, 2)).then(check(outcome == Rejected(AlreadyConsumed) and terminalSticky))
    .then(prepare(2, exact)).then(check(outcome == Rejected(AlreadyConsumed) and singleAccountEffect
      and state.durable.accounts == 1))
  run replacementTest = init.then(replace(1, 2)).then(check(outcome == Rejected(Cooldown)))
    .then(clock(2)).then(replace(1, 1)).then(check(outcome == Rejected(ReusedLocator)))
    .then(replace(1, 2)).then(check(outcome == Transition(ReplacedProof)
      and state.durable.proof == { loc: 2, state: Pending, issuedAt: 2, expiresAt: 4 }))
    .then(prepare(1, at(1))).then(check(retiredLocatorRefused and retiredLocatorRejected and state == before))
    .then(cancel(1)).then(check(outcome == Rejected(ProofChanged) and retiredLocatorRejected))
    .then(prepare(1, { ...at(2), secret: 1 })).then(check(outcome == Rejected(InvalidSecret)))
    .then(clock(3)).then(replace(2, 1)).then(check(outcome == Rejected(ReusedLocator)))
    .then(prepare(1, at(2))).then(settle(Commit)).then(check(consumedExplicitly and consumedWithinBoundWindow))
  run stalePlanTest = init.then(clock(2)).then(prepare(1, exact)).then(replace(1, 2)).then(settle(Commit))
    .then(check(staleCommitRefused and state.durable.proof.loc == 2 and state.durable.accounts == 0
      and commitComparesSnapshot))
    .then(prepare(1, at(2))).then(cancel(2)).then(settle(Commit))
    .then(check(outcome == Rejected(StaleSnapshot) and state.durable.proof.state == Cancelled
      and state.durable.accounts == 0))
  run lostResponseTest = init.then(prepare(1, exact)).then(settle(UnknownApplied))
    .then(check(outcome == UnknownEffect and state.durable.proof.state == Consumed and state.durable.accounts == 1))
    .then(prepare(1, exact)).then(check(exactRetryReplayed and singleAccountEffect and state == before))
    .then(prepare(1, { ...exact, epoch: 2 })).then(check(outcome == Rejected(EpochChanged)))
    .then(prepare(1, { ...exact, secret: 0 })).then(check(outcome == Rejected(OperationReused)))
    .then(prepare(1, { ...exact, explicit: false })).then(check(outcome == Rejected(OperationReused)))
    .then(prepare(2, exact)).then(check(outcome == Rejected(AlreadyConsumed) and state.durable.accounts == 1
      and noAuthority))
  run lostBeforeCommitTest = init.then(prepare(1, exact)).then(settle(UnknownLost))
    .then(check(outcome == UnknownEffect and state.durable.proof.state == Pending and state.durable.accounts == 0
      and state.durable.receipts.keys() == Set()))
    .then(prepare(1, exact)).then(check(planWithoutEffect))
    .then(settle(Reject)).then(check(outcome == Rejected(CommitRejected) and state.durable.proof.state == Pending
      and state.durable.accounts == 0))
    .then(prepare(1, exact)).then(settle(Commit)).then(check(consumedExplicitly and singleAccountEffect))
}
