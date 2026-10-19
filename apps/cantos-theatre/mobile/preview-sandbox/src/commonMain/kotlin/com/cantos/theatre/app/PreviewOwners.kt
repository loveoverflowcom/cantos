package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.PreviewAccount
import com.cantos.theatre.adapters.preview.PreviewCatalogPort
import com.cantos.theatre.adapters.preview.PreviewCatalogScenario
import com.cantos.theatre.adapters.preview.PreviewFixtures
import com.cantos.theatre.adapters.preview.PreviewIdentity
import com.cantos.theatre.adapters.preview.PreviewLibraryApi
import com.cantos.theatre.adapters.preview.PreviewListeningSeed
import com.cantos.theatre.adapters.preview.PreviewPlaybackAccess
import com.cantos.theatre.adapters.preview.PreviewStreamingEngine
import com.cantos.theatre.adapters.library.nativeLibraryController
import com.cantos.theatre.app.NativeSessionRuntime
import com.cantos.theatre.app.NavigationEvent
import com.cantos.theatre.app.TheatreRoute
import com.cantos.theatre.app.TheatreShell
import com.cantos.theatre.app.TheatreShellInputs
import com.cantos.theatre.app.projectTheatreShell
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.application.preferences.PreferencesState
import com.cantos.theatre.domain.membership.MembershipState
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.publication.WorkKey
import com.cantos.theatre.ports.LibraryLocalPort
import com.cantos.theatre.ports.LocalePreferenceStore
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellTimer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/** What a workbench scenario starts the real owners with. Everything is data; no owner is named. */
data class PreviewSeed(
    /** The shared presentation vocabulary: route stack, theme, locale, query, saved works, positions. */
    val state: ShellState,
    /** True when the listener starts signed in as the first sample account with the remote data below. */
    val signedIn: Boolean = false,
    /** Simulated remote Library write latency, in milliseconds. */
    val writeLatencyMs: Long = 0,
    /** True when the remote Library is unreachable: writes stay durable and visibly pending. */
    val remoteOffline: Boolean = false,
    val membership: MembershipState = MembershipState.unavailable(),
)

/**
 * The Cantos Theatre owners of one preview mount: the production catalog, account, Library,
 * player and preference controllers and the one [TheatreShell], wired to mocks only at their
 * external ports (public catalog, playback access, decoder, identity transport, remote Library
 * and locale storage). The owners are the same classes the Android and iOS hosts construct.
 *
 * Confined to [scope]'s dispatcher like the production composition. [close] ends every owner.
 */
