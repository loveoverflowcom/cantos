package com.cantos.theatre.app

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
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
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.toAccountView
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
import com.cantos.theatre.domain.membership.MembershipEvent
import com.cantos.theatre.domain.membership.MembershipScreen
import com.cantos.theatre.domain.membership.MembershipState
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.features.auth.presentation.AccountContent
import com.cantos.theatre.features.auth.presentation.AuthHeader
import com.cantos.theatre.features.auth.presentation.AuthIntent
import com.cantos.theatre.features.auth.presentation.LoginScreen
import com.cantos.theatre.features.auth.presentation.RegisterScreen
import com.cantos.theatre.features.auth.presentation.authReturnFor
import com.cantos.theatre.features.shell.presentation.CantosDemoPresentation
import com.cantos.theatre.features.membership.presentation.MembershipPreview
import com.cantos.theatre.features.membership.presentation.MembershipHeader

/** Root composition: owners outlive every route; query, tab and timer selection stay local. */
@Composable
fun CantosDemoApp(
    modifier: Modifier = Modifier,
    initialState: DemoState = DemoState(),
    textScale: Float = 1f,
    onThemeChanged: (Boolean) -> Unit = {},
    backHandler: @Composable (Boolean, () -> Unit) -> Unit = { _, _ -> },
    appContainer: AppContainer? = null,
    membershipPreviewState: MembershipState = MembershipState.unavailable(),
    // Only for the default container; an injected one already carries its own (or no) identity.
    account: NativeAccountController? = null,
) {
    val parentScope = rememberCoroutineScope()
    val container = remember(initialState, appContainer, account) {
        appContainer ?: AppContainer.mock(parentScope, initialState, account = account)
    }
    DisposableEffect(container) {
        onDispose { if (appContainer == null) container.close() }
    }
    var query by remember(initialState) { mutableStateOf(initialState.query) }
    var libraryTab by remember(initialState) { mutableStateOf(initialState.libraryTab) }
    // Selection only. It schedules no timer and is never sent to a media engine.
    var timer by remember(initialState) { mutableStateOf(initialState.timer) }
    // Separate, opt-in preview authority. No native identity/session is projected into it.
    var membership by remember(initialState, appContainer, membershipPreviewState) { mutableStateOf(membershipPreviewState) }
    val navigation by container.navigation.collectAsState()
    val catalog by container.catalog.state.collectAsState()
    val listening by container.listening.state.collectAsState()
    val playback by container.playback.state.collectAsState()
    val preferences by container.preferences.state.collectAsState()
    val accountView = container.accountState.collectAsState().value.toAccountView()
    // A private draft or receipt must not outlive the account that made it (logout, expiry, switch);
    // a guest draft survives signing in, so Membership -> Login -> success resumes the same intent.
    val signedIn = accountView == AccountView.SignedIn
    var wasSignedIn by remember(container) { mutableStateOf(signedIn) }
    LaunchedEffect(signedIn) {
        if (wasSignedIn && !signedIn) membership = membership.reduce(MembershipEvent.ClearSession).state
        wasSignedIn = signedIn
    }
    // Feedback for an explicit restore that found nothing; it belongs to the Login entry only.
    var nothingSaved by remember(container) { mutableStateOf(false) }
    LaunchedEffect(navigation.route) { nothingSaved = false }
    val projection = projectDemoState(
        navigation, catalog, listening, playback, preferences, query, libraryTab, timer, initialState,
    )
    val dispatch: (DemoEvent) -> Unit = { event ->
        when (event) {
            is DemoEvent.Navigate -> when (event.route) {
                // Auth opens only through the container's guarded intents, never as a bare route.
                DemoRoute.Login -> container.openLogin()
                DemoRoute.Register -> container.openRegister()
                else -> {
                    if (navigation.route == TheatreRoute.MembershipPreview || event.route == DemoRoute.MembershipPreview) {
                        membership = membership.reduce(MembershipEvent.OpenIntro).state
                    }
                    container.navigate(NavigationEvent.Navigate(event.route.toTheatreRoute()))
                }
            }
            DemoEvent.Back -> if (navigation.route == TheatreRoute.MembershipPreview &&
                membership.screen != MembershipScreen.Intro && membership.screen != MembershipScreen.Account) {
                membership = membership.reduce(MembershipEvent.Back).state
            } else {
                if (navigation.route == TheatreRoute.MembershipPreview) membership = membership.reduce(MembershipEvent.OpenIntro).state
                container.back()
            }
            DemoEvent.Reset -> {
                query = ""
                libraryTab = DemoLibraryTab.Continuing
                timer = DemoTimer.Off
                membership = membership.reduce(MembershipEvent.ClearSession).state
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
    // Every account intent is an explicit listener action; the container and owner decide the outcome.
    val onAuth: (AuthIntent) -> Unit = { intent ->
        when (intent) {
            AuthIntent.BeginLogin -> container.beginLogin()
            AuthIntent.CancelLogin -> container.cancelLogin()
            AuthIntent.RestoreSession -> nothingSaved = container.restoreSession() == RestoreOutcome.NothingSaved
            AuthIntent.RetryCleanup, AuthIntent.Logout -> container.logout()
            AuthIntent.OpenLogin -> container.openLogin()
            AuthIntent.OpenRegister -> container.openRegister()
            AuthIntent.OpenHome -> dispatch(DemoEvent.Navigate(DemoRoute.Discovery))
            AuthIntent.OpenLibrary -> dispatch(DemoEvent.Navigate(DemoRoute.Library))
            AuthIntent.OpenMembership -> dispatch(DemoEvent.Navigate(DemoRoute.MembershipPreview))
            AuthIntent.OpenSettings -> dispatch(DemoEvent.Navigate(DemoRoute.Settings))
            AuthIntent.ToggleTheme -> dispatch(DemoEvent.SetTheme(if (projection.theme == DemoTheme.Dark) DemoTheme.Light else DemoTheme.Dark))
            AuthIntent.Back -> dispatch(DemoEvent.Back)
        }
    }
    CantosDemoPresentation(
        projection, catalog.lastKnownInventory.toDemoView(), dispatch, modifier, textScale, onThemeChanged, backHandler,
        membershipContent = {
            MembershipPreview(membership, projection.locale, { membership = membership.reduce(it).state },
                onExplore = { dispatch(DemoEvent.Navigate(DemoRoute.Discovery)) })
        },
        membershipHeader = {
            MembershipHeader(projection.locale, projection.theme,
                onBack = { dispatch(DemoEvent.Back) },
                onToggleTheme = { dispatch(DemoEvent.SetTheme(if (projection.theme == DemoTheme.Dark) DemoTheme.Light else DemoTheme.Dark)) })
        },
        membershipScreen = membership.screen,
        accountContent = { loginFocus ->
            AccountContent(projection.locale, projection.theme, accountView, onAuth, loginFocus)
        },
        authContent = {
            if (projection.route == DemoRoute.Register) RegisterScreen(projection.locale, onAuth)
            else LoginScreen(projection.locale, accountView, nothingSaved, onAuth)
        },
        authHeader = {
            AuthHeader(projection.locale, projection.theme, authReturnFor(projection.stack), onAuth)
        },
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
            TheatreRoute.MembershipPreview -> DemoRoute.MembershipPreview
            TheatreRoute.Login -> DemoRoute.Login
            TheatreRoute.Register -> DemoRoute.Register
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
