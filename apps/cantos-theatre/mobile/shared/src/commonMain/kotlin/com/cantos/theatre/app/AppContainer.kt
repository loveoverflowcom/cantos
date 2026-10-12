package com.cantos.theatre.app

import com.cantos.theatre.adapters.mock.CatalogScenario
import com.cantos.theatre.adapters.mock.MemoryProgressPort
import com.cantos.theatre.adapters.mock.MockCatalogPort
import com.cantos.theatre.adapters.mock.MockMediaEngine
import com.cantos.theatre.adapters.mock.mockInventory
import com.cantos.theatre.adapters.mock.seedListening
import com.cantos.theatre.adapters.mock.toEpisodeKey
import com.cantos.theatre.adapters.mock.toPlaybackSource
import com.cantos.theatre.adapters.mock.toWorkKey
import com.cantos.theatre.application.catalog.CatalogController
import com.cantos.theatre.application.listening.ListeningController
import com.cantos.theatre.application.playback.PlaybackController
import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.application.preferences.PreferencesState
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.domain.listening.Bookmark
import com.cantos.theatre.domain.listening.ListeningEvent
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackSnapshot
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.domain.publication.CatalogInventory
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.ports.CatalogPort
import com.cantos.theatre.ports.MediaEngine
import com.cantos.theatre.ports.ProgressPort
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineName
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Composition inputs only. This immutable seed is never a writable application store. */
data class AppStartState(
    val inventory: CatalogInventory,
    val listening: ListeningSnapshot,
    val playback: PlaybackSnapshot,
    val navigation: NavigationState = NavigationState(),
    val preferences: PreferencesState = PreferencesState(),
    val wantsSimulation: Boolean = false,
) {
    init {
        val episode = inventory.episode(EpisodeKey(WorkKey(playback.source.workId), playback.source.episodeIndex))
        require(episode != null && episode.durationMs == playback.source.durationMs) {
            "App seed must select a duration pinned by its synthetic catalog"
        }
    }
}

/**
 * One mounted application owns independent authorities and exactly one media engine.
 * Public intents and their consumers are confined to the host application dispatcher;
 * tests inject one virtual dispatcher. Ports never write these owners directly.
 */
