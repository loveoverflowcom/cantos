package com.cantos.theatre.app

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import com.cantos.theatre.adapters.mock.toDemoEpisodeId
import com.cantos.theatre.adapters.mock.toDemoView
import com.cantos.theatre.adapters.mock.toDemoWorkId
import com.cantos.theatre.adapters.mock.toEpisodeKey
import com.cantos.theatre.adapters.mock.toWorkKey
import com.cantos.theatre.application.catalog.CatalogState
import com.cantos.theatre.application.catalog.CatalogStatus
import com.cantos.theatre.application.listening.ListeningFailure
import com.cantos.theatre.application.listening.ListeningState
import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesState
import com.cantos.theatre.demo.DemoBookmark
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoEvent
import com.cantos.theatre.demo.DemoLibraryTab
import com.cantos.theatre.demo.DemoProgress
import com.cantos.theatre.demo.DemoListeningFailure
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.demo.DemoTimer
import com.cantos.theatre.demo.PlaybackUiStatus
import com.cantos.theatre.domain.listening.ListeningEvent
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.features.shell.presentation.CantosDemoPresentation

/** Root composition: owners outlive every route; query, tab and timer selection stay local. */
@Composable
fun CantosDemoApp(
    modifier: Modifier = Modifier,
    initialState: DemoState = DemoState(),
    textScale: Float = 1f,
    onThemeChanged: (Boolean) -> Unit = {},
    backHandler: @Composable (Boolean, () -> Unit) -> Unit = { _, _ -> },
    appContainer: AppContainer? = null,
) {
    val parentScope = rememberCoroutineScope()
    val container = remember(initialState, appContainer) {
        appContainer ?: AppContainer.mock(parentScope, initialState)
    }
    DisposableEffect(container) {
        onDispose { if (appContainer == null) container.close() }
    }
    var query by remember(initialState) { mutableStateOf(initialState.query) }
    var libraryTab by remember(initialState) { mutableStateOf(initialState.libraryTab) }
    // Selection only. It schedules no timer and is never sent to a media engine.
    var timer by remember(initialState) { mutableStateOf(initialState.timer) }
    val navigation by container.navigation.collectAsState()
    val catalog by container.catalog.state.collectAsState()
    val listening by container.listening.state.collectAsState()
    val playback by container.playback.state.collectAsState()
    val preferences by container.preferences.state.collectAsState()
    val projection = projectDemoState(
        navigation, catalog, listening, playback, preferences, query, libraryTab, timer, initialState,
    )
    val dispatch: (DemoEvent) -> Unit = { event ->
        when (event) {
            is DemoEvent.Navigate -> container.navigate(NavigationEvent.Navigate(event.route.toTheatreRoute()))
            DemoEvent.Back -> container.navigate(NavigationEvent.Back)
            DemoEvent.Reset -> {
                query = ""
                libraryTab = DemoLibraryTab.Continuing
                timer = DemoTimer.Off
                container.reset()
            }
            is DemoEvent.ResumeEpisode -> if (!container.resumeEpisode(event.id.toEpisodeKey())) {
                container.navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
            }
            is DemoEvent.ResumeHistoryEpisode -> if (!container.resumeHistoryEpisode(event.id.toEpisodeKey())) {
                container.navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
            }
            is DemoEvent.SetSaved -> container.setSaved(event.id.toWorkKey(), event.saved)
            DemoEvent.ClearHistory -> container.clearHistory()
            DemoEvent.RetryListening -> container.listening.retry()
            is DemoEvent.OpenEpisode -> container.selectEpisode(event.id.toEpisodeKey())
            is DemoEvent.OpenBookmark -> container.selectBookmark(
                event.bookmark.episodeId.toEpisodeKey(), event.bookmark.positionMs,
            )
            DemoEvent.ToggleSimulation -> container.playback.toggleIntent()
            is DemoEvent.Seek -> container.playback.dispatch(PlaybackEvent.SkipRequested(
                (event.deltaSeconds.toLong() * 1000L).coerceIn(Int.MIN_VALUE.toLong(), Int.MAX_VALUE.toLong()).toInt(),
            ))
            is DemoEvent.SeekTo -> container.playback.dispatch(PlaybackEvent.SeekRequested(event.seconds.toLong() * 1000L))
            is DemoEvent.SaveWork -> container.listening.dispatch(ListeningEvent.ToggleSaved(event.id.toWorkKey()))
            DemoEvent.Bookmark -> container.bookmark()
            is DemoEvent.SelectLibrary -> libraryTab = event.tab
            is DemoEvent.Search -> query = event.query
            is DemoEvent.CatalogMode -> container.setCatalogScenario(event.mode)
            DemoEvent.Retry -> container.retryCatalog()
            is DemoEvent.SetTheme -> container.preferences.setTheme(
                if (event.theme == DemoTheme.Dark) PreferenceTheme.Dark else PreferenceTheme.Light,
            )
            is DemoEvent.SetLocale -> container.preferences.setLocale(
                if (event.locale == DemoLocale.English) PreferenceLocale.English else PreferenceLocale.Vietnamese,
            )
            is DemoEvent.SetSpeed -> container.playback.dispatch(PlaybackEvent.SpeedRequested(event.percent * 10))
            is DemoEvent.SetTimer -> timer = event.timer
        }
    }
    CantosDemoPresentation(
        projection, catalog.lastKnownInventory.toDemoView(), dispatch, modifier, textScale, onThemeChanged, backHandler,
    )
}

