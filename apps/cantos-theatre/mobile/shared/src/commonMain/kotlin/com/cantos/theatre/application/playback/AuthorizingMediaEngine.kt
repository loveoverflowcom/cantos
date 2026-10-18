package com.cantos.theatre.application.playback

import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackFailure
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.ports.MediaEngine
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.StreamingMediaEngine
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch

/**
 * Process-owned access shell. Commands never wait for HTTP, so a slow grant cannot block Pause.
 * A load token fences grants as well as source/generation; native adapters additionally fence
 * the concrete decoder item. This owner alone refreshes, at most twice after a failed grant.
 */
class AuthorizingMediaEngine(
    private val scope: CoroutineScope,
    private val native: StreamingMediaEngine,
    private val access: PlaybackAccessPort,
    private val nowMs: () -> Long,
) : MediaEngine {
    private val observations = MutableSharedFlow<PlaybackObservation>(extraBufferCapacity = 64)
    override val events = observations.asSharedFlow()
    private var command: PlaybackEngineCommand? = null
    private var playing = false
    private var loadToken = 0L
    private var request: Job? = null
    private var refresh: Job? = null
    private var failures = 0
    private var closed = false
    private val collector = scope.launch {
        native.events.collect { event ->
            val current = command ?: return@collect
            val identity = when (event) {
                is PlaybackEvent.EngineReady -> event.identity
                is PlaybackEvent.EngineStarted -> event.identity
                is PlaybackEvent.EnginePaused -> event.identity
                is PlaybackEvent.EngineBuffering -> event.identity
                is PlaybackEvent.EnginePosition -> event.identity
                is PlaybackEvent.EngineEnded -> event.identity
                is PlaybackEvent.EngineFailed -> event.identity
            }
            if (closed || identity != current.identity) return@collect
            if (event is PlaybackEvent.EnginePosition) {
                command = current.copy(positionMs = event.positionMs.coerceIn(0, current.source.durationMs))
            }
            if (event is PlaybackEvent.EngineFailed && playing &&
                event.failure in setOf(PlaybackFailure.Network, PlaybackFailure.Expired) && failures < 2) {
                failures += 1
                observations.emit(PlaybackEvent.EngineBuffering(current.identity))
                launchLoad(backoffMs = failures * 500L)
            } else {
                if (event is PlaybackEvent.EngineEnded || event is PlaybackEvent.EngineFailed) {
                    playing = false
                    refresh?.cancel()
                }
                observations.emit(event)
            }
        }
    }

    override suspend fun prepare(command: PlaybackEngineCommand) {
        cancelAccess()
        this.command = command
        playing = false
        failures = 0
        // No decoder or audio focus is activated for a cold restore.
        native.pause(command)
    }

    override suspend fun start(command: PlaybackEngineCommand) {
        if (closed) return
        this.command = command
        playing = true
        failures = 0
        launchLoad()
    }

    private fun launchLoad(backoffMs: Long = 0) {
        cancelAccess()
        val token = loadToken
        request = scope.launch {
            if (backoffMs > 0) delay(backoffMs)
            val current = command ?: return@launch
            val audio = current.source.audio ?: return@launch
            when (val result = access.authorize(audio)) {
                is PlaybackAccessResult.Refused -> {
                    if (closed || token != loadToken) return@launch
                    if (result.reason in setOf(com.cantos.theatre.ports.PlaybackAccessFailure.Network,
                            com.cantos.theatre.ports.PlaybackAccessFailure.Unavailable) && failures < 2) {
                        failures += 1
                        launchLoad(failures * 500L)
                    } else {
                        playing = false
                        native.pause(command ?: current)
                        observations.emit(PlaybackEvent.EngineFailed(current.identity,
                            if (result.reason == com.cantos.theatre.ports.PlaybackAccessFailure.Network) PlaybackFailure.Network
                            else PlaybackFailure.Unavailable))
                    }
                }
                is PlaybackAccessResult.Accepted -> {
                    if (closed || token != loadToken || !playing || command?.identity != current.identity) return@launch
                    if (result.value.expiresAtMs <= nowMs()) {
                        if (failures < 2) {
                            failures += 1
                            launchLoad(failures * 500L)
                        } else {
                            playing = false
                            native.pause(command ?: current)
                            observations.emit(PlaybackEvent.EngineFailed(current.identity, PlaybackFailure.Expired))
                        }
                        return@launch
                    }
                    // HTTP can finish after a seek/speed; use the latest command, never its old point.
                    val latest = command ?: return@launch
                    observations.emit(PlaybackEvent.EngineBuffering(latest.identity))
                    native.load(latest, result.value.location)
                    if (closed || token != loadToken || !playing) return@launch
                    native.start(latest)
                    refresh = scope.launch {
                        delay((result.value.expiresAtMs - nowMs() - 15_000).coerceAtLeast(1_000))
                        if (!closed && token == loadToken && playing) {
                            failures = 0
                            launchLoad()
                        }
                    }
                }
            }
        }
    }

    override suspend fun pause(command: PlaybackEngineCommand) {
        cancelAccess()
        this.command = command
        playing = false
        native.pause(command)
        if (command.clearSession) this.command = null
    }

    override suspend fun seek(command: PlaybackEngineCommand) {
        this.command = command
        if (playing) {
            native.pause(command)
            launchLoad()
        } else native.seek(command)
    }

    override suspend fun setSpeed(command: PlaybackEngineCommand) {
        this.command = command
        native.setSpeed(command)
    }

    private fun cancelAccess() {
        loadToken += 1
        request?.cancel()
        refresh?.cancel()
    }

    override fun close() {
        if (closed) return
        closed = true
        cancelAccess()
        collector.cancel()
        native.close()
    }
}