class PreviewOwners(
    private val scope: CoroutineScope,
    val seed: PreviewSeed,
    store: LibraryLocalPort,
    localeStore: LocalePreferenceStore? = null,
    private val nowMs: () -> Long = { FixedNowMs },
    pageSize: Int = 50,
) {
    val catalogPort = PreviewCatalogPort(seed.state.catalogMode.toScenario(), pageSize)
    val access = PreviewPlaybackAccess(nowMs)
    val engine = PreviewStreamingEngine()
    val libraryApi = PreviewLibraryApi(nowMs, seed.writeLatencyMs).also { it.unavailable = seed.remoteOffline }
    private val firstAccount = PreviewAccount.Linh
    val identity = PreviewIdentity(scope, { nowMs() / 1000 }, firstAccount.takeIf { seed.signedIn })

    init {
        seedRemoteLibrary()
    }

    private var operations = 0
    val account get() = identity.account
    private val library = nativeLibraryController(
        scope, libraryApi, store, { uuid(9) },
        onSessionInvalidated = { expected -> account.revalidate(expected) },
        ownsAuthorization = { owner ->
            account.belongsToCurrent(owner.bearer, owner.session) &&
                account.verifiedSession.value?.session?.actor == owner.account
        },
        nowMs = nowMs,
    )
    val preferences = PreferencesController(
        PreferencesState(
            if (seed.state.theme == ShellTheme.Dark) PreferenceTheme.Dark else PreferenceTheme.Light,
            if (seed.state.locale == ShellLocale.English) PreferenceLocale.English else PreferenceLocale.Vietnamese,
        ),
        localeStore,
    )
    val catalog = PublicCatalogController(scope, catalogPort).also { it.dispatch(PublicCatalogIntent.Start) }
    val runtime = NativeSessionRuntime(
        scope, nativeFactory = { engine }, access = access, account = account, library = library,
        newUuid = { uuid(8) }, nowMs = nowMs,
        onAccountChanged = { catalog.dispatch(PublicCatalogIntent.AccountChanged) },
    )
    val shell = TheatreShell(scope, catalog, runtime, preferences, initialMembership = seed.membership)

    init {
        applyInteractionSeed()
    }

    private fun seedRemoteLibrary() {
        val state = seed.state
        libraryApi.seed(
            firstAccount.actor,
            saved = if (seed.signedIn) state.savedWorks.map { it.value }.toSet() else emptySet(),
            progress = if (seed.signedIn) state.progress.map(::seedOf) else emptyList(),
            // ShellState lists history newest first; the remote seed is oldest first.
            history = if (seed.signedIn) state.history.asReversed().map(::seedOf) else emptyList(),
        )
    }

    /** True while the verified owner of the Library is signed in, for snapshots and tests. */
    val signedIn: Boolean get() = identity.account.state.value is NativeAccountState.SignedIn

    /** The read-only presentation projection of the live owners, derived exactly as the screens derive it. */
    fun projection(): ShellState {
        val ui = shell.ui.value
        val prefs = preferences.state.value
        return projectTheatreShell(
            TheatreShellInputs(
                shell.navigation.value, catalog.state.value, shell.known.value, runtime.listening.playback.state.value,
                runtime.listening.selection.value, shell.requested.value, runtime.listening.failure.value != null,
                shell.library.value,
                if (prefs.theme == PreferenceTheme.Dark) ShellTheme.Dark else ShellTheme.Light,
                if (prefs.locale == PreferenceLocale.English) ShellLocale.English else ShellLocale.Vietnamese,
                ui.query, ui.libraryTab, ui.timer,
            ),
        ).state
    }

    private fun seedOf(point: ShellProgress) = PreviewListeningSeed(point.episodeId.value, point.positionMs)

    /** Navigation, typed text, tab, timer and a paused or playing selection, all through the shell's own events. */
    private fun applyInteractionSeed() {
        val state = seed.state
        state.stack.filterNot { it == ShellRoute.Login || it == ShellRoute.Register }
            .forEachIndexed { index, route ->
                if (index > 0 || route != ShellRoute.Discovery) shell.navigate(NavigationEvent.Navigate(route.toTheatreRoute()))
            }
        // Auth is an optional suffix opened by the shell's own guarded intents, as a listener would open it;
        // a signed-in seed has no Login destination.
        if (ShellRoute.Register in state.stack) shell.openRegister() else if (ShellRoute.Login in state.stack) shell.openLogin()
        if (state.query.isNotEmpty()) shell.dispatch(ShellEvent.Search(state.query))
        if (state.libraryTab != ShellLibraryTab.Continuing) shell.dispatch(ShellEvent.SelectLibrary(state.libraryTab))
        if (state.timer != ShellTimer.Off) shell.dispatch(ShellEvent.SetTimer(state.timer))
        selectionSeed()?.let { selected ->
            // A guest has no restored player (the runtime clears any session when the guest is established),
            // so the selection is made the way a listener makes it: open the unit, then settle it paused.
            scope.launch {
                val (work, unit) = checkNotNull(PreviewFixtures.unit(selected.value))
                val listening = runtime.listening
                listening.open(work.work.id.value, unit.id.value, unit.publicationId.value, unit.title, work.work.title)
                listening.playback.state.first { it.session?.source?.audio?.unitId == unit.id.value }
                if (!state.transportRequested) listening.playback.dispatch(PlaybackEvent.PauseRequested)
                if (state.positionMs > 0) listening.playback.dispatch(PlaybackEvent.SeekRequested(state.positionMs))
                if (state.speedPercent != 100) listening.playback.dispatch(PlaybackEvent.SpeedRequested(state.speedPercent * 10))
            }
        }
    }

    private fun selectionSeed() = seed.state.selectedEpisode.takeIf {
        PreviewFixtures.unit(it.value) != null && !seed.signedIn &&
            (seed.state.transportRequested || seed.state.positionMs > 0 || ShellRoute.Player in seed.state.stack)
    }

    private fun uuid(prefix: Int): String = "${prefix}0000000-0000-4000-8000-${(++operations).toString().padStart(12, '0')}"

    /** Ends the runtime (player, Library and account); the catalog and preference owners hold no resources. */
    fun close() {
        shell.close()
        runtime.close()
    }

    companion object {
        /** A fixed instant so captures and long-lived windows are deterministic. */
        const val FixedNowMs: Long = 1_790_900_000_000L
    }
}

private fun ShellCatalogMode.toScenario(): PreviewCatalogScenario = when (this) {
    ShellCatalogMode.Ready -> PreviewCatalogScenario.Ready
    ShellCatalogMode.Loading -> PreviewCatalogScenario.Loading
    ShellCatalogMode.Empty -> PreviewCatalogScenario.Empty
    ShellCatalogMode.Error -> PreviewCatalogScenario.Error
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
