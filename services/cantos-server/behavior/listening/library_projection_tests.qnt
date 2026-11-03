module library_projection_tests {
  import library_projection.* from "./library_projection"

  action check(predicate: bool): bool = all {
    assert(predicate), state' = state, before' = state, last' = Assertion
  }

  // J1: a metadata-only Work can be saved and is presented without a fake Play action.
  run savedWithoutAudioTest = init.then(withdraw(1)).then(withdraw(3)).then(save(true))
    .then(check(savedNoAudio and savedPresentable and savedListenHonest))
    .then(attach(2)).then(check(savedRow(state) == SavedShown(Metadata(Playable)) and savedListenHonest))
    .then(withdraw(2)).then(check(savedRow(state) == SavedShown(Metadata(NoAudioYet))))
  run saveAdmissionTest = init.then(expose(false)).then(save(true))
    .then(check(not(state.saved) and savedAdmission)).then(expose(true)).then(withdraw(1))
    .then(withdraw(3)).then(save(true)).then(check(metadataOnlySave and savedAdmission))
    .then(save(true)).then(check(state.saved and savedAdmission))
  run hiddenRetainsTest = init.then(save(true)).then(start(1)).then(expose(false))
    .then(check(hiddenRetained and retainedWhenHidden and libraryAction(state) == Hidden
      and not(inContinue(state)) and not(historyShown(state))))
    .then(expose(true)).then(check(libraryAction(state) == Resume(1) and historyShown(state)))
  // J2: exact resume; a replacement is offered explicitly and never substituted.
  run replacedReleaseTest = init.then(start(1)).then(checkpoint(1, false))
    .then(check(libraryAction(state) == Resume(1) and exactResume)).then(attach(1))
    .then(check(replacedRelease and exactResume and inContinue(state))).then(withdraw(1))
    .then(check(libraryAction(state) == AudioUnavailable(1) and inContinue(state))).then(attach(1))
    .then(start(1)).then(check(libraryAction(state) == Resume(1) and exactResume))
  // J3/J5: a finished chapter points to the immediately following chapter, never skipping it.
  run nextInOrderTest = init.then(start(1)).then(checkpoint(1, true)).then(complete(1))
    .then(check(libraryAction(state) == WaitingAudio(2) and waitingBeforeLater and nextInOrder
      and not(inContinue(state))))
    .then(attach(2)).then(check(chapterReentered and nextInOrder and continueMembership))
    .then(hide(2)).then(check(libraryAction(state) == StartNext(3) and nextInOrder))
  run caughtUpTest = init.then(start(3)).then(checkpoint(3, true)).then(complete(3))
    .then(check(caughtUp and continueMembership and not(inContinue(state))))
    .then(publish(4)).then(check(libraryAction(state) == WaitingAudio(4) and continueMembership))
    .then(attach(4)).then(check(chapterReentered and inContinue(state)))
    .then(start(1)).then(check(libraryAction(state) == Resume(1)))
  run hiddenAnchorTest = init.then(start(1)).then(checkpoint(1, false)).then(start(3)).then(hide(3))
    .then(check(hiddenLatest and hiddenFailsClosed and not(inContinue(state))))
    .then(publish(3)).then(check(libraryAction(state) == AudioUnavailable(3) and inContinue(state)))
  run finishNotCompleteTest = init.then(start(1)).then(checkpoint(1, true))
    .then(check(finishedNotCompleted and noAutoComplete and libraryAction(state) == Resume(1)))
    .then(complete(1)).then(check(state.progress.get(1).completed and libraryAction(state) == WaitingAudio(2)))
  // Unsave keeps Continue, Recent and progress; saving again restores only the Saved row.
  run unsaveRetainsTest = init.then(save(true)).then(start(1)).then(save(false))
    .then(check(unsavedKeepsContinue and registersIndependent and savedRow(state) == SavedAbsent))
    .then(save(true)).then(check(registersIndependent and libraryAction(state) == Resume(1)
      and historyShown(state)))
  // D-L08: no clear; Recent moves only on an acknowledged engine start, never on a checkpoint.
  run historyFromStartTest = init.then(start(1)).then(start(3)).then(checkpoint(1, false))
    .then(check(checkpointWithoutHistory and historyFromStart and state.historyUnit == 3))
    .then(start(1)).then(check(state.historyUnit == 1 and historyFromStart))
}
