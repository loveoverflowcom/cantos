package com.cantos.theatre.domain.playback

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

internal const val PLAYBACK_VECTOR_CONTRACT = "theatre-mock-playback@1"

/** Independently pinned reviewed inventory; expected transitions stay only in canonical JSON. */
internal val reviewedPlaybackHashes = linkedMapOf(
    "play-pause.json" to "bc9b73d4be95362d1a6d7e178464bf26d100d93edac02edb27cf018c45a54622",
    "buffering.json" to "4b5f7df3d009c0855f328a600f2c4c9149e6277d50469b01b864ea2604a1797c",
    "interruption.json" to "3c39358f06d38dce35fd73ddebae35fe0a6a36bc04727ceee8dd6feb624fb945",
    "stale-response.json" to "d58ea2b08b79ab453c3864da3cb5532fe01825adf4db0c93f895c4c81f018ae6",
    "restore-session.json" to "bca2510896ed12250a3979abe42414e66c14357c73d403532a90c54316ddb900",
    "preparing-races.json" to "b5d21a9cece278313b89de9b5e885bff424ca1752f5b0d23d8fc39f838fa8082",
    "invalid-controls.json" to "bfa0807d0df0186dd5429ac082ef40df87879d487c6823d3c365e6e5ba0304a9",
)

internal enum class PlaybackVectorError {
    ObjectFields, StringType, ArrayType, IntegerType, IntegerBounds, BooleanType,
    Source, WorkKey, Speed, Position, Intent, Engine, Interruption, PauseCause, Status,
    StatusCause, EventType, EffectType, Rejection, Format, Contract, VectorId, EmptySteps,
    VectorFile, DuplicateFile, Digest, Inventory,
}

internal class PlaybackVectorDecodeException(val reason: PlaybackVectorError) : IllegalArgumentException(reason.name)

internal data class PlaybackVectorSnapshot(
    val state: PlaybackState,
    val status: PlaybackStatus,
    val pauseCause: PlaybackPauseCause?,
) {
    companion object {
        fun of(state: PlaybackState): PlaybackVectorSnapshot = PlaybackVectorSnapshot(state, state.status, state.pauseCause)
    }
}

internal data class PlaybackVectorStep(
    val event: PlaybackEvent,
    val expected: PlaybackVectorSnapshot,
    val effects: List<PlaybackEffect>,
)

internal data class PlaybackVector(
    val id: String,
    val initial: PlaybackVectorSnapshot,
    val steps: List<PlaybackVectorStep>,
)

internal fun decodePlaybackVector(json: String): PlaybackVector = decodePlaybackVector(Json.parseToJsonElement(json))

internal fun decodePlaybackVector(value: JsonElement): PlaybackVector {
    val root = fields(value, "vector_format", "contract", "id", "initial_state", "steps")
    version(root)
    val id = string(root.getValue("id"))
    if ("$id.json" !in reviewedPlaybackHashes) fail(PlaybackVectorError.VectorId)
    val steps = array(root.getValue("steps"))
    if (steps.isEmpty()) fail(PlaybackVectorError.EmptySteps)
    return PlaybackVector(
        id = id,
        initial = snapshot(root.getValue("initial_state")),
        steps = steps.map { raw ->
            val step = fields(raw, "event", "expected")
            val expected = fields(step.getValue("expected"), "state", "effects")
            PlaybackVectorStep(
                event = event(step.getValue("event")),
                expected = snapshot(expected.getValue("state")),
                effects = array(expected.getValue("effects")).map(::effect),
            )
        },
    )
}

internal fun decodePlaybackIndex(json: String): Map<String, String> = decodePlaybackIndex(Json.parseToJsonElement(json))

