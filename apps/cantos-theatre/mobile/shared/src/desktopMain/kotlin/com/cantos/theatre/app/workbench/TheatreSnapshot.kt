package com.cantos.theatre.app.workbench

import com.cantos.theatre.adapters.mock.mockInventory
import com.cantos.theatre.adapters.mock.toEpisodeKey
import com.cantos.theatre.demo.DemoBookmark
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoLibraryTab
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoProgress
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.demo.DemoTimer
import com.cantos.theatre.demo.DemoWorkId
import com.cantos.theatre.demo.PlaybackUiStatus
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

/** No membership form/session data, native paths, service objects, or credentials are exported. */
class TheatreSnapshot(val recipe: TheatreRecipe, val state: DemoState)

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
    val bookmarks: List<PointWire>,
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
    val settings = recipe.settings
    val queryId = syntheticQueries.entries.firstOrNull { it.value == state.query }?.key
        ?: return TheatreSnapshotExport.UnsupportedState("Export supports declared synthetic queries only; clear the search first")
    if (state.listeningFailure != null || DemoRoute.MembershipPreview in state.stack ||
        state.playbackStatus !in setOf(PlaybackUiStatus.Paused, PlaybackUiStatus.Simulating) ||
        (state.playbackStatus == PlaybackUiStatus.Simulating) != state.transportRequested) {
        return TheatreSnapshotExport.UnsupportedState("Settle or reset transient playback/storage and membership state before export")
    }
    val encoded = snapshotJson.encodeToString(SnapshotWire(
        TheatreRecipe.Version, recipe.id, recipe.seed, settings.themeMode.name, settings.density.name,
        settings.locale.name, settings.fontScale, settings.constraints.widthDp, settings.constraints.heightDp,
        settings.reducedMotion, recipe.writeLatencyMs, recipe.steps.map { it.name },
        StateWire(state.stack.map(::routeId), state.selectedEpisode.name, state.positionMs,
            state.savedWorks.map { it.name }.sorted(),
            state.bookmarks.map { PointWire(it.episodeId.name, it.positionMs) },
            state.progress.map { PointWire(it.episodeId.name, it.positionMs) },
            state.history.map { PointWire(it.episodeId.name, it.positionMs) },
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
        TheatreSnapshotResult.Accepted(TheatreSnapshot(recipe, wire.state.toState()))
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

private fun StateWire.toState(): DemoState {
    require(stack.size in 1..32 && savedWorks.size <= 3 && bookmarks.size <= 40 && progress.size <= 40 && history.size <= 40)
    val query = requireNotNull(syntheticQueries[queryId])
    require(savedWorks.distinct().size == savedWorks.size)
    val episode = enumValue<DemoEpisodeId>(episode)
    validatePosition(episode, positionMs)
    fun point(point: PointWire): DemoProgress {
        val id = enumValue<DemoEpisodeId>(point.episode)
        validatePosition(id, point.positionMs)
        return DemoProgress(id, (point.positionMs / 1000L).toInt(), point.positionMs)
    }
    val progress = progress.map(::point)
    val history = history.map(::point)
    require(progress.distinctBy { it.episodeId }.size == progress.size)
    require(history.distinctBy { it.episodeId.workId }.size == history.size)
    return DemoState(
        stack = stack.map(::decodeRoute).also { require(DemoRoute.MembershipPreview !in it) }, selectedEpisode = episode,
        positionSeconds = (positionMs / 1000L).toInt(), positionMs = positionMs,
        savedWorks = savedWorks.map { enumValue<DemoWorkId>(it) }.toSet(),
        bookmarks = bookmarks.map(::point).map { DemoBookmark(it.episodeId, it.positionSeconds, it.positionMs) },
        progress = progress, history = history, libraryTab = enumValue(libraryTab), query = query,
        catalogMode = enumValue(catalogMode), theme = enumValue(theme), locale = enumValue(locale),
        speedPercent = speedPercent, timer = enumValue(timer), transportRequested = transportRequested,
        playbackStatus = if (transportRequested) PlaybackUiStatus.Simulating else PlaybackUiStatus.Paused,
    )
}

private val syntheticQueries = mapOf("empty" to "", "night" to "Đèn", "son-ha" to "Sơn Hà")

private fun validatePosition(id: DemoEpisodeId, positionMs: Long) {
    val episode = requireNotNull(mockInventory().episode(id.toEpisodeKey()))
    require(positionMs in 0L..episode.durationMs)
}

private fun routeId(route: DemoRoute): String = when (route) {
    DemoRoute.Discovery -> "Discovery"
    DemoRoute.Search -> "Search"
    DemoRoute.Library -> "Library"
    DemoRoute.Settings -> "Settings"
    DemoRoute.Account -> "Account"
    DemoRoute.MembershipPreview -> "MembershipPreview"
    DemoRoute.Login -> "Login"
    DemoRoute.Register -> "Register"
    DemoRoute.History -> "History"
    DemoRoute.Queue -> "Queue"
    DemoRoute.Player -> "Player"
    DemoRoute.Unavailable -> "Unavailable"
    is DemoRoute.Work -> "Work.${route.id.name}"
}

private fun decodeRoute(id: String): DemoRoute = when (id) {
    "Discovery" -> DemoRoute.Discovery
    "Search" -> DemoRoute.Search
    "Library" -> DemoRoute.Library
    "Settings" -> DemoRoute.Settings
    "Account" -> DemoRoute.Account
    "MembershipPreview" -> DemoRoute.MembershipPreview
    "Login" -> DemoRoute.Login
    "Register" -> DemoRoute.Register
    "History" -> DemoRoute.History
    "Queue" -> DemoRoute.Queue
    "Player" -> DemoRoute.Player
    "Unavailable" -> DemoRoute.Unavailable
    else -> if (id.startsWith("Work.")) DemoRoute.Work(enumValue(id.removePrefix("Work.")))
        else throw IllegalArgumentException("Unknown route")
}
