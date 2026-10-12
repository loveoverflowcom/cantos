package com.cantos.theatre.domain.playback

/** theatre-mock-playback@1: pure facts in, immutable state and ordered effects out. */
fun reducePlayback(state: PlaybackState, event: PlaybackEvent): PlaybackTransition = when (event) {
    is PlaybackEvent.SourceSelected -> {
        if (state.session?.source == event.source) {
            unchanged(state)
        } else {
            loadSource(state, event.source, event.positionMs, 1000, PlaybackPauseCause.User, restore = false)
        }
    }
    is PlaybackEvent.RestoreRequested -> loadSource(
        state,
        event.snapshot.source,
        event.snapshot.positionMs,
        event.snapshot.speedPermille,
        PlaybackPauseCause.ColdRestore,
        restore = true,
    )
    PlaybackEvent.PlayRequested -> requestPlay(state)
    PlaybackEvent.PauseRequested -> requestPause(state)
    is PlaybackEvent.SeekRequested -> seek(state, event.positionMs)
    is PlaybackEvent.SkipRequested -> {
        val session = state.session
        if (session == null) {
            rejected(state, PlaybackRejection.NoSession)
        } else {
            // Uint32 position plus Int32 delta fits in Long before clamping.
            seek(state, (session.positionMs + event.deltaMs.toLong()).coerceIn(0, session.source.durationMs))
        }
    }
    is PlaybackEvent.SpeedRequested -> requestSpeed(state, event.speedPermille)
    PlaybackEvent.InterruptionBegan -> beginInterruption(state)
    is PlaybackEvent.InterruptionEnded -> endInterruption(state, event.resumeAllowed)
    is PlaybackEvent.EngineReady -> callback(state, event.identity) { session ->
        if (session.engine == PlaybackEngine.Preparing) session.copy(engine = PlaybackEngine.Ready) else session
    }
    is PlaybackEvent.EngineStarted -> callback(state, event.identity) { session ->
        if (session.intent == PlaybackIntent.Play && session.interruption == PlaybackInterruption.None) {
            session.copy(engine = PlaybackEngine.Running)
        } else {
            session
        }
    }
    is PlaybackEvent.EnginePaused -> callback(state, event.identity) { it.copy(engine = PlaybackEngine.Paused) }
    is PlaybackEvent.EngineBuffering -> callback(state, event.identity) { session ->
        if (session.intent == PlaybackIntent.Play && session.interruption == PlaybackInterruption.None) {
            session.copy(engine = PlaybackEngine.Buffering)
        } else {
            session
        }
    }
    is PlaybackEvent.EnginePosition -> enginePosition(state, event)
    is PlaybackEvent.EngineEnded -> callback(state, event.identity) { session ->
        session.copy(
            positionMs = session.source.durationMs,
            intent = PlaybackIntent.Pause(PlaybackPauseCause.User),
            engine = PlaybackEngine.Ended,
        )
    }
}

private fun unchanged(state: PlaybackState): PlaybackTransition = PlaybackTransition(state, emptyList())

private fun rejected(state: PlaybackState, reason: PlaybackRejection): PlaybackTransition =
    PlaybackTransition(state, listOf(PlaybackEffect.Rejected(reason)))

private fun command(state: PlaybackState, session: PlaybackSession): PlaybackEngineCommand =
    PlaybackEngineCommand(
        identity = PlaybackIdentity.fromState(session.source, state),
        source = session.source,
        positionMs = session.positionMs,
        speedPermille = session.speedPermille,
    )

private fun replaceWithCommand(
    state: PlaybackState,
    session: PlaybackSession,
    effect: (PlaybackEngineCommand) -> PlaybackEffect,
): PlaybackTransition {
    if (state.generation == MAX_PLAYBACK_GENERATION) return rejected(state, PlaybackRejection.GenerationExhausted)
    val next = state.copy(generation = state.generation + 1, session = session)
    return PlaybackTransition(next, listOf(effect(command(next, session))))
}

private fun loadSource(
    state: PlaybackState,
    source: PlaybackSource,
    positionMs: Long,
    speedPermille: Int,
    cause: PlaybackPauseCause,
    restore: Boolean,
): PlaybackTransition {
    if (positionMs !in 0..source.durationMs) return rejected(state, PlaybackRejection.OutOfBounds)
    if (!isSupportedPlaybackSpeed(speedPermille)) return rejected(state, PlaybackRejection.UnsupportedSpeed)
    val session = PlaybackSession(
        source = source,
        positionMs = positionMs,
        speedPermille = speedPermille,
        intent = PlaybackIntent.Pause(cause),
        engine = PlaybackEngine.Preparing,
        interruption = if (restore) PlaybackInterruption.None else state.session?.interruption ?: PlaybackInterruption.None,
    )
    val loaded = replaceWithCommand(state, session, PlaybackEffect::PrepareEngine)
    if (!restore || loaded.nextState == state) return loaded
    return loaded.copy(effects = loaded.effects + PlaybackEffect.OfferResume(source, positionMs))
}

