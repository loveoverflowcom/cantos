package com.cantos.theatre.app.workbench

import com.cantos.theatre.adapters.preview.PreviewEpisodeIds
import com.cantos.theatre.adapters.preview.PreviewFixtures
import com.cantos.theatre.app.PreviewNoSelection
import com.cantos.theatre.adapters.preview.PreviewWorkIds
import com.cantos.theatre.app.previewState
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.presentation.model.PlaybackUiStatus
import com.cantos.theatre.presentation.model.WorkListenPhase
import com.cantos.theatre.presentation.model.WorkListenRelease
import com.cantos.ui.Density
import com.cantos.ui.ThemeMode
import com.cantos.ui.preview.workbench.ConstraintFrame
import com.cantos.ui.preview.workbench.PreviewLocale
import com.cantos.ui.preview.workbench.PreviewSettings
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * No membership form/session data, native paths, service objects, or credentials are exported.
 * [signedIn] only says whether the sample account is the Library owner; the account itself is a
 * seed fact and never carries a credential.
 */
class TheatreSnapshot(
    val recipe: TheatreRecipe,
    val state: ShellState,
    val signedIn: Boolean = recipe.startsSignedIn,
    /** False while the Library has unsent writes, a failure or a refresh in flight. */
    val settled: Boolean = true,
)

sealed interface TheatreSnapshotExport {
    data class Exported(val text: String) : TheatreSnapshotExport
    data class UnsupportedState(val reason: String) : TheatreSnapshotExport
}

sealed interface TheatreSnapshotResult {
    data class Accepted(val snapshot: TheatreSnapshot) : TheatreSnapshotResult
    data class UnsupportedVersion(val version: Int) : TheatreSnapshotResult
    data class UnknownRecipe(val id: String) : TheatreSnapshotResult
    data class Invalid(val reason: String) : TheatreSnapshotResult
}

private val snapshotJson = Json { encodeDefaults = true; prettyPrint = true }
private const val MaxSnapshotCharacters = 65536

/** Allowlisted wire projection: opaque strings cannot become adapters or arbitrary routes. */
@Serializable
private data class SnapshotWire(
    val version: Int,
    val recipe: String,
    val seed: Int,
    val themeMode: String,
    val density: String,
    val locale: String,
    val fontScale: Float,
    val widthDp: Int,
    val heightDp: Int,
    val reducedMotion: Boolean,
    val writeLatencyMs: Long,
    val steps: List<String>,
    val signedIn: Boolean,
    val state: StateWire,
)

@Serializable
private data class PointWire(val episode: String, val positionMs: Long)

@Serializable
private data class StateWire(
    val stack: List<String>,
    val episode: String,
    val positionMs: Long,
    val savedWorks: List<String>,
    val progress: List<PointWire>,
    val history: List<PointWire>,
    val libraryTab: String,
    val queryId: String,
    val catalogMode: String,
    val theme: String,
    val locale: String,
    val speedPercent: Int,
    val timer: String,
    val transportRequested: Boolean,
)

