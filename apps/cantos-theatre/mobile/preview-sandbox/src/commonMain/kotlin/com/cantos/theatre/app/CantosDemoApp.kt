package com.cantos.theatre.app

import com.cantos.theatre.demo.demoState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import com.cantos.theatre.adapters.mock.toDemoEpisodeId
import com.cantos.theatre.adapters.mock.toDemoView
import com.cantos.theatre.adapters.mock.toDemoWorkId
import com.cantos.theatre.application.catalog.CatalogState
import com.cantos.theatre.application.catalog.CatalogStatus
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.toAccountView
import com.cantos.theatre.domain.membership.MembershipEvent
import com.cantos.theatre.application.listening.ListeningFailure
import com.cantos.theatre.application.listening.ListeningState
import com.cantos.theatre.application.preferences.PreferencesState
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesUpdate
import com.cantos.theatre.presentation.model.ShellBookmark
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellListeningFailure
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.PlaybackUiStatus
import com.cantos.theatre.domain.membership.MembershipState
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
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.app.WithDemoShellPresentation
import com.cantos.theatre.features.shell.presentation.WithDemoArtwork
import com.cantos.theatre.features.membership.presentation.MembershipPreview
import com.cantos.theatre.features.membership.presentation.MembershipHeader

/** Root composition: owners outlive every route; query, tab and timer selection stay local. */
@Composable
fun CantosDemoApp(
    modifier: Modifier = Modifier,
    initialState: ShellState = demoState(),
    textScale: Float = 1f,
    onThemeChanged: (Boolean) -> Unit = {},
    backHandler: @Composable (Boolean, () -> Unit) -> Unit = { _, _ -> },
    appContainer: AppContainer? = null,
    membershipPreviewState: MembershipState = MembershipState.unavailable(),
    appSession: DemoAppSession? = null,
    density: com.cantos.ui.Density = com.cantos.ui.Density.Comfortable,
    motionPreference: com.cantos.ui.MotionPreference = com.cantos.ui.MotionPreference.Reduced,
    // Only for the default container; an injected one already carries its own (or no) identity.
    account: NativeAccountController? = null,
) {
    require(appSession == null || appContainer == null || appSession.container === appContainer) {
        "An injected app session and app container must share the same owner"
    }
    val parentScope = rememberCoroutineScope()
    val binding = remember(initialState, appContainer, appSession, account) {
        AppCompositionBinding(appSession?.container ?: appContainer ?: mockAppContainer(parentScope, initialState, account = account),
            ownsContainer = appContainer == null && appSession == null)
    }
    val container = binding.container
    DisposableEffect(binding) { onDispose { if (binding.ownsContainer) container.close() } }
    val session = remember(initialState, container, membershipPreviewState, appSession) {
        appSession ?: DemoAppSession(container, initialState, membershipPreviewState)
    }
    DisposableEffect(session) { onDispose { if (appSession == null) session.close() } }
    val ui by session.ui.collectAsState()
    val navigation by container.navigation.collectAsState()
    val catalog by container.catalog.state.collectAsState()
    val listening by container.listening.state.collectAsState()
    val playback by container.playback.state.collectAsState()
    val preferences by container.preferences.state.collectAsState()
    val localeUpdate by container.preferences.localeUpdate.collectAsState()
    val projection = projectDemoState(
        navigation, catalog, listening, playback, preferences, ui.query, ui.libraryTab, ui.timer, session.seed,
    )
    val accountView = container.accountState.collectAsState().value.toAccountView()
    // A private draft or receipt must not outlive the account that made it (logout, expiry, switch);
    // a guest draft survives signing in, so Membership -> Login -> success resumes the same intent.
    val signedIn = accountView == AccountView.SignedIn
    var wasSignedIn by remember(container) { mutableStateOf(signedIn) }
    LaunchedEffect(signedIn) {
        if (wasSignedIn && !signedIn) session.membership(MembershipEvent.ClearSession)
        wasSignedIn = signedIn
    }
    // Feedback for an explicit restore that found nothing; it belongs to the Login entry only.
    var nothingSaved by remember(container) { mutableStateOf(false) }
    LaunchedEffect(navigation.route) { nothingSaved = false }
    val dispatch: (ShellEvent) -> Unit = session::dispatch
    // Every account intent is an explicit listener action; the container and owner decide the outcome.
    val onAuth: (AuthIntent) -> Unit = { intent ->
        when (intent) {
            AuthIntent.BeginLogin -> container.beginLogin()
            AuthIntent.CancelLogin -> container.cancelLogin()
            AuthIntent.RestoreSession -> nothingSaved = container.restoreSession() == RestoreOutcome.NothingSaved
            AuthIntent.RetryCleanup, AuthIntent.Logout -> container.logout()
            AuthIntent.OpenLogin -> container.openLogin()
            AuthIntent.OpenRegister -> container.openRegister()
            AuthIntent.OpenHome -> dispatch(ShellEvent.Navigate(ShellRoute.Discovery))
            AuthIntent.OpenLibrary -> dispatch(ShellEvent.Navigate(ShellRoute.Library))
            AuthIntent.OpenMembership -> dispatch(ShellEvent.Navigate(ShellRoute.MembershipPreview))
            AuthIntent.OpenSettings -> dispatch(ShellEvent.Navigate(ShellRoute.Settings))
            AuthIntent.ToggleTheme -> dispatch(ShellEvent.SetTheme(if (projection.theme == ShellTheme.Dark) ShellTheme.Light else ShellTheme.Dark))
            AuthIntent.Back -> dispatch(ShellEvent.Back)
        }
    }
    WithDemoArtwork {
        WithDemoShellPresentation {
            CantosPresentation(
                projection, catalog.lastKnownInventory.toDemoView(), dispatch, modifier, textScale, onThemeChanged, backHandler,
                membershipContent = {
                    MembershipPreview(ui.membership, projection.locale, session::membership,
                        onExplore = { dispatch(ShellEvent.Navigate(ShellRoute.Discovery)) })
                },
                membershipHeader = {
                    MembershipHeader(projection.locale, projection.theme,
                        onBack = { dispatch(ShellEvent.Back) },
                        onToggleTheme = { dispatch(ShellEvent.SetTheme(if (projection.theme == ShellTheme.Dark) ShellTheme.Light else ShellTheme.Dark)) })
                },
                membershipScreen = ui.membership.screen,
                accountContent = { loginFocus ->
                    AccountContent(projection.locale, projection.theme, accountView, onAuth, loginFocus,
                        previewActionIcons = true)
                },
                authContent = {
                    if (projection.route == ShellRoute.Register) RegisterScreen(projection.locale, onAuth, previewActionIcons = true)
                    else LoginScreen(projection.locale, accountView, nothingSaved, onAuth, previewActionIcons = true)
                },
                authHeader = {
                    AuthHeader(projection.locale, projection.theme, authReturnFor(projection.stack), onAuth)
                },
                density = density,
                motionPreference = motionPreference,
                localeSaveUnavailable = localeUpdate == PreferencesUpdate.LocaleStoreUnavailable,
            )
        }
    }
}