internal fun decodePlaybackIndex(value: JsonElement): Map<String, String> {
    val root = fields(value, "vector_format", "contract", "vectors")
    version(root)
    val entries = array(root.getValue("vectors"))
    val result = linkedMapOf<String, String>()
    entries.forEach { raw ->
        val entry = fields(raw, "file", "sha256")
        val file = string(entry.getValue("file"))
        if (file !in reviewedPlaybackHashes) fail(PlaybackVectorError.VectorFile)
        if (file in result) fail(PlaybackVectorError.DuplicateFile)
        val digest = string(entry.getValue("sha256"))
        if (!Regex("[0-9a-f]{64}").matches(digest)) fail(PlaybackVectorError.Digest)
        result[file] = digest
    }
    if (result.size != reviewedPlaybackHashes.size) fail(PlaybackVectorError.Inventory)
    return result.toMap()
}

internal fun decodePlaybackEvent(value: JsonElement): PlaybackEvent = event(value)

internal fun decodePlaybackEffect(value: JsonElement): PlaybackEffect = effect(value)

private fun fail(reason: PlaybackVectorError): Nothing = throw PlaybackVectorDecodeException(reason)

private fun fields(value: JsonElement, vararg names: String): JsonObject {
    val objectValue = value as? JsonObject ?: fail(PlaybackVectorError.ObjectFields)
    if (objectValue.keys != names.toSet()) fail(PlaybackVectorError.ObjectFields)
    return objectValue
}

private fun string(value: JsonElement): String {
    val primitive = value as? JsonPrimitive ?: fail(PlaybackVectorError.StringType)
    if (!primitive.isString) fail(PlaybackVectorError.StringType)
    return primitive.content
}

private fun array(value: JsonElement): JsonArray = value as? JsonArray ?: fail(PlaybackVectorError.ArrayType)

private fun integer(value: JsonElement, minimum: Long, maximum: Long): Long {
    val primitive = value as? JsonPrimitive ?: fail(PlaybackVectorError.IntegerType)
    // serde_json preserves a raw -0 as a floating token; neither harness accepts it as integer.
    if (primitive.isString || primitive.content == "-0" || !Regex("-?(0|[1-9][0-9]*)").matches(primitive.content)) {
        fail(PlaybackVectorError.IntegerType)
    }
    val number = primitive.content.toLongOrNull() ?: fail(PlaybackVectorError.IntegerBounds)
    if (number !in minimum..maximum) fail(PlaybackVectorError.IntegerBounds)
    return number
}

private fun uint32(value: JsonElement): Long = integer(value, 0, MAX_PLAYBACK_GENERATION)

private fun uint16(value: JsonElement): Int = integer(value, 0, 65_535).toInt()

private fun boolean(value: JsonElement): Boolean {
    val primitive = value as? JsonPrimitive ?: fail(PlaybackVectorError.BooleanType)
    if (primitive.isString) fail(PlaybackVectorError.BooleanType)
    return when (primitive.content) {
        "true" -> true
        "false" -> false
        else -> fail(PlaybackVectorError.BooleanType)
    }
}

private fun speed(value: JsonElement): Int {
    val decoded = uint16(value)
    if (!isSupportedPlaybackSpeed(decoded)) fail(PlaybackVectorError.Speed)
    return decoded
}

private fun workKey(value: JsonElement): String {
    val key = string(value)
    if (key !in setOf("son-ha-ky", "den-khuya", "mong-ho-diep")) fail(PlaybackVectorError.WorkKey)
    return key
}

private fun source(value: JsonElement): PlaybackSource {
    val root = fields(value, "work_id", "episode_index", "duration_ms")
    return PlaybackSource.of(
        workId = workKey(root.getValue("work_id")),
        episodeIndex = uint16(root.getValue("episode_index")),
        durationMs = uint32(root.getValue("duration_ms")),
    ) ?: fail(PlaybackVectorError.Source)
}

private fun identity(value: JsonElement): PlaybackIdentity {
    val root = fields(value, "work_id", "episode_index", "generation")
    return PlaybackIdentity.of(
        workId = workKey(root.getValue("work_id")),
        episodeIndex = uint16(root.getValue("episode_index")),
        generation = uint32(root.getValue("generation")),
    ) ?: fail(PlaybackVectorError.Source)
}