/** Derived view only. There is no second writable store mirroring the capability owners. */
fun projectDemoState(
    navigation: NavigationState,
    catalog: CatalogState,
    listening: ListeningState,
    playback: PlaybackState,
    preferences: PreferencesState,
    query: String,
    libraryTab: DemoLibraryTab,
    timer: DemoTimer,
    seed: DemoState = DemoState(),
): DemoState {
    val session = playback.session
    val episodeId = session?.source?.let {
        EpisodeKey(WorkKey(it.workId), it.episodeIndex).toDemoEpisodeId()
    } ?: seed.selectedEpisode
    return DemoState(
        stack = navigation.stack.map { route -> when (route) {
            TheatreRoute.Discovery -> DemoRoute.Discovery
            TheatreRoute.Search -> DemoRoute.Search
            TheatreRoute.Library -> DemoRoute.Library
            TheatreRoute.Account -> DemoRoute.Account
            TheatreRoute.History -> DemoRoute.History
            TheatreRoute.Settings -> DemoRoute.Settings
            TheatreRoute.Queue -> DemoRoute.Queue
            TheatreRoute.Player -> DemoRoute.Player
            TheatreRoute.Unavailable -> DemoRoute.Unavailable
            is TheatreRoute.Work -> route.key.toDemoWorkId()?.let { DemoRoute.Work(it) } ?: DemoRoute.Unavailable
        } },
        selectedEpisode = episodeId,
        positionSeconds = session?.positionMs?.div(1000L)?.toInt() ?: seed.positionSeconds,
        positionMs = session?.positionMs ?: seed.positionMs,
        speedPercent = session?.speedPermille?.div(10) ?: seed.speedPercent,
        savedWorks = listening.snapshot.savedWorks.mapNotNull { it.work.toDemoWorkId() }.toSet(),
        bookmarks = listening.snapshot.bookmarks.mapNotNull { bookmark ->
            bookmark.episode.toDemoEpisodeId()?.let { id ->
                DemoBookmark(id, (bookmark.positionMs / 1000L).toInt(), bookmark.positionMs)
            }
        },
        progress = listening.snapshot.progress.mapNotNull { point ->
            point.episode.toDemoEpisodeId()?.let { id ->
                DemoProgress(id, (point.positionMs / 1000L).toInt(), point.positionMs)
            }
        },
        history = listening.snapshot.history.mapNotNull { point ->
            point.episode.toDemoEpisodeId()?.let { id ->
                DemoProgress(id, (point.positionMs / 1000L).toInt(), point.positionMs)
            }
        },
        listeningFailure = when (listening.failure) {
            null -> null
            ListeningFailure.MemoryStoreUnavailable -> DemoListeningFailure.MemoryStoreUnavailable
        },
        catalogMode = when (catalog.status) {
            CatalogStatus.Ready -> DemoCatalogMode.Ready
            CatalogStatus.Loading -> DemoCatalogMode.Loading
            CatalogStatus.Empty -> DemoCatalogMode.Empty
            CatalogStatus.Unavailable -> DemoCatalogMode.Error
        },
        theme = if (preferences.theme == PreferenceTheme.Dark) DemoTheme.Dark else DemoTheme.Light,
        locale = if (preferences.locale == PreferenceLocale.English) DemoLocale.English else DemoLocale.Vietnamese,
        playbackStatus = when (playback.status) {
            PlaybackStatus.Idle -> PlaybackUiStatus.Idle
            PlaybackStatus.Loading -> PlaybackUiStatus.Loading
            PlaybackStatus.Paused -> PlaybackUiStatus.Paused
            PlaybackStatus.Simulating -> PlaybackUiStatus.Simulating
            PlaybackStatus.Buffering -> PlaybackUiStatus.Buffering
            PlaybackStatus.Ended -> PlaybackUiStatus.Ended
        },
        transportRequested = session?.intent == PlaybackIntent.Play,
        query = query, libraryTab = libraryTab, timer = timer,
    )
}