fun encodeTheatreSnapshot(snapshot: TheatreSnapshot): TheatreSnapshotExport {
    val recipe = snapshot.recipe
    val state = snapshot.state
    if (state.stack.any { it is ShellRoute.SharedWork }) {
        return TheatreSnapshotExport.UnsupportedState("Private shared routes are never preview snapshots")
    }
    val settings = recipe.settings
    val queryId = syntheticQueries.entries.firstOrNull { it.value == state.query }?.key
        ?: return TheatreSnapshotExport.UnsupportedState("Export supports declared synthetic queries only; clear the search first")
    if (!snapshot.settled || state.listeningFailure != null || ShellRoute.MembershipPreview in state.stack ||
        state.playbackStatus !in setOf(PlaybackUiStatus.Idle, PlaybackUiStatus.Paused, PlaybackUiStatus.Playing) ||
        (state.playbackStatus == PlaybackUiStatus.Playing) != state.transportRequested) {
        return TheatreSnapshotExport.UnsupportedState("Settle or reset transient playback/storage and membership state before export")
    }
    val encoded = snapshotJson.encodeToString(SnapshotWire(
        TheatreRecipe.Version, recipe.id, recipe.seed, settings.themeMode.name, settings.density.name,
        settings.locale.name, settings.fontScale, settings.constraints.widthDp, settings.constraints.heightDp,
        settings.reducedMotion, recipe.writeLatencyMs, recipe.steps.map { it.name }, snapshot.signedIn,
        StateWire(state.stack.map(::routeId), state.selectedEpisode.value, state.positionMs,
            state.savedWorks.map { it.value }.sorted(),
            state.progress.map { PointWire(it.episodeId.value, it.positionMs) },
            state.history.map { PointWire(it.episodeId.value, it.positionMs) },
            state.libraryTab.name, queryId, state.catalogMode.name, state.theme.name, state.locale.name,
            state.speedPercent, state.timer.name, state.transportRequested),
    ))
    val validated = decodeTheatreSnapshot(encoded)
    return if (validated is TheatreSnapshotResult.Accepted && validated.snapshot.state == state) {
        TheatreSnapshotExport.Exported(encoded)
    } else {
        TheatreSnapshotExport.UnsupportedState("Synthetic UI state exceeds supported snapshot bounds")
    }
}

/** Invalid/unrecognized input returns facts; no default live or mock adapter is constructed. */
fun decodeTheatreSnapshot(text: String): TheatreSnapshotResult {
    if (text.length > MaxSnapshotCharacters) return TheatreSnapshotResult.Invalid("Snapshot exceeds 64 KiB")
    return try {
        val document = snapshotJson.parseToJsonElement(text).jsonObject
        val version = document["version"]?.jsonPrimitive?.intOrNull
            ?: return TheatreSnapshotResult.Invalid("Missing snapshot version")
        if (version != TheatreRecipe.Version) return TheatreSnapshotResult.UnsupportedVersion(version)
        val wire = snapshotJson.decodeFromString<SnapshotWire>(text)
        val settings = PreviewSettings(enumValue(wire.themeMode), enumValue(wire.density), enumValue(wire.locale),
            wire.fontScale, ConstraintFrame(wire.widthDp, wire.heightDp), wire.reducedMotion)
        val recipe = when (val selected = TheatreRecipe.select(wire.recipe, settings, wire.seed, wire.writeLatencyMs,
            wire.steps.map { enumValue<TheatreStep>(it) })) {
            is TheatreRecipeResult.Accepted -> selected.recipe
            is TheatreRecipeResult.UnknownRecipe -> return TheatreSnapshotResult.UnknownRecipe(selected.id)
            is TheatreRecipeResult.Invalid -> return TheatreSnapshotResult.Invalid(selected.reason)
        }
        TheatreSnapshotResult.Accepted(TheatreSnapshot(recipe, wire.state.toState(), wire.signedIn))
    } catch (_: SerializationException) {
        TheatreSnapshotResult.Invalid("Malformed snapshot fields")
    } catch (_: IllegalArgumentException) {
        TheatreSnapshotResult.Invalid("Unsupported synthetic state or settings")
    } catch (_: IllegalStateException) {
        TheatreSnapshotResult.Invalid("Malformed snapshot structure")
    }
}

private inline fun <reified E : Enum<E>> enumValue(value: String): E =
    enumValues<E>().firstOrNull { it.name == value } ?: throw IllegalArgumentException("Unknown enum value")