private fun cause(value: JsonElement): PlaybackPauseCause = when (string(value)) {
    "user" -> PlaybackPauseCause.User
    "interruption" -> PlaybackPauseCause.Interruption
    "cold_restore" -> PlaybackPauseCause.ColdRestore
    else -> fail(PlaybackVectorError.PauseCause)
}

private fun tagged(value: JsonElement): Pair<JsonObject, String> {
    val root = value as? JsonObject ?: fail(PlaybackVectorError.ObjectFields)
    val tag = root["type"] ?: fail(PlaybackVectorError.ObjectFields)
    return root to string(tag)
}

private fun intent(value: JsonElement): PlaybackIntent {
    val (root, tag) = tagged(value)
    return when (tag) {
        "play" -> {
            fields(root, "type")
            PlaybackIntent.Play
        }
        "pause" -> {
            fields(root, "type", "cause")
            PlaybackIntent.Pause(cause(root.getValue("cause")))
        }
        else -> fail(PlaybackVectorError.Intent)
    }
}

private fun engine(value: JsonElement): PlaybackEngine = when (string(value)) {
    "preparing" -> PlaybackEngine.Preparing
    "ready" -> PlaybackEngine.Ready
    "running" -> PlaybackEngine.Running
    "paused" -> PlaybackEngine.Paused
    "buffering" -> PlaybackEngine.Buffering
    "ended" -> PlaybackEngine.Ended
    else -> fail(PlaybackVectorError.Engine)
}

private fun interruption(value: JsonElement): PlaybackInterruption = when (string(value)) {
    "none" -> PlaybackInterruption.None
    "active" -> PlaybackInterruption.Active
    else -> fail(PlaybackVectorError.Interruption)
}

private fun session(value: JsonElement): PlaybackSession {
    val root = fields(value, "source", "position_ms", "speed_permille", "intent", "engine", "interruption")
    val source = source(root.getValue("source"))
    val position = uint32(root.getValue("position_ms"))
    if (position > source.durationMs) fail(PlaybackVectorError.Position)
    return PlaybackSession(
        source, position, speed(root.getValue("speed_permille")), intent(root.getValue("intent")),
        engine(root.getValue("engine")), interruption(root.getValue("interruption")),
    )
}

private fun snapshot(value: JsonElement): PlaybackVectorSnapshot {
    val root = fields(value, "generation", "session", "status", "pause_cause")
    val pauseCause = root.getValue("pause_cause").let { if (it == JsonNull) null else cause(it) }
    val status = when (string(root.getValue("status"))) {
        "idle" -> PlaybackStatus.Idle
        "loading" -> PlaybackStatus.Loading
        "paused" -> PlaybackStatus.Paused
        "simulating" -> PlaybackStatus.Simulating
        "buffering" -> PlaybackStatus.Buffering
        "ended" -> PlaybackStatus.Ended
        else -> fail(PlaybackVectorError.Status)
    }
    if ((status == PlaybackStatus.Paused) != (pauseCause != null)) fail(PlaybackVectorError.StatusCause)
    val state = PlaybackState(
        generation = uint32(root.getValue("generation")),
        session = root.getValue("session").let { if (it == JsonNull) null else session(it) },
    )
    return PlaybackVectorSnapshot(state, status, pauseCause)
}

private fun resume(value: JsonElement): PlaybackSnapshot {
    val root = fields(value, "source", "position_ms", "speed_permille")
    return PlaybackSnapshot(source(root.getValue("source")), uint32(root.getValue("position_ms")), speed(root.getValue("speed_permille")))
}