private class AppCompositionBinding(val container: AppContainer, val ownsContainer: Boolean)

/** Derived view only. There is no second writable store mirroring the capability owners. */
fun projectDemoState(
    navigation: NavigationState,
    catalog: CatalogState,
    listening: ListeningState,
    playback: PlaybackState,
    preferences: PreferencesState,
    query: String,
    libraryTab: ShellLibraryTab,
    timer: ShellTimer,
    seed: ShellState = demoState(),
): ShellState {
    val session = playback.session
    val episodeId = session?.source?.let {
        EpisodeKey(WorkKey(it.workId), it.episodeIndex).toDemoEpisodeId()
    } ?: seed.selectedEpisode
    return demoState(
        stack = navigation.stack.map { route -> when (route) {
            TheatreRoute.Discovery -> ShellRoute.Discovery
            TheatreRoute.Search -> ShellRoute.Search
            TheatreRoute.Library -> ShellRoute.Library
            TheatreRoute.Account -> ShellRoute.Account
            TheatreRoute.MembershipPreview -> ShellRoute.MembershipPreview
            TheatreRoute.Login -> ShellRoute.Login
            TheatreRoute.Register -> ShellRoute.Register
            TheatreRoute.History -> ShellRoute.History
            TheatreRoute.Settings -> ShellRoute.Settings
            TheatreRoute.Queue -> ShellRoute.Queue
            TheatreRoute.Player -> ShellRoute.Player
            TheatreRoute.Unavailable -> ShellRoute.Unavailable
            is TheatreRoute.Work -> route.key.toDemoWorkId()?.let { ShellRoute.Work(it) } ?: ShellRoute.Unavailable
        } },
        selectedEpisode = episodeId,
        positionSeconds = session?.positionMs?.div(1000L)?.toInt() ?: seed.positionSeconds,
        positionMs = session?.positionMs ?: seed.positionMs,
        speedPercent = session?.speedPermille?.div(10) ?: seed.speedPercent,
        savedWorks = listening.snapshot.savedWorks.mapNotNull { it.work.toDemoWorkId() }.toSet(),
        bookmarks = listening.snapshot.bookmarks.mapNotNull { bookmark ->
            bookmark.episode.toDemoEpisodeId()?.let { id ->
                ShellBookmark(id, (bookmark.positionMs / 1000L).toInt(), bookmark.positionMs)
            }
        },
        progress = listening.snapshot.progress.mapNotNull { point ->
            point.episode.toDemoEpisodeId()?.let { id ->
                ShellProgress(id, (point.positionMs / 1000L).toInt(), point.positionMs)
            }
        },
        history = listening.snapshot.history.mapNotNull { point ->
            point.episode.toDemoEpisodeId()?.let { id ->
                ShellProgress(id, (point.positionMs / 1000L).toInt(), point.positionMs)
            }
        },
        listeningFailure = when (listening.failure) {
            null -> null
            ListeningFailure.MemoryStoreUnavailable -> ShellListeningFailure.MemoryStoreUnavailable
        },
        catalogMode = when (catalog.status) {
            CatalogStatus.Ready -> ShellCatalogMode.Ready
            CatalogStatus.Loading -> ShellCatalogMode.Loading
            CatalogStatus.Empty -> ShellCatalogMode.Empty
            CatalogStatus.Unavailable -> ShellCatalogMode.Error
        },
        theme = if (preferences.theme == PreferenceTheme.Dark) ShellTheme.Dark else ShellTheme.Light,
        locale = preferences.locale.toShellLocale(),
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