private fun StateWire.toState(): ShellState {
    require(stack.size in 1..32 && savedWorks.size <= 3 && progress.size <= 40 && history.size <= 40)
    val query = requireNotNull(syntheticQueries[queryId])
    require(savedWorks.distinct().size == savedWorks.size)
    val episode = episodeId(episode)
    require(episode != PreviewNoSelection || !transportRequested)
    validatePosition(episode, positionMs)
    fun point(point: PointWire): ShellProgress {
        val id = episodeId(point.episode)
        validatePosition(id, point.positionMs)
        return ShellProgress(id, (point.positionMs / 1000L).toInt(), point.positionMs)
    }
    val progress = progress.map(::point)
    val history = history.map(::point)
    require(progress.distinctBy { it.episodeId }.size == progress.size)
    require(history.distinctBy { it.episodeId.workId }.size == history.size)
    val selected = episode
    val restored = previewState(
        stack = stack.map(::decodeRoute).also { require(ShellRoute.MembershipPreview !in it) }, selectedEpisode = selected,
        positionMs = positionMs, savedWorks = savedWorks.map { workId(it) }.toSet(),
        progress = progress, history = history, libraryTab = enumValue(libraryTab), query = query,
        catalogMode = enumValue(catalogMode), theme = enumValue(theme), locale = enumValue(locale),
        speedPercent = speedPercent, timer = enumValue(timer), transportRequested = transportRequested,
        playbackStatus = when {
            transportRequested -> PlaybackUiStatus.Playing
            selected == PreviewNoSelection -> PlaybackUiStatus.Idle
            else -> PlaybackUiStatus.Paused
        },
    )
    // Derived from the same declared synthetic release as the restored player, never serialized
    // runtime authority or a guessed live publication. Keep the existing snapshot schema stable.
    val release = PreviewFixtures.playable(selected.value)?.audio?.let { audio ->
        WorkListenRelease(selected, audio.publicationId, when {
            transportRequested -> WorkListenPhase.PausePlaying
            positionMs > 0L -> WorkListenPhase.Resume
            else -> WorkListenPhase.Ready
        })
    }
    return restored.copy(workListenRelease = release)
}

private val syntheticQueries = mapOf("empty" to "", "night" to "Đèn", "son-ha" to "Sơn Hà")

private fun validatePosition(id: ShellEpisodeId, positionMs: Long) {
    if (id == PreviewNoSelection) return require(positionMs == 0L)
    // A position belongs to released audio; metadata without a release is refused as Invalid, never zero-length.
    val audio = PreviewFixtures.playable(id.value)?.audio
        ?: throw IllegalArgumentException("Snapshot position names no released audio")
    require(positionMs in 0L..audio.durationMs)
}

private fun routeId(route: ShellRoute): String = when (route) {
    ShellRoute.Discovery -> "Discovery"
    ShellRoute.Search -> "Search"
    ShellRoute.Library -> "Library"
    ShellRoute.Settings -> "Settings"
    ShellRoute.Account -> "Account"
    ShellRoute.MembershipPreview -> "MembershipPreview"
    ShellRoute.Login -> "Login"
    ShellRoute.Register -> "Register"
    // Only a received link opens it and the preview composes no verification owner.
    ShellRoute.VerifyEmail -> error("Email verification is never part of a preview snapshot")
    ShellRoute.History -> "History"
    ShellRoute.Queue -> "Queue"
    ShellRoute.Player -> "Player"
    ShellRoute.Unavailable -> "Unavailable"
    is ShellRoute.Work -> "Work.${route.id.value}"
    is ShellRoute.SharedWork -> error("Shared routes are rejected before snapshot encoding")
}

private fun decodeRoute(id: String): ShellRoute = when (id) {
    "Discovery" -> ShellRoute.Discovery
    "Search" -> ShellRoute.Search
    "Library" -> ShellRoute.Library
    "Settings" -> ShellRoute.Settings
    "Account" -> ShellRoute.Account
    "MembershipPreview" -> ShellRoute.MembershipPreview
    "Login" -> ShellRoute.Login
    "Register" -> ShellRoute.Register
    "History" -> ShellRoute.History
    "Queue" -> ShellRoute.Queue
    "Player" -> ShellRoute.Player
    "Unavailable" -> ShellRoute.Unavailable
    else -> if (id.startsWith("Work.")) ShellRoute.Work(workId(id.removePrefix("Work.")))
        else throw IllegalArgumentException("Unknown route")
}

private fun workId(value: String): ShellWorkId =
    PreviewWorkIds.entries.firstOrNull { it.value == value }
        ?: throw IllegalArgumentException("Unknown synthetic work identity")

private fun episodeId(value: String): ShellEpisodeId =
    if (value == PreviewNoSelection.value) PreviewNoSelection
    else PreviewEpisodeIds.entries.firstOrNull { it.value == value }
        ?: throw IllegalArgumentException("Unknown synthetic episode identity")
