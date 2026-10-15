package com.cantos.theatre.app

import com.cantos.theatre.adapters.mock.CatalogScenario
import com.cantos.theatre.adapters.mock.MockCatalogPort
import com.cantos.theatre.adapters.mock.MockMediaEngine
import com.cantos.theatre.adapters.mock.mockInventory
import com.cantos.theatre.adapters.mock.sandbox.database.MockPersistentProgressPort
import com.cantos.theatre.adapters.mock.sandbox.database.MockStorageConfig
import com.cantos.theatre.adapters.mock.sandbox.database.MockStorageDatabase
import com.cantos.theatre.adapters.mock.sandbox.sessions.MockStorageSession
import com.cantos.theatre.adapters.mock.sandbox.sessions.seed
import com.cantos.theatre.adapters.mock.seedListening
import com.cantos.theatre.adapters.mock.toEpisodeKey
import com.cantos.theatre.domain.publication.toPlaybackSource
import com.cantos.theatre.adapters.mock.toWorkKey
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesState
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.playback.PlaybackSnapshot
import com.cantos.theatre.domain.publication.CatalogInventory
import kotlinx.coroutines.CoroutineScope

/**
 * Every mock mount stores listening data in one mock storage database; [storage]
 * receives the exact listening seed the controller starts with. Reset restores the
 * [MockStorageSession.Demo] preset.
 */
fun mockAppContainer(
    scope: CoroutineScope,
    seed: ShellState = MockStorageSession.Demo.seed(),
    account: NativeAccountController? = null,
    progressConfig: MockStorageConfig = MockStorageConfig(),
    storage: (ListeningSnapshot) -> MockStorageDatabase = ::MockStorageDatabase,
): AppContainer {
    val inventory = mockInventory()
    val start = startState(inventory, seed)
    val defaults = startState(inventory, MockStorageSession.Demo.seed())
    val port = MockCatalogPort(inventory, seed.catalogMode.toScenario())
    val container = AppContainer(
        scope, port, MockPersistentProgressPort(storage(start.listening), progressConfig), MockMediaEngine(), start, defaults,
        changeScenario = { status -> port.setScenario(status.toScenario()) }, account = account,
    )
    if (seed.catalogMode != ShellCatalogMode.Ready) container.catalog.refresh()
    return container
}

private fun startState(inventory: CatalogInventory, seed: ShellState): AppStartState {
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
            if (seed.theme == ShellTheme.Dark) PreferenceTheme.Dark else PreferenceTheme.Light,
            if (seed.locale == ShellLocale.English) PreferenceLocale.English else PreferenceLocale.Vietnamese,
        ),
        wantsSimulation = seed.transportRequested,
    )
}

private fun ShellProgress.toProgressRecord(inventory: CatalogInventory): ProgressRecord? {
    val key = episodeId.toEpisodeKey()
    val episode = inventory.episode(key) ?: return null
    return ProgressRecord(key, positionMs.coerceIn(0L, episode.durationMs), episode.durationMs)
}

fun ShellRoute.toTheatreRoute(): TheatreRoute = when (this) {
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
    is ShellRoute.Work -> TheatreRoute.Work(id.toWorkKey())
}

private fun ShellCatalogMode.toScenario(): CatalogScenario = when (this) {
    ShellCatalogMode.Ready -> CatalogScenario.Ready
    ShellCatalogMode.Loading -> CatalogScenario.Loading
    ShellCatalogMode.Empty -> CatalogScenario.Empty
    ShellCatalogMode.Error -> CatalogScenario.Error
}

private fun com.cantos.theatre.application.catalog.CatalogStatus.toScenario(): CatalogScenario = when (this) {
    com.cantos.theatre.application.catalog.CatalogStatus.Ready -> CatalogScenario.Ready
    com.cantos.theatre.application.catalog.CatalogStatus.Loading -> CatalogScenario.Loading
    com.cantos.theatre.application.catalog.CatalogStatus.Empty -> CatalogScenario.Empty
    com.cantos.theatre.application.catalog.CatalogStatus.Unavailable -> CatalogScenario.Error
}