private fun event(value: JsonElement): PlaybackEvent {
    val (root, tag) = tagged(value)
    return when (tag) {
        "play_requested", "pause_requested", "interruption_began" -> {
            fields(root, "type")
            when (tag) {
                "play_requested" -> PlaybackEvent.PlayRequested
                "pause_requested" -> PlaybackEvent.PauseRequested
                else -> PlaybackEvent.InterruptionBegan
            }
        }
        "source_selected" -> {
            fields(root, "type", "source", "position_ms")
            PlaybackEvent.SourceSelected(source(root.getValue("source")), uint32(root.getValue("position_ms")))
        }
        "restore_requested" -> {
            fields(root, "type", "snapshot")
            PlaybackEvent.RestoreRequested(resume(root.getValue("snapshot")))
        }
        "seek_requested" -> {
            fields(root, "type", "position_ms")
            PlaybackEvent.SeekRequested(uint32(root.getValue("position_ms")))
        }
        "skip_requested" -> {
            fields(root, "type", "delta_ms")
            PlaybackEvent.SkipRequested(integer(root.getValue("delta_ms"), Int.MIN_VALUE.toLong(), Int.MAX_VALUE.toLong()).toInt())
        }
        "speed_requested" -> {
            fields(root, "type", "speed_permille")
            PlaybackEvent.SpeedRequested(uint16(root.getValue("speed_permille")))
        }
        "interruption_ended" -> {
            fields(root, "type", "resume_allowed")
            PlaybackEvent.InterruptionEnded(boolean(root.getValue("resume_allowed")))
        }
        "engine_ready", "engine_started", "engine_paused", "engine_buffering", "engine_ended" -> {
            fields(root, "type", "identity")
            val callback = identity(root.getValue("identity"))
            when (tag) {
                "engine_ready" -> PlaybackEvent.EngineReady(callback)
                "engine_started" -> PlaybackEvent.EngineStarted(callback)
                "engine_paused" -> PlaybackEvent.EnginePaused(callback)
                "engine_buffering" -> PlaybackEvent.EngineBuffering(callback)
                else -> PlaybackEvent.EngineEnded(callback)
            }
        }
        "engine_position" -> {
            fields(root, "type", "identity", "position_ms")
            PlaybackEvent.EnginePosition(identity(root.getValue("identity")), uint32(root.getValue("position_ms")))
        }
        else -> fail(PlaybackVectorError.EventType)
    }
}

private fun rejection(value: JsonElement): PlaybackRejection = when (string(value)) {
    "no_session" -> PlaybackRejection.NoSession
    "interrupted" -> PlaybackRejection.Interrupted
    "out_of_bounds" -> PlaybackRejection.OutOfBounds
    "unsupported_speed" -> PlaybackRejection.UnsupportedSpeed
    "generation_exhausted" -> PlaybackRejection.GenerationExhausted
    "invalid_source" -> PlaybackRejection.InvalidSource
    else -> fail(PlaybackVectorError.Rejection)
}

private fun effect(value: JsonElement): PlaybackEffect {
    val (root, tag) = tagged(value)
    return when (tag) {
        "prepare_engine", "start_engine", "pause_engine", "seek_engine", "set_speed" -> {
            fields(root, "type", "identity", "source", "position_ms", "speed_permille")
            val command = PlaybackEngineCommand(
                identity(root.getValue("identity")), source(root.getValue("source")),
                uint32(root.getValue("position_ms")), speed(root.getValue("speed_permille")),
            )
            when (tag) {
                "prepare_engine" -> PlaybackEffect.PrepareEngine(command)
                "start_engine" -> PlaybackEffect.StartEngine(command)
                "pause_engine" -> PlaybackEffect.PauseEngine(command)
                "seek_engine" -> PlaybackEffect.SeekEngine(command)
                else -> PlaybackEffect.SetSpeed(command)
            }
        }
        "offer_resume" -> {
            fields(root, "type", "source", "position_ms")
            PlaybackEffect.OfferResume(source(root.getValue("source")), uint32(root.getValue("position_ms")))
        }
        "rejected" -> {
            fields(root, "type", "reason")
            PlaybackEffect.Rejected(rejection(root.getValue("reason")))
        }
        else -> fail(PlaybackVectorError.EffectType)
    }
}

private fun version(root: JsonObject) {
    if (uint32(root.getValue("vector_format")) != 1L) fail(PlaybackVectorError.Format)
    if (string(root.getValue("contract")) != PLAYBACK_VECTOR_CONTRACT) fail(PlaybackVectorError.Contract)
}
