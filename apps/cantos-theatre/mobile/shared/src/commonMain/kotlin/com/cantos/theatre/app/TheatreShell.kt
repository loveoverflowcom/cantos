package com.cantos.theatre.app

import com.cantos.theatre.application.catalog.DetailPhase
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.catalog.WalkPhase
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.application.identity.toAccountView
import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.library.LibraryResume
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.membership.MembershipEvent
import com.cantos.theatre.domain.membership.MembershipScreen
import com.cantos.theatre.domain.membership.MembershipState
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellTimer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Route text a listener typed, the Library tab, the sleep timer and the Membership preview. */
data class TheatreUiInputs(
    val query: String = "",
    val libraryTab: ShellLibraryTab = ShellLibraryTab.Continuing,
    val timer: ShellTimer = ShellTimer.Off,
    val membership: MembershipState = MembershipState.unavailable(),
)

/**
 * The production counterpart of the sample session: it interprets the controls the listener
 * already knows ([ShellEvent]) against the real owners — the live catalog, the account, the
 * durable Library and the native player — and holds only navigation and typed route text.
 * Every other fact the screens show is read from those owners, never mirrored here.
 *
 * Confined to the application dispatcher, like the owners it borrows. It never closes them.
 */
class TheatreShell(
    private val scope: CoroutineScope,
    val catalog: PublicCatalogController,
    val runtime: NativeSessionRuntime,
    val preferences: PreferencesController,
    /** Debounce before typed search text becomes one API read. */
    private val searchDebounceMs: Long = 350,
    /** Sleep-timer durations, replaceable only by tests. */
    private val timerMinutesMs: (ShellTimer) -> Long = { if (it == ShellTimer.Fifteen) 15 * 60_000L else 30 * 60_000L },
) {
    private val mutableNavigation = MutableStateFlow(NavigationState())
    val navigation: StateFlow<NavigationState> = mutableNavigation.asStateFlow()
    private val mutableUi = MutableStateFlow(TheatreUiInputs())
    val ui: StateFlow<TheatreUiInputs> = mutableUi.asStateFlow()
    private val mutableKnown = MutableStateFlow(KnownCatalog())
    val known: StateFlow<KnownCatalog> = mutableKnown.asStateFlow()
    private val mutableRequested = MutableStateFlow<UnitRequest?>(null)
    val requested: StateFlow<UnitRequest?> = mutableRequested.asStateFlow()
    private var timerJob: Job? = null
    private var closed = false

    /** The Library only for the verified account that owns it; a guest or stale actor sees none. */
    val library: StateFlow<LibraryState> = combine(
        runtime.account?.verifiedSession ?: flowOf(null),
        runtime.library?.state ?: flowOf(null),
    ) { verified, stored ->
        stored?.takeIf { verified != null && it.account == verified.session.actor } ?: LibraryState()
    }.stateIn(scope, SharingStarted.Eagerly, LibraryState())

    init {
        scope.launch { catalog.state.collect { state -> mutableKnown.update { it.merge(state) } } }
        // The open work follows the navigation stack: the nearest Work route at or below the top.
        scope.launch { navigation.collect { syncDetail(it) } }
        scope.launch {
            combine(navigation, ui) { nav, inputs -> desiredQuery(nav, inputs.query) }
                .distinctUntilChanged()
                .collectLatest { query ->
                    if (query == null || query == catalog.state.value.query) return@collectLatest
                    if (query.text.isNotEmpty()) delay(searchDebounceMs)
                    catalog.dispatch(PublicCatalogIntent.Query(query))
                }
        }
        scope.launch {
            runtime.listening.playback.state.collect { state ->
                val unit = state.session?.source?.audio?.unitId
                if (unit != null && unit == mutableRequested.value?.unitId) mutableRequested.value = null
                // The end of an episode satisfies "end of episode"; the timer does not outlive it.
                if (state.status == com.cantos.theatre.domain.playback.PlaybackStatus.Ended && mutableUi.value.timer != ShellTimer.Off) setTimer(ShellTimer.Off)
            }
        }
        runtime.account?.let { owner ->
            scope.launch { owner.state.collect { if (it is NativeAccountState.SignedIn) navigate(NavigationEvent.AuthCompleted) } }
        }
    }

    // --- events ---------------------------------------------------------------------------------

    fun dispatch(event: ShellEvent) {
        if (closed) return
        when (event) {
            is ShellEvent.Navigate -> when (event.route) {
                ShellRoute.Login -> openLogin()
                ShellRoute.Register -> openRegister()
                else -> {
                    if (mutableNavigation.value.route == TheatreRoute.MembershipPreview || event.route == ShellRoute.MembershipPreview) {
                        membership(MembershipEvent.OpenIntro)
                    }
                    navigate(NavigationEvent.Navigate(event.route.toTheatreRoute()))
                }
            }
            ShellEvent.Back -> back()
            // Diagnostic-only controls of the sample session have no meaning against real owners.
            ShellEvent.Reset, is ShellEvent.CatalogMode -> Unit
            is ShellEvent.OpenEpisode -> open(event.id)
            is ShellEvent.ResumeEpisode -> resume(event.id, fromHistory = false)
            is ShellEvent.ResumeHistoryEpisode -> resume(event.id, fromHistory = true)
            is ShellEvent.SetSaved -> setSaved(event.id.value, event.saved)
            is ShellEvent.SaveWork -> setSaved(event.id.value, saved = event.id.value !in savedIds())
            ShellEvent.ClearHistory -> activeLibrary()?.clearHistory()
            ShellEvent.RetryListening -> mutableRequested.value?.let(::start)
            // The Library API has no bookmark resource; the control is not offered (see capabilities).
            is ShellEvent.OpenBookmark, ShellEvent.Bookmark -> Unit
            ShellEvent.ToggleSimulation -> runtime.listening.playback.toggleIntent()
            is ShellEvent.Seek -> runtime.listening.playback.dispatch(PlaybackEvent.SkipRequested(
                (event.deltaSeconds.toLong() * 1000L).coerceIn(Int.MIN_VALUE.toLong(), Int.MAX_VALUE.toLong()).toInt()))
            is ShellEvent.SeekTo -> runtime.listening.playback.dispatch(PlaybackEvent.SeekRequested(event.seconds.toLong() * 1000L))
            is ShellEvent.SelectLibrary -> mutableUi.update { it.copy(libraryTab = event.tab) }
            is ShellEvent.Search -> mutableUi.update { it.copy(query = event.query) }
            ShellEvent.Retry -> retry()
            ShellEvent.LoadMore -> loadMore()
            ShellEvent.Refresh -> refresh()
            is ShellEvent.SetTheme -> preferences.setTheme(if (event.theme == ShellTheme.Dark) PreferenceTheme.Dark else PreferenceTheme.Light)
            is ShellEvent.SetLocale -> preferences.setLocale(if (event.locale == ShellLocale.English) PreferenceLocale.English else PreferenceLocale.Vietnamese)
            is ShellEvent.SetSpeed -> runtime.listening.playback.dispatch(PlaybackEvent.SpeedRequested(event.percent * 10))
            is ShellEvent.SetTimer -> setTimer(event.timer)
        }
    }

    fun membership(event: MembershipEvent) {
        if (!closed) mutableUi.update { it.copy(membership = it.membership.reduce(event).state) }
    }

    fun clearMembership() = membership(MembershipEvent.ClearSession)

    // --- navigation and account -----------------------------------------------------------------

    fun navigate(event: NavigationEvent) {
        if (!closed) mutableNavigation.update { reduceNavigation(it, event) }
    }

    /** Back closes an open Login first (cancelling a pending attempt), then walks the typed stack. */
    private fun back() {
        val current = mutableUi.value.membership
        if (mutableNavigation.value.route == TheatreRoute.MembershipPreview) {
            if (current.screen != MembershipScreen.Intro && current.screen != MembershipScreen.Account) {
                membership(MembershipEvent.Back)
                return
            }
            membership(MembershipEvent.OpenIntro)
        }
        if (mutableNavigation.value.route.isAuth) cancelLogin()
        navigate(NavigationEvent.Back)
    }

    /** Only an explicit listener intent opens Login, and a signed-in listener has none. */
    fun openLogin(): Boolean {
        val owner = runtime.account ?: return false
        if (closed || owner.state.value is NativeAccountState.SignedIn) return false
        if (owner.state.value is NativeAccountState.Failed) owner.cancelLogin()
        navigate(NavigationEvent.OpenLogin)
        return true
    }

    /** Registration is an explained-unavailable destination; it never reaches the identity owner. */
    fun openRegister(): Boolean {
        val owner = runtime.account ?: return false
        val state = owner.state.value
        if (closed || state is NativeAccountState.SignedIn || state == NativeAccountState.Pending) return false
        navigate(NavigationEvent.OpenRegister)
        return true
    }

    /** One attempt, only from the open Login route, never while another is pending or cleanup is owed. */
    fun beginLogin(): Boolean {
        val owner = runtime.account ?: return false
        if (closed || mutableNavigation.value.route != TheatreRoute.Login) return false
        if (!owner.state.value.toAccountView().canBeginLogin) return false
        owner.beginLogin()
        return true
    }

    /** An explicit restore of the credential this device holds, never automatic. */
    fun restoreSession(): RestoreOutcome {
        val owner = runtime.account ?: return RestoreOutcome.Refused
        if (closed || mutableNavigation.value.route != TheatreRoute.Login || !owner.state.value.toAccountView().canBeginLogin) {
            return RestoreOutcome.Refused
        }
        owner.restore()
        return if (owner.state.value == NativeAccountState.SignedOut) RestoreOutcome.NothingSaved else RestoreOutcome.Requested
    }

    fun cancelLogin() {
        val owner = runtime.account ?: return
        if (!closed && owner.state.value == NativeAccountState.Pending) owner.cancelLogin()
    }

    /** Also retries a cleanup that failed: the owner hides the account before any I/O. */
    fun logout() {
        val owner = runtime.account ?: return
        val state = owner.state.value
        if (!closed && (state is NativeAccountState.SignedIn || state == NativeAccountState.CleanupRequired)) owner.logout()
    }

    /** A saved work needs an account: a guest is taken to sign in, or to the Account explanation. */
    private fun requireAccount() {
        if (!openLogin()) navigate(NavigationEvent.Navigate(TheatreRoute.Account))
    }

    // --- catalog --------------------------------------------------------------------------------

    private fun syncDetail(navigation: NavigationState) {
        val wanted = navigation.stack.lastOrNull { it is TheatreRoute.Work } as? TheatreRoute.Work
        val open = catalog.state.value.detail?.id?.value
        when {
            wanted == null && open != null -> catalog.dispatch(PublicCatalogIntent.Back)
            wanted != null && wanted.key.value != open -> {
                val id = (PublicWorkId.parse(wanted.key.value) as? CatalogCheck.Valid)?.value
                if (id == null) navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
                else known.value.work(id.value)?.let { catalog.dispatch(PublicCatalogIntent.OpenWork(it)) }
                    ?: catalog.dispatch(PublicCatalogIntent.OpenWorkId(id))
            }
        }
    }

    /** Typed text is a server search only on the Search route; everywhere else the full list. */
    private fun desiredQuery(navigation: NavigationState, text: String): PublicCatalogQuery? =
        if (navigation.route != TheatreRoute.Search) PublicCatalogQuery.All
        else (PublicCatalogQuery.of(text) as? CatalogCheck.Valid)?.value

    private fun onWorkRoute(): Boolean = mutableNavigation.value.stack.any { it is TheatreRoute.Work }

    private fun retry() {
        val state = catalog.state.value
        if (onWorkRoute() && state.detail != null) {
            val detail = state.detail
            catalog.dispatch(if (detail?.phase is DetailPhase.Failed) PublicCatalogIntent.RetryWork else PublicCatalogIntent.RetryUnits)
            return
        }
        catalog.dispatch(when (state.works.phase) {
            is WalkPhase.Failed -> PublicCatalogIntent.RetryWorks
            WalkPhase.More -> PublicCatalogIntent.LoadMoreWorks
            else -> PublicCatalogIntent.RefreshWorks
        })
    }

    private fun loadMore() {
        catalog.dispatch(if (onWorkRoute()) PublicCatalogIntent.LoadMoreUnits else PublicCatalogIntent.LoadMoreWorks)
    }

    fun refresh() {
        when {
            onWorkRoute() -> catalog.dispatch(PublicCatalogIntent.RefreshWork)
            mutableNavigation.value.route == TheatreRoute.Library -> { activeLibrary()?.refresh() }
            else -> catalog.dispatch(PublicCatalogIntent.RefreshWorks)
        }
    }

    // --- listening ------------------------------------------------------------------------------

    private fun activeLibrary() = runtime.library?.takeIf { library.value.account != null }

    private fun savedIds(): Set<String> = library.value.saved.filter { it.saved }.map { it.workId }.toSet()

    private fun setSaved(workId: String, saved: Boolean) {
        val owner = activeLibrary()
        if (owner == null) requireAccount() else owner.setSaved(workId, saved)
    }

    private fun open(id: ShellEpisodeId) {
        val known = known.value
        val unit = known.unit(id.value)
        val work = known.work(id.workId.value)
        if (unit == null || work == null || unit.workId.value != id.workId.value) {
            navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
            return
        }
        start(UnitRequest(work.id.value, unit.unit.id.value, unit.unit.publicationId.value, unit.unit.title, work.title))
        navigate(NavigationEvent.OpenPlayer)
    }

    private fun start(request: UnitRequest) {
        mutableRequested.value = request
        runtime.listening.open(request.workId, request.unitId, request.publicationId, request.title, request.workTitle)
    }

    /** Continue and History reopen the saved position paused, or ask the API again when it is stale. */
    private fun resume(id: ShellEpisodeId, fromHistory: Boolean) {
        val state = library.value
        val entry = state.continueListening.firstOrNull { it.audio.unitId == id.value }
        if (entry != null) {
            when (entry.resume) {
                LibraryResume.ExactAvailable -> {
                    mutableRequested.value = null
                    runtime.listening.restore(entry.audio, entry.positionMs)
                    navigate(NavigationEvent.OpenPlayer)
                }
                LibraryResume.NeedsRefresh -> activeLibrary()?.refresh()
                LibraryResume.ReleaseReplaced, LibraryResume.AccessUnavailable ->
                    navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
            }
            return
        }
        val history = state.history.firstOrNull { it.audio.unitId == id.value }
        if (fromHistory && history != null && history.resume == LibraryResume.ExactAvailable) open(id)
        else navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
    }

    private fun setTimer(timer: ShellTimer) {
        timerJob?.cancel()
        timerJob = null
        mutableUi.update { it.copy(timer = timer) }
        if (timer == ShellTimer.Fifteen || timer == ShellTimer.Thirty) {
            timerJob = scope.launch {
                delay(timerMinutesMs(timer))
                runtime.listening.playback.dispatch(PlaybackEvent.PauseRequested)
                mutableUi.update { it.copy(timer = ShellTimer.Off) }
            }
        }
    }

    /** Stops this binding only; the catalog, runtime and preferences owners outlive it. */
    fun close() {
        closed = true
        timerJob?.cancel()
    }
}

private fun ShellRoute.toTheatreRoute(): TheatreRoute = when (this) {
    ShellRoute.Discovery -> TheatreRoute.Discovery
    ShellRoute.Search -> TheatreRoute.Search
    ShellRoute.Library -> TheatreRoute.Library
    ShellRoute.History -> TheatreRoute.History
    ShellRoute.Account -> TheatreRoute.Account
    ShellRoute.MembershipPreview -> TheatreRoute.MembershipPreview
    ShellRoute.Login -> TheatreRoute.Login
    ShellRoute.Register -> TheatreRoute.Register
    ShellRoute.Settings -> TheatreRoute.Settings
    ShellRoute.Queue -> TheatreRoute.Queue
    ShellRoute.Player -> TheatreRoute.Player
    ShellRoute.Unavailable -> TheatreRoute.Unavailable
    is ShellRoute.Work -> TheatreRoute.Work(WorkKey(id.value))
}
