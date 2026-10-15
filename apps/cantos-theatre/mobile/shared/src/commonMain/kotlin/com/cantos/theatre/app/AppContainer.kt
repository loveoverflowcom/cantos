package com.cantos.theatre.app

import com.cantos.theatre.adapters.mock.CatalogScenario
import com.cantos.theatre.adapters.mock.MockCatalogPort
import com.cantos.theatre.adapters.mock.MockMediaEngine
import com.cantos.theatre.adapters.mock.mockInventory
import com.cantos.theatre.adapters.mock.sandbox.database.FakePersistentProgressPort
import com.cantos.theatre.adapters.mock.sandbox.database.FakeStorageConfig
import com.cantos.theatre.adapters.mock.sandbox.database.FakeStorageDatabase
import com.cantos.theatre.adapters.mock.sandbox.sessions.FakeStorageSession
import com.cantos.theatre.adapters.mock.sandbox.sessions.seed
import com.cantos.theatre.adapters.mock.seedListening
import com.cantos.theatre.adapters.mock.toEpisodeKey
import com.cantos.theatre.adapters.mock.toPlaybackSource
import com.cantos.theatre.adapters.mock.toWorkKey
import com.cantos.theatre.application.catalog.CatalogController
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.application.identity.toAccountView
import com.cantos.theatre.application.listening.ListeningController
import com.cantos.theatre.application.playback.PlaybackController
import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.application.preferences.PreferencesState
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoProgress
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.domain.listening.Bookmark
import com.cantos.theatre.domain.listening.ListeningEvent
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackSnapshot
import com.cantos.theatre.domain.playback.PlaybackSource
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.domain.publication.CatalogInventory
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.ports.CatalogPort
import com.cantos.theatre.ports.MediaEngine
import com.cantos.theatre.ports.ProgressPort
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