private fun requestPlay(state: PlaybackState): PlaybackTransition {
    val session = state.session ?: return rejected(state, PlaybackRejection.NoSession)
    if (session.interruption == PlaybackInterruption.Active) return rejected(state, PlaybackRejection.Interrupted)
    if (session.intent == PlaybackIntent.Play && session.engine != PlaybackEngine.Ended) return unchanged(state)
    val nextSession = session.copy(
        positionMs = if (session.positionMs == session.source.durationMs) 0 else session.positionMs,
        intent = PlaybackIntent.Play,
        engine = PlaybackEngine.Preparing,
    )
    return replaceWithCommand(state, nextSession, PlaybackEffect::StartEngine)
}

private fun requestPause(state: PlaybackState): PlaybackTransition {
    val session = state.session ?: return rejected(state, PlaybackRejection.NoSession)
    if (session.intent == PlaybackIntent.Pause(PlaybackPauseCause.User) && session.engine == PlaybackEngine.Paused) {
        return unchanged(state)
    }
    return replaceWithCommand(
        state,
        session.copy(intent = PlaybackIntent.Pause(PlaybackPauseCause.User), engine = PlaybackEngine.Paused),
        PlaybackEffect::PauseEngine,
    )
}

private fun seek(state: PlaybackState, positionMs: Long): PlaybackTransition {
    val session = state.session ?: return rejected(state, PlaybackRejection.NoSession)
    if (positionMs !in 0..session.source.durationMs) return rejected(state, PlaybackRejection.OutOfBounds)
    if (positionMs == session.positionMs) return unchanged(state)
    val nextSession = session.copy(positionMs = positionMs)
    return replaceWithCommand(state, nextSession) { engineCommand ->
        when (session.engine) {
            PlaybackEngine.Preparing -> when (session.intent) {
                PlaybackIntent.Play -> PlaybackEffect.StartEngine(engineCommand)
                is PlaybackIntent.Pause -> PlaybackEffect.PrepareEngine(engineCommand)
            }
            PlaybackEngine.Ready, PlaybackEngine.Running, PlaybackEngine.Paused,
            PlaybackEngine.Buffering, PlaybackEngine.Ended -> PlaybackEffect.SeekEngine(engineCommand)
        }
    }
}

private fun requestSpeed(state: PlaybackState, speedPermille: Int): PlaybackTransition {
    if (!isSupportedPlaybackSpeed(speedPermille)) return rejected(state, PlaybackRejection.UnsupportedSpeed)
    val session = state.session ?: return rejected(state, PlaybackRejection.NoSession)
    if (session.speedPermille == speedPermille) return unchanged(state)
    val nextSession = session.copy(speedPermille = speedPermille)
    val next = state.copy(session = nextSession)
    return PlaybackTransition(next, listOf(PlaybackEffect.SetSpeed(command(next, nextSession))))
}

private fun beginInterruption(state: PlaybackState): PlaybackTransition {
    val session = state.session ?: return unchanged(state)
    if (session.interruption == PlaybackInterruption.Active) return unchanged(state)
    return replaceWithCommand(
        state,
        session.copy(interruption = PlaybackInterruption.Active, engine = PlaybackEngine.Paused),
        PlaybackEffect::PauseEngine,
    )
}

private fun endInterruption(state: PlaybackState, resumeAllowed: Boolean): PlaybackTransition {
    val session = state.session ?: return unchanged(state)
    if (session.interruption != PlaybackInterruption.Active) return unchanged(state)
    if (state.generation == MAX_PLAYBACK_GENERATION) return rejected(state, PlaybackRejection.GenerationExhausted)
    val resume = resumeAllowed && session.intent == PlaybackIntent.Play
    val nextSession = session.copy(
        interruption = PlaybackInterruption.None,
        engine = if (resume) PlaybackEngine.Preparing else session.engine,
        intent = if (!resume && session.intent == PlaybackIntent.Play) {
            PlaybackIntent.Pause(PlaybackPauseCause.Interruption)
        } else {
            session.intent
        },
    )
    val next = state.copy(generation = state.generation + 1, session = nextSession)
    val effects = if (resume) listOf(PlaybackEffect.StartEngine(command(next, nextSession))) else emptyList()
    return PlaybackTransition(next, effects)
}

private fun callback(
    state: PlaybackState,
    identity: PlaybackIdentity,
    change: (PlaybackSession) -> PlaybackSession,
): PlaybackTransition {
    val session = state.session ?: return unchanged(state)
    if (!state.accepts(identity)) return unchanged(state)
    return unchanged(state.copy(session = change(session)))
}

private fun enginePosition(state: PlaybackState, event: PlaybackEvent.EnginePosition): PlaybackTransition {
    val session = state.session ?: return unchanged(state)
    if (!state.accepts(event.identity)) return unchanged(state)
    if (event.positionMs !in 0..session.source.durationMs) return rejected(state, PlaybackRejection.OutOfBounds)
    return callback(state, event.identity) { it.copy(positionMs = event.positionMs) }
}