class AppContainer(
    parentScope: CoroutineScope,
    catalogPort: CatalogPort,
    progressPort: ProgressPort,
    val mediaEngine: MediaEngine,
    start: AppStartState,
    private val resetStart: AppStartState = start,
    private val changeScenario: ((CatalogScenario) -> Unit)? = null,
) {
    private val ownerJob = SupervisorJob(parentScope.coroutineContext[Job])
    private val scope = CoroutineScope(parentScope.coroutineContext.minusKey(Job) + ownerJob)
    val catalog = CatalogController(scope, catalogPort, start.inventory)
    val listening = ListeningController(scope, progressPort, start.listening)
    val preferences = PreferencesController(start.preferences)
    val playback = PlaybackController(scope, mediaEngine)
    private val mutableNavigation = MutableStateFlow(start.navigation)
    val navigation: StateFlow<NavigationState> = mutableNavigation.asStateFlow()
    private var closed = false
    private var resetEpoch = 0L
    private var progressResetPending = false
    private var minimumProgressGeneration = 0L

    init {
        playback.dispatch(PlaybackEvent.RestoreRequested(start.playback))
        if (start.wantsSimulation) playback.dispatch(PlaybackEvent.PlayRequested)
        scope.launch(CoroutineName("theatre-progress")) {
            playback.state.collect { state ->
                if (!progressResetPending && state.generation >= minimumProgressGeneration) recordProgress(state)
            }
        }
    }

    fun navigate(event: NavigationEvent) {
        if (!closed) mutableNavigation.update { reduceNavigation(it, event) }
    }

    /** The catalog boundary refuses a missing/unsupported source before any engine command. */
    fun selectEpisode(key: EpisodeKey): Boolean {
        if (closed) return false
        val episode = catalog.state.value.lastKnownInventory.episode(key) ?: return false
        val source = episode.toPlaybackSource() ?: return false
        if (!playback.dispatch(PlaybackEvent.SourceSelected(source, 0L))) return false
        if (!playback.dispatch(PlaybackEvent.PlayRequested)) return false
        navigate(NavigationEvent.OpenPlayer)
        return true
    }

    fun bookmark() {
        if (closed) return
        val epoch = resetEpoch
        playback.captureBookmarkPoint { point ->
            if (!closed && epoch == resetEpoch) {
                listening.dispatch(ListeningEvent.AddBookmark(Bookmark(
                    EpisodeKey(WorkKey(point.source.workId), point.source.episodeIndex),
                    point.positionMs, point.source.durationMs,
                )))
            }
        }
    }

    /** Validate source and point together; a refused bookmark cannot seek the current episode. */
    fun selectBookmark(key: EpisodeKey, positionMs: Long): Boolean {
        if (closed) return false
        val episode = catalog.state.value.lastKnownInventory.episode(key) ?: return false
        if (positionMs !in 0L..episode.durationMs) return false
        val source = episode.toPlaybackSource() ?: return false
        if (!playback.dispatch(PlaybackEvent.SourceSelected(source, 0L))) return false
        if (!playback.dispatch(PlaybackEvent.PlayRequested)) return false
        if (!playback.dispatch(PlaybackEvent.SeekRequested(positionMs))) return false
        navigate(NavigationEvent.OpenPlayer)
        return true
    }

    fun setCatalogScenario(mode: DemoCatalogMode) {
        if (closed) return
        changeScenario?.invoke(mode.toScenario())
        catalog.refresh()
    }

    fun retryCatalog() {
        if (closed) return
        changeScenario?.invoke(CatalogScenario.Ready)
        catalog.refresh()
    }

    /** Restore allocates a fresh playback generation, fencing every pre-reset callback. */
    fun reset() {
        if (closed) return
        check(resetEpoch < Long.MAX_VALUE) { "Mock reset epoch exhausted" }
        resetEpoch += 1L
        progressResetPending = true
        changeScenario?.invoke(CatalogScenario.Ready)
        catalog.reset(resetStart.inventory)
        listening.reset(resetStart.listening)
        preferences.reset()
        navigate(NavigationEvent.Reset)
        playback.restoreForReset(resetStart.playback) { accepted ->
            minimumProgressGeneration = accepted.generation
            progressResetPending = false
            recordProgress(accepted)
        }
    }

    private fun recordProgress(state: PlaybackState) {
        if (closed) return
        val session = state.session ?: return
        listening.dispatch(ListeningEvent.RecordProgress(ProgressRecord(
            EpisodeKey(WorkKey(session.source.workId), session.source.episodeIndex),
            session.positionMs, session.source.durationMs,
        )))
    }

    fun close() {
        if (closed) return
        closed = true
        scope.cancel()
        playback.close()
    }

    companion object {
        fun mock(scope: CoroutineScope, seed: DemoState = DemoState()): AppContainer {
            val inventory = mockInventory()
            val start = startState(inventory, seed)
            val defaults = startState(inventory, DemoState())
            val port = MockCatalogPort(inventory, seed.catalogMode.toScenario())
            val container = AppContainer(
                scope, port, MemoryProgressPort(start.listening), MockMediaEngine(), start, defaults,
                changeScenario = port::setScenario,
            )
            if (seed.catalogMode != DemoCatalogMode.Ready) container.catalog.refresh()
            return container
        }
    }
}

private fun startState(inventory: CatalogInventory, seed: DemoState): AppStartState {
    val key = seed.selectedEpisode.toEpisodeKey()
    val episode = requireNotNull(inventory.episode(key)) { "Diagnostic seed must select a known fixture" }
    val source = requireNotNull(episode.toPlaybackSource()) { "Diagnostic source must match the mock contract" }
    val position = seed.positionMs.coerceIn(0L, episode.durationMs)
    return AppStartState(
        inventory = inventory,
        listening = seedListening(inventory, key, position, seed.savedWorks, seed.bookmarks),
        playback = PlaybackSnapshot(source, position, seed.speedPercent * 10),
        navigation = NavigationState(seed.stack.map { it.toTheatreRoute() }),
        preferences = PreferencesState(
            if (seed.theme == DemoTheme.Dark) PreferenceTheme.Dark else PreferenceTheme.Light,
            if (seed.locale == DemoLocale.English) PreferenceLocale.English else PreferenceLocale.Vietnamese,
        ),
        wantsSimulation = seed.transportRequested,
    )
}

fun DemoRoute.toTheatreRoute(): TheatreRoute = when (this) {
    DemoRoute.Discovery -> TheatreRoute.Discovery
    DemoRoute.Search -> TheatreRoute.Search
    DemoRoute.Library -> TheatreRoute.Library
    DemoRoute.Settings -> TheatreRoute.Settings
    DemoRoute.Queue -> TheatreRoute.Queue
    DemoRoute.Player -> TheatreRoute.Player
    DemoRoute.Unavailable -> TheatreRoute.Unavailable
    is DemoRoute.Work -> TheatreRoute.Work(id.toWorkKey())
}

private fun DemoCatalogMode.toScenario(): CatalogScenario = when (this) {
    DemoCatalogMode.Ready -> CatalogScenario.Ready
    DemoCatalogMode.Loading -> CatalogScenario.Loading
    DemoCatalogMode.Empty -> CatalogScenario.Empty
    DemoCatalogMode.Error -> CatalogScenario.Error
}