enum class RestoreOutcome { Refused, Requested, NothingSaved }

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
    /** Optional process-owned identity boundary. This container observes it and never closes it. */
    val account: NativeAccountController? = null,
) {
    private val ownerJob = SupervisorJob(parentScope.coroutineContext[Job])
    private val scope = CoroutineScope(parentScope.coroutineContext.minusKey(Job) + ownerJob)
    val catalog = CatalogController(scope, catalogPort, start.inventory)
    val listening = ListeningController(scope, progressPort, start.listening)
    val preferences = PreferencesController(start.preferences)
    val playback = PlaybackController(scope, mediaEngine, ::recordCommittedPlayback)
    private val mutableNavigation = MutableStateFlow(start.navigation)
    val navigation: StateFlow<NavigationState> = mutableNavigation.asStateFlow()
    /** Null when no identity boundary is configured: a guest-only shell, never a failure. */
    val accountState: StateFlow<NativeAccountState?> = account?.state ?: MutableStateFlow(null)
    private var closed = false
    private var resetEpoch = 0L
    private var initialRestorePending = true
    private var progressResetPending = false

    init {
        // Restoring the mounted player is not a listening visit; preserve explicit empty seeds.
        playback.restoreForReset(start.playback) { initialRestorePending = false }
        if (start.wantsSimulation) playback.dispatch(PlaybackEvent.PlayRequested)
        // A confirmed session closes an auth intent that is still open. With none open (Back,
        // another destination, an explicit restore) this is inert, so a late completion can
        // never navigate or reopen anything.
        account?.let { owner ->
            scope.launch {
                owner.state.collect { if (it is NativeAccountState.SignedIn) navigate(NavigationEvent.AuthCompleted) }
            }
        }
    }

    fun navigate(event: NavigationEvent) {
        if (!closed) mutableNavigation.update { reduceNavigation(it, event) }
    }

    /** Back closes the open auth intent first: a pending login is cancelled, so its callback is fenced. */
    fun back() {
        if (closed) return
        if (navigation.value.route.isAuth) cancelLogin()
        navigate(NavigationEvent.Back)
    }

    /** Only an explicit listener intent opens Login, and a signed-in listener has none. */
    fun openLogin(): Boolean {
        val owner = account ?: return false
        if (closed || owner.state.value is NativeAccountState.SignedIn) return false
        // A failure from an earlier attempt is not the result of this one.
        if (owner.state.value is NativeAccountState.Failed) owner.cancelLogin()
        navigate(NavigationEvent.OpenLogin)
        return true
    }

    /** Registration is an explained-unavailable destination: it never reaches the identity owner. */
    fun openRegister(): Boolean {
        val owner = account ?: return false
        val state = owner.state.value
        if (closed || state is NativeAccountState.SignedIn || state == NativeAccountState.Pending) return false
        navigate(NavigationEvent.OpenRegister)
        return true
    }

    /** One attempt, only from the open Login route, and never while another is pending or cleanup is owed. */
    fun beginLogin(): Boolean {
        val owner = account ?: return false
        if (closed || navigation.value.route != TheatreRoute.Login) return false
        if (!owner.state.value.toAccountView().canBeginLogin) return false
        owner.beginLogin()
        return true
    }

    /**
     * An explicit restore of the credential this device still holds; never automatic. The owner
     * settles a missing credential synchronously, so a [NativeAccountState.SignedOut] right after
     * the call means there was nothing to restore and the screen can say so.
     */
    fun restoreSession(): RestoreOutcome {
        val owner = account ?: return RestoreOutcome.Refused
        if (closed || navigation.value.route != TheatreRoute.Login || !owner.state.value.toAccountView().canBeginLogin) {
            return RestoreOutcome.Refused
        }
        owner.restore()
        return if (owner.state.value == NativeAccountState.SignedOut) RestoreOutcome.NothingSaved else RestoreOutcome.Requested
    }

    /** Abandons a pending attempt and stays on the entry; the browser is not reopened. */
    fun cancelLogin() {
        val owner = account ?: return
        if (!closed && owner.state.value == NativeAccountState.Pending) owner.cancelLogin()
    }

    /** Also retries a cleanup that failed: the owner hides the account before any I/O. */
    fun logout() {
        val owner = account ?: return
        val state = owner.state.value
        if (!closed && (state is NativeAccountState.SignedIn || state == NativeAccountState.CleanupRequired)) owner.logout()
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

    fun setSaved(work: WorkKey, saved: Boolean) {
        if (!closed) listening.dispatch(ListeningEvent.SetSaved(work, saved))
    }

    fun clearHistory() {
        if (!closed) listening.dispatch(ListeningEvent.ClearHistory)
    }

    /** Resolve current progress by exact episode identity and open paused, never auto-play. */
    fun resumeEpisode(key: EpisodeKey): Boolean = requestResume(key, requireHistory = false)

    /** A cleared or replaced history row cannot act as a captured resume capability. */
    fun resumeHistoryEpisode(key: EpisodeKey): Boolean = requestResume(key, requireHistory = true)

    private fun requestResume(key: EpisodeKey, requireHistory: Boolean): Boolean {
        if (closed || progressResetPending) return false
        val snapshot = listening.state.value.snapshot
        if (requireHistory && snapshot.history.none { it.episode == key }) return false
        if (!requireHistory && snapshot.progress.asReversed().firstOrNull { it.episode.work == key.work }?.episode != key) {
            return false
        }
        val record = snapshot.progress.firstOrNull { it.episode == key } ?: return false
        val episode = catalog.state.value.lastKnownInventory.episode(key) ?: return false
        if (record.durationMs != episode.durationMs) return false
        val source = episode.toPlaybackSource() ?: return false
        val epoch = resetEpoch
        return playback.captureState { _, revision ->
            resolveResume(key, source, revision, epoch, requireHistory)
        }
    }

    private fun resolveResume(
        key: EpisodeKey,
        source: PlaybackSource,
        revision: Long,
        epoch: Long,
        requireHistory: Boolean,
    ) {
        if (closed || epoch != resetEpoch) return
        listening.captureProgress(key, requireHistory) { record ->
            if (closed || epoch != resetEpoch || record == null || record.durationMs != source.durationMs) {
                return@captureProgress
            }
            playback.restoreForResume(PlaybackSnapshot(source, record.positionMs, 1000), expectedRevision = revision) {
                if (!closed && epoch == resetEpoch) navigate(NavigationEvent.OpenPlayer)
            }
        }
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
        val epoch = resetEpoch
        progressResetPending = true
        changeScenario?.invoke(CatalogScenario.Ready)
        catalog.reset(resetStart.inventory)
        listening.reset(resetStart.listening)
        preferences.reset()
        navigate(NavigationEvent.Reset)
        playback.restoreForReset(resetStart.playback) {
            if (!closed && epoch == resetEpoch) {
                progressResetPending = false
            }
        }
    }

    /** Observe each committed transition, including starts too brief for a conflating UI flow. */
    private fun recordCommittedPlayback(before: PlaybackState, after: PlaybackState) {
        if (closed || initialRestorePending || progressResetPending) return
        if (before.session?.snapshot() != after.session?.snapshot() ||
            before.session?.intent != after.session?.intent ||
            after.status == PlaybackStatus.Simulating || after.status == PlaybackStatus.Ended
        ) {
            recordProgress(after)
        }
        if (before.status != PlaybackStatus.Simulating && after.status == PlaybackStatus.Simulating) {
            val session = after.session ?: return
            listening.dispatch(ListeningEvent.RecordStarted(ProgressRecord(
                EpisodeKey(WorkKey(session.source.workId), session.source.episodeIndex),
                session.positionMs, session.source.durationMs,
            )))
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
        /**
         * Every mock mount stores listening data in one fake storage database; [storage]
         * receives the exact listening seed the controller starts with. Reset restores the
         * [FakeStorageSession.Demo] preset.
         */
        fun mock(
            scope: CoroutineScope,
            seed: DemoState = FakeStorageSession.Demo.seed(),
            account: NativeAccountController? = null,
            progressConfig: FakeStorageConfig = FakeStorageConfig(),
            storage: (ListeningSnapshot) -> FakeStorageDatabase = ::FakeStorageDatabase,
        ): AppContainer {
            val inventory = mockInventory()
            val start = startState(inventory, seed)
            val defaults = startState(inventory, FakeStorageSession.Demo.seed())
            val port = MockCatalogPort(inventory, seed.catalogMode.toScenario())
            val container = AppContainer(
                scope, port, FakePersistentProgressPort(storage(start.listening), progressConfig), MockMediaEngine(), start, defaults,
                changeScenario = port::setScenario, account = account,
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
    val listening = seedListening(inventory, key, position, seed.savedWorks, seed.bookmarks)
    return AppStartState(
        inventory = inventory,
        listening = ListeningSnapshot(
            savedWorks = listening.savedWorks,
            bookmarks = listening.bookmarks,
            progress = seed.progress.mapNotNull { it.toProgressRecord(inventory) }
                .asReversed().distinctBy { it.episode }.asReversed(),
            history = seed.history.mapNotNull { it.toProgressRecord(inventory) }.distinctBy { it.episode.work },
        ),
        playback = PlaybackSnapshot(source, position, seed.speedPercent * 10),
        navigation = NavigationState(seed.stack.map { it.toTheatreRoute() }),
        preferences = PreferencesState(
            if (seed.theme == DemoTheme.Dark) PreferenceTheme.Dark else PreferenceTheme.Light,
            if (seed.locale == DemoLocale.English) PreferenceLocale.English else PreferenceLocale.Vietnamese,
        ),
        wantsSimulation = seed.transportRequested,
    )
}

private fun DemoProgress.toProgressRecord(inventory: CatalogInventory): ProgressRecord? {
    val key = episodeId.toEpisodeKey()
    val episode = inventory.episode(key) ?: return null
    return ProgressRecord(key, positionMs.coerceIn(0L, episode.durationMs), episode.durationMs)
}

fun DemoRoute.toTheatreRoute(): TheatreRoute = when (this) {
    DemoRoute.Discovery -> TheatreRoute.Discovery
    DemoRoute.Search -> TheatreRoute.Search
    DemoRoute.Library -> TheatreRoute.Library
    DemoRoute.History -> TheatreRoute.History
    DemoRoute.Account -> TheatreRoute.Account
    DemoRoute.MembershipPreview -> TheatreRoute.MembershipPreview
    DemoRoute.Login -> TheatreRoute.Login
    DemoRoute.Register -> TheatreRoute.Register
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
