package com.cantos.theatre.app

import com.cantos.theatre.application.catalog.DetailPhase
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.catalog.WalkPhase
import com.cantos.theatre.application.errors.ErrorAction
import com.cantos.theatre.application.errors.ErrorCenterState
import com.cantos.theatre.application.errors.ErrorEffect
import com.cantos.theatre.application.errors.ErrorEvent
import com.cantos.theatre.application.errors.ErrorKey
import com.cantos.theatre.application.errors.ErrorKind
import com.cantos.theatre.application.errors.ErrorNotice
import com.cantos.theatre.application.errors.ErrorOperation
import com.cantos.theatre.application.errors.reduceErrors
import com.cantos.theatre.application.errors.toErrorNotice
import com.cantos.theatre.application.identity.EmailVerificationController
import com.cantos.theatre.application.identity.LinkReceipt
import com.cantos.theatre.application.identity.ConfirmedSessionLoss
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.application.library.NativeLibraryController
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.application.identity.RegistrationController
import com.cantos.theatre.application.identity.toAccountView
import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.library.BookmarkSync
import com.cantos.theatre.domain.library.LibraryResume
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.library.valueOrNull
import com.cantos.theatre.domain.membership.MembershipEvent
import com.cantos.theatre.domain.membership.MembershipScreen
import com.cantos.theatre.domain.membership.MembershipState
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibrarySection
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellSearchKind
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellThemeMode
import com.cantos.theatre.presentation.model.ShellTimer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Route text a listener typed, the Library tab, the sleep timer and the Membership preview. */
data class TheatreUiInputs(
    val query: String = "",
    val searchKind: ShellSearchKind = ShellSearchKind.All,
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
    /** Membership starts unavailable; only a diagnostic host (the Desktop Preview) supplies a fixture. */
    initialMembership: MembershipState = MembershipState.unavailable(),
    /** Borrowed from the platform owner; no controller means an honest unavailable destination. */
    val registration: RegistrationController? = null,
    /** Borrowed process owner for completing an emailed signup; absent on hosts that claim no link. */
    val verification: EmailVerificationController? = null,
) {
    private val mutableNavigation = MutableStateFlow(NavigationState())
    val navigation: StateFlow<NavigationState> = mutableNavigation.asStateFlow()
    private val mutableUi = MutableStateFlow(TheatreUiInputs(membership = initialMembership))
    val ui: StateFlow<TheatreUiInputs> = mutableUi.asStateFlow()
    /** Counts explicit submits; a change skips the debounce for the text already typed. */
    private val searchSubmits = MutableStateFlow(0)
    private val mutableKnown = MutableStateFlow(KnownCatalog())
    val known: StateFlow<KnownCatalog> = mutableKnown.asStateFlow()
    private val mutableRequested = MutableStateFlow<UnitRequest?>(null)
    val requested: StateFlow<UnitRequest?> = mutableRequested.asStateFlow()
    private val mutableErrors = MutableStateFlow(ErrorCenterState())
    val errors: StateFlow<ErrorCenterState> = mutableErrors.asStateFlow()
    private var timerJob: Job? = null
    private var closed = false
    private var privateLoss: ConfirmedSessionLoss? = null

    /** The Library only for the verified account that owns it; a guest or stale actor sees none. */
    val library: StateFlow<LibraryState> = combine(
        runtime.account?.verifiedSession ?: flowOf(null),
        runtime.library?.state ?: flowOf(null),
    ) { verified, stored ->
        stored?.takeIf { verified != null && it.account == verified.session.actor } ?: LibraryState()
    }.stateIn(scope, SharingStarted.Eagerly, LibraryState())

    init {
        scope.launch { catalog.state.collect { state ->
            mutableKnown.update { it.merge(state) }
            fun observe(operation: ErrorOperation, failure: com.cantos.theatre.application.catalog.CatalogFailure?, hasContent: Boolean, settled: Boolean) {
                if (failure != null) onErrorEvent(ErrorEvent.Reported(failure.toErrorNotice(operation), hasContent))
                else if (settled) ErrorKind.entries.forEach { kind -> onErrorEvent(ErrorEvent.Resolved(ErrorKey(kind, operation))) }
            }
            val phase = state.works.phase
            observe(ErrorOperation.LoadCatalog, (phase as? com.cantos.theatre.application.catalog.WalkPhase.Failed)?.failure,
                state.works.items.isNotEmpty(), phase == com.cantos.theatre.application.catalog.WalkPhase.Complete || phase == com.cantos.theatre.application.catalog.WalkPhase.More)
            state.detail?.let { detail ->
                observe(ErrorOperation.LoadWork, (detail.phase as? com.cantos.theatre.application.catalog.DetailPhase.Failed)?.failure,
                    false, detail.phase is com.cantos.theatre.application.catalog.DetailPhase.Ready)
                detail.units?.let { walk ->
                    val units = walk.phase
                    observe(ErrorOperation.LoadUnits, (units as? com.cantos.theatre.application.catalog.WalkPhase.Failed)?.failure,
                        walk.items.isNotEmpty(), units == com.cantos.theatre.application.catalog.WalkPhase.Complete || units == com.cantos.theatre.application.catalog.WalkPhase.More)
                }
            }
        } }
        // The open work follows the navigation stack: the nearest Work route at or below the top.
        scope.launch { navigation.collect { syncDetail(it) } }
        scope.launch {
            var seenSubmits = 0
            combine(navigation, ui, searchSubmits) { nav, inputs, submits -> DesiredSearch(desiredQuery(nav, inputs.query, inputs.searchKind), submits) }
                .distinctUntilChanged()
                .collectLatest { (query, submits) ->
                    val submitted = submits != seenSubmits
                    seenSubmits = submits
                    if (query == null || query == catalog.state.value.query) return@collectLatest
                    if (query.text.isNotEmpty() && !submitted) delay(searchDebounceMs)
                    catalog.dispatch(PublicCatalogIntent.Query(query))
                }
        }
        scope.launch {
            runtime.listening.playback.state.collect { state ->
                val audio = state.session?.source?.audio
                val pending = mutableRequested.value
                if (audio != null && pending != null && audio.workId == pending.workId &&
                    audio.unitId == pending.unitId && audio.publicationId == pending.publicationId) {
                    mutableRequested.value = null
                    ErrorKind.entries.forEach { kind ->
                        onErrorEvent(ErrorEvent.Resolved(ErrorKey(kind, ErrorOperation.StartPlayback)))
                    }
                }
                // The end of an episode satisfies "end of episode"; the timer does not outlive it.
                if (state.status == com.cantos.theatre.domain.playback.PlaybackStatus.Ended && mutableUi.value.timer != ShellTimer.Off) setTimer(ShellTimer.Off)
            }
        }
        scope.launch {
            runtime.listening.failure.collect { failure ->
                if (failure != null) {
                    onErrorEvent(ErrorEvent.Reported(failure.toErrorNotice(ErrorOperation.StartPlayback), hasContent = false))
                }
            }
        }
        runtime.account?.let { owner ->
            scope.launch { owner.state.collect {
                if (closed) return@collect
                observeSessionLoss()
                if (it is NativeAccountState.SignedIn) {
                    ErrorKind.entries.forEach { kind ->
                        onErrorEvent(ErrorEvent.Resolved(ErrorKey(kind, ErrorOperation.SignIn)))
                    }
                    registration?.reset()
                    navigate(NavigationEvent.AuthCompleted)
                }
            } }
        }
    }

    /** Routes an [ErrorEvent] through the pure [reduceErrors] contract and executes any returned effect. */
    fun onErrorEvent(event: ErrorEvent) {
        if (closed) return
        val transition = reduceErrors(mutableErrors.value, event)
        mutableErrors.value = transition.state
        when (val effect = transition.effect) {
            null -> Unit
            is ErrorEffect.Retry -> when (effect.operation) {
                ErrorOperation.StartPlayback -> dispatch(ShellEvent.RetryListening)
                ErrorOperation.UpdateLibrary -> runtime.library?.let { if (library.value.pending > 0) it.drain() else it.refresh() }
                ErrorOperation.SignIn -> openLogin()
                ErrorOperation.LoadCatalog, ErrorOperation.LoadWork, ErrorOperation.LoadUnits -> retry()
            }
            ErrorEffect.OpenSignIn -> if (!openLogin()) navigate(NavigationEvent.Navigate(TheatreRoute.Account))
        }
    }

    // --- events ---------------------------------------------------------------------------------

    fun dispatch(event: ShellEvent) {
        if (closed) return
        when (event) {
            is ShellEvent.Navigate -> when (event.route) {
                ShellRoute.Login -> openLogin()
                ShellRoute.Register -> openRegister()
                // Only a received link opens completion; a route event never does.
                ShellRoute.VerifyEmail -> Unit
                else -> {
                    if (mutableNavigation.value.route == TheatreRoute.MembershipPreview || event.route == ShellRoute.MembershipPreview) {
                        membership(MembershipEvent.OpenIntro)
                    }
                    navigate(NavigationEvent.Navigate(event.route.toTheatreRoute()))
                    if (event.route == ShellRoute.Library || event.route == ShellRoute.History) {
                        if (runtime.account?.verifiedSession?.value != null) withPrivateLibrary { }
                        else promptExpiredPrivateAccess()
                    }
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
            ShellEvent.ClearHistory -> withPrivateLibrary { it.clearHistory() }
            ShellEvent.RetryListening -> mutableRequested.value?.let(::start)
            ShellEvent.Bookmark -> addBookmark()
            is ShellEvent.OpenBookmark -> openBookmark(event.bookmark.id)
            is ShellEvent.DeleteBookmark -> withPrivateLibrary { it.deleteBookmark(event.id) }
            ShellEvent.ToggleSimulation -> runtime.listening.playback.toggleIntent()
            is ShellEvent.Seek -> runtime.listening.playback.dispatch(PlaybackEvent.SkipRequested(
                (event.deltaSeconds.toLong() * 1000L).coerceIn(Int.MIN_VALUE.toLong(), Int.MAX_VALUE.toLong()).toInt()))
            is ShellEvent.SeekTo -> runtime.listening.playback.dispatch(PlaybackEvent.SeekRequested(event.seconds.toLong() * 1000L))
            is ShellEvent.SelectLibrary -> mutableUi.update { it.copy(libraryTab = event.tab) }
            is ShellEvent.Search -> mutableUi.update { it.copy(query = event.query) }
            // A chip is a deliberate choice: it reads now, like a submit, and keeps the typed text.
            is ShellEvent.SelectSearchKind -> {
                mutableUi.update { it.copy(searchKind = event.kind) }
                searchSubmits.update { it + 1 }
            }
            ShellEvent.SubmitSearch -> searchSubmits.update { it + 1 }
            ShellEvent.Retry -> retry()
            is ShellEvent.LoadMoreLibrary -> activeLibrary()?.nextPage(event.section.toDomain())
            is ShellEvent.RetryLibrary -> retryLibrary(event.section.toDomain())
            ShellEvent.LoadMore -> loadMore()
            ShellEvent.Refresh -> refresh()
            is ShellEvent.SetTheme -> preferences.setTheme(if (event.theme == ShellTheme.Dark) PreferenceTheme.Dark else PreferenceTheme.Light)
            is ShellEvent.SetThemeMode -> preferences.setTheme(when (event.mode) {
                ShellThemeMode.Light -> PreferenceTheme.Light
                ShellThemeMode.Dark -> PreferenceTheme.Dark
                ShellThemeMode.System -> PreferenceTheme.System
            })
            is ShellEvent.SetLocale -> preferences.setLocale(event.locale.toPreferenceLocale())
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
        if (closed) return
        val before = mutableNavigation.value
        val next = reduceNavigation(before, event)
        if (before.route == TheatreRoute.Login && next.route != TheatreRoute.Login) cancelLogin()
        if (before.route == TheatreRoute.Register && next.route != TheatreRoute.Register) registration?.leave()
        if (before.route == TheatreRoute.VerifyEmail && next.route != TheatreRoute.VerifyEmail) verification?.leave()
        mutableNavigation.value = next
        if (next.route == TheatreRoute.Register && before.route != TheatreRoute.Register) registration?.enter()
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

    /** Registration borrows its separate enrollment owner, never the account's signed-in state. */
    fun openRegister(): Boolean {
        val owner = runtime.account ?: return false
        val state = owner.state.value
        if (closed || state is NativeAccountState.SignedIn || state == NativeAccountState.Pending) return false
        navigate(NavigationEvent.OpenRegister)
        return true
    }

    /**
     * A URL the platform host received from a `VIEW` launch. Only the pinned handler is claimed; it
     * opens completion over the current destination, closing an open auth intent. The session, the
     * Library and the player are untouched, whoever is signed in.
     */
    fun receiveLink(url: String): LinkReceipt {
        val owner = verification
        if (closed || owner == null) return LinkReceipt.NotClaimed
        return owner.receive(url).also { if (it == LinkReceipt.Shown) navigate(NavigationEvent.OpenVerifyEmail) }
    }

    /**
     * After completion: a signed-out listener goes to Login. A signed-in listener keeps the current
     * session and goes to Account, where the explicit sign-out is; nothing signs out automatically.
     */
    fun continueAfterVerification() {
        if (closed || mutableNavigation.value.route != TheatreRoute.VerifyEmail) return
        if (runtime.account?.state?.value is NativeAccountState.SignedIn) navigate(NavigationEvent.Navigate(TheatreRoute.Account))
        else if (!openLogin()) navigate(NavigationEvent.Back)
    }

    /** One attempt, only from the open Login route, never while another is pending or cleanup is owed. */
    fun login(email: com.cantos.theatre.domain.identity.RegistrationEmail, password: String): Boolean {
        val owner = runtime.account ?: return false
        if (closed || mutableNavigation.value.route != TheatreRoute.Login || !owner.state.value.toAccountView().canBeginLogin) return false
        owner.login(email, password)
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
        if (promptExpiredPrivateAccess()) return
        if (!openLogin()) navigate(NavigationEvent.Navigate(TheatreRoute.Account))
    }

    private fun observeSessionLoss() {
        val loss = (runtime.account?.state?.value as? NativeAccountState.Failed)?.sessionLoss
        onErrorEvent(ErrorEvent.SessionObserved(loss))
        if (loss != null && privateLoss == loss && !mutableNavigation.value.route.isAuth) {
            onErrorEvent(ErrorEvent.PrivateAccessRequested)
        }
        if (runtime.account?.confirmedLoss == null) privateLoss = null
    }

    /** A private intent can request a prompt; a passive observation alone never does. */
    private fun promptExpiredPrivateAccess(): Boolean {
        val loss = runtime.account?.confirmedLoss
        if (loss != null) {
            // Cleanup can still be in flight: remember the requesting intent, but only the
            // eventual cleaned Failed state may promote it into an expiration dialog.
            privateLoss = loss
            observeSessionLoss()
            return true
        }
        if (runtime.account?.state?.value == NativeAccountState.CleanupRequired) {
            navigate(NavigationEvent.Navigate(TheatreRoute.Account))
            return true
        }
        return false
    }

    /** One authorization attempt before a new private intent; never replay a queued/uncertain write. */
    private fun withPrivateLibrary(action: (NativeLibraryController) -> Unit) {
        val account = runtime.account
        val lease = account?.verifiedSession?.value
        val library = activeLibrary()
        if (account == null || lease == null || library == null) { requireAccount(); return }
        val epoch = account.epoch
        scope.launch {
            val result = account.authorization(lease.session.session)
            if (closed) return@launch
            when (result) {
                is NativeApiResult.Accepted -> if (account.accepts(result.value) &&
                    result.value.intent == lease.intent && activeLibrary() === library) action(library)
                is NativeApiResult.Refused -> {
                    // An old A waiter cannot prompt or navigate B, even if B has since failed too.
                    if (account.confirmedLoss?.epoch == epoch) promptExpiredPrivateAccess()
                    else if (account.epoch == epoch && result.reason != NativeApiFailure.SessionChanged) {
                        val kind = when (result.reason) {
                            NativeApiFailure.Unavailable, NativeApiFailure.UnknownOutcome -> ErrorKind.ServiceUnavailable
                            NativeApiFailure.Protocol -> ErrorKind.Protocol
                            NativeApiFailure.Denied -> ErrorKind.Forbidden
                            NativeApiFailure.Unauthorized -> ErrorKind.Unauthenticated
                            NativeApiFailure.SessionChanged -> return@launch
                        }
                        onErrorEvent(ErrorEvent.Reported(ErrorNotice(kind, ErrorOperation.UpdateLibrary, retryable = false), hasContent = false))
                    }
                }
            }
        }
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
    private fun desiredQuery(navigation: NavigationState, text: String, kind: ShellSearchKind): PublicCatalogQuery? =
        if (navigation.route != TheatreRoute.Search) PublicCatalogQuery.All
        else (PublicCatalogQuery.of(text, kind.toPublic()) as? CatalogCheck.Valid)?.value

    private data class DesiredSearch(val query: PublicCatalogQuery?, val submits: Int)

    private fun onWorkRoute(): Boolean = mutableNavigation.value.stack.any { it is TheatreRoute.Work }

    private fun retry() {
        for (operation in listOf(ErrorOperation.LoadCatalog, ErrorOperation.LoadWork, ErrorOperation.LoadUnits)) {
            ErrorKind.entries.forEach { kind -> onErrorEvent(ErrorEvent.Resolved(ErrorKey(kind, operation))) }
        }
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

    /** A failed next page repeats that page; any other failed read restarts only its own section. */
    private fun retryLibrary(section: LibrarySection) {
        val owner = activeLibrary() ?: return
        val state = library.value
        if (state.section(section).failure?.nextPage == true && state.cursors[section] != null) owner.nextPage(section) else owner.refresh(section)
    }

    private fun savedIds(): Set<String> = library.value.saved.filter { it.saved }.map { it.workId }.toSet()

    private fun setSaved(workId: String, saved: Boolean) {
        withPrivateLibrary { it.setSaved(workId, saved) }
    }

    /**
     * A guest is taken to sign in, never shown a bookmark that exists nowhere. For an account the audio identity and the
     * position come from one snapshot the player owner holds at this moment, so they always name the same release and
     * episode even when the listener changes episode right after pressing the control. The snapshot is read on the
     * owner's own loop, and the account is checked again there: a callback that outlives its account does nothing.
     */
    private fun addBookmark() {
        val account = runtime.account
        val lease = account?.verifiedSession?.value
        val owner = activeLibrary()
        if (account == null || lease == null || owner == null) { requireAccount(); return }
        // Queue capture at the press, before a later SourceSelected, without waiting for HTTP.
        runtime.listening.playback.captureBookmarkPoint { snapshot ->
            val audio = snapshot.source.audio ?: return@captureBookmarkPoint
            if (closed || activeLibrary() !== owner || account.verifiedSession.value?.intent != lease.intent) {
                return@captureBookmarkPoint
            }
            withPrivateLibrary { it.createBookmark(audio, snapshot.positionMs) }
        }
    }

    /**
     * Opens exactly the bookmark's stored release at its stored position through the one player owner, paused: the
     * listener presses Play (listing bookmarks never plays). A replaced or unavailable release is explained, never
     * substituted. If that very release is already loaded it is simply moved, keeping the listener's own intent.
     */
    private fun openBookmark(id: String) {
        val entry = library.value.bookmarks.firstOrNull { it.id == id } ?: return
        // A bookmark this device just made, not yet listed back, was made on the very release that was playing.
        val resume = if (entry.resume == LibraryResume.NeedsRefresh && (entry.sync == BookmarkSync.Pending || entry.sync == BookmarkSync.Offline)) {
            LibraryResume.ExactAvailable
        } else entry.resume
        when (resume) {
            LibraryResume.ExactAvailable -> {
                mutableRequested.value = null
                val playback = runtime.listening.playback
                if (playback.state.value.session?.source?.audio == entry.audio) {
                    playback.dispatch(PlaybackEvent.SeekRequested(entry.positionMs))
                } else {
                    val info = entry.presentation.valueOrNull
                    runtime.listening.restore(entry.audio, entry.positionMs, title = info?.unitTitle.orEmpty(), workTitle = info?.work?.title.orEmpty())
                }
                navigate(NavigationEvent.OpenPlayer)
            }
            LibraryResume.NeedsRefresh -> activeLibrary()?.refresh(LibrarySection.Bookmarks)
            LibraryResume.ReleaseReplaced, LibraryResume.AccessUnavailable -> navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
        }
    }

    private fun open(id: ShellEpisodeId) {
        val known = known.value
        val unit = known.unit(id.value)
        val work = known.work(id.workId.value)
        if (unit == null || work == null || unit.workId.value != id.workId.value) {
            navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
            return
        }
        val request = unitRequest(work, unit.unit) ?: return
        start(request)
        navigate(NavigationEvent.OpenPlayer)
    }

    private fun start(request: UnitRequest) {
        // UI recomposition can lag rapid input. A pending resolve of this exact release is one
        // operation; a failed resolve may still retry through the same owner.
        val pending = mutableRequested.value
        if (pending?.workId == request.workId && pending.unitId == request.unitId &&
            pending.publicationId == request.publicationId && runtime.listening.failure.value == null) return
        mutableRequested.value = request
        runtime.listening.open(request.workId, request.unitId, request.publicationId, request.title, request.workTitle)
    }

    /** Continue and History reopen the saved position paused, or ask the API again when it is stale. */
    private fun resume(id: ShellEpisodeId, fromHistory: Boolean) {
        val state = library.value
        val historyRow = state.history.firstOrNull { it.audio.unitId == id.value }
        // From History, only a Continue entry for the very release that row names may be resumed.
        val entry = state.continueListening.firstOrNull {
            it.audio.unitId == id.value && (!fromHistory || historyRow == null || it.audio.publicationId == historyRow.audio.publicationId)
        }
        if (entry != null) {
            when (entry.resume) {
                LibraryResume.ExactAvailable -> {
                    mutableRequested.value = null
                    // Titles only when the API confirmed them for this exact release; never a guess.
                    val info = entry.presentation.valueOrNull
                    runtime.listening.restore(entry.audio, entry.positionMs, title = info?.unitTitle.orEmpty(), workTitle = info?.work?.title.orEmpty())
                    navigate(NavigationEvent.OpenPlayer)
                }
                LibraryResume.NeedsRefresh -> activeLibrary()?.refresh()
                LibraryResume.ReleaseReplaced, LibraryResume.AccessUnavailable ->
                    navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
            }
            return
        }
        val history = historyRow
        if (fromHistory && history != null && history.resume == LibraryResume.ExactAvailable) {
            // The stored release, named by the API, needs no catalog row to be reopened.
            val info = history.presentation.valueOrNull
            if (info != null) {
                start(UnitRequest(history.audio.workId, history.audio.unitId, history.audio.publicationId, info.unitTitle, info.work.title))
                navigate(NavigationEvent.OpenPlayer)
            } else if (known.value.unit(id.value)?.unit?.publicationId?.value == history.audio.publicationId) open(id)
            else navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
        } else navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
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
        cancelLogin()
        closed = true
        timerJob?.cancel()
        registration?.leave()
        verification?.leave()
    }
}

private fun ShellSearchKind.toPublic(): PublicWorkKind? = when (this) {
    ShellSearchKind.All -> null
    ShellSearchKind.Novel -> PublicWorkKind.Novel
    ShellSearchKind.Story -> PublicWorkKind.Story
    ShellSearchKind.Podcast -> PublicWorkKind.Podcast
}

private fun ShellLibrarySection.toDomain(): LibrarySection = when (this) {
    ShellLibrarySection.Continue -> LibrarySection.Continue
    ShellLibrarySection.Saved -> LibrarySection.Saved
    ShellLibrarySection.History -> LibrarySection.History
    ShellLibrarySection.Bookmarks -> LibrarySection.Bookmarks
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
    ShellRoute.VerifyEmail -> TheatreRoute.VerifyEmail
    ShellRoute.Settings -> TheatreRoute.Settings
    ShellRoute.Queue -> TheatreRoute.Queue
    ShellRoute.Player -> TheatreRoute.Player
    ShellRoute.Unavailable -> TheatreRoute.Unavailable
    is ShellRoute.Work -> TheatreRoute.Work(WorkKey(id.value))
}
