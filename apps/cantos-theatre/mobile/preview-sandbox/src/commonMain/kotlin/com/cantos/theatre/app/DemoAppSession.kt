package com.cantos.theatre.app

import com.cantos.theatre.demo.demoState
import com.cantos.theatre.adapters.mock.toEpisodeKey
import com.cantos.theatre.adapters.mock.toWorkKey
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.domain.listening.ListeningEvent
import com.cantos.theatre.domain.membership.MembershipEvent
import com.cantos.theatre.domain.membership.MembershipScreen
import com.cantos.theatre.domain.membership.MembershipState
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.application.preferences.PreferenceTheme
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/** Route UI facts have one authority; no service, scope or database is created here. */
data class DemoUiInputs(
    val query: String,
    val libraryTab: ShellLibraryTab,
    val timer: ShellTimer,
    val membership: MembershipState,
)

/**
 * The shared route dispatcher used by the app and the desktop workbench. Closing this binding
 * fences captured callbacks; the composition root alone owns [container] and its lifetime.
 */
class DemoAppSession(
    val container: AppContainer,
    val seed: ShellState = demoState(),
    membership: MembershipState = MembershipState.unavailable(),
    private val resetSelected: (() -> Unit)? = null,
    private val onEvent: (ShellEvent) -> Unit = {},
) {
    private val mutableUi = MutableStateFlow(DemoUiInputs(seed.query, seed.libraryTab, seed.timer, membership))
    val ui: StateFlow<DemoUiInputs> = mutableUi.asStateFlow()
    private var closed = false

    fun snapshot(): ShellState = mutableUi.value.let {
        projectDemoState(container.navigation.value, container.catalog.state.value,
            container.listening.state.value, container.playback.state.value, container.preferences.state.value,
            it.query, it.libraryTab, it.timer, seed)
    }

    fun membership(event: MembershipEvent) {
        if (!closed) mutableUi.update { it.copy(membership = it.membership.reduce(event).state) }
    }

    fun dispatch(event: ShellEvent) {
        if (closed) return
        onEvent(event)
        when (event) {
            is ShellEvent.Navigate -> when (event.route) {
                // Auth opens only through the container's guarded intents, never as a bare route.
                ShellRoute.Login -> container.openLogin()
                ShellRoute.Register -> container.openRegister()
                else -> {
                    if (container.navigation.value.route == TheatreRoute.MembershipPreview || event.route == ShellRoute.MembershipPreview) {
                        membership(MembershipEvent.OpenIntro)
                    }
                    container.navigate(NavigationEvent.Navigate(event.route.toTheatreRoute()))
                }
            }
            ShellEvent.Back -> {
                val current = mutableUi.value.membership
                if (container.navigation.value.route == TheatreRoute.MembershipPreview &&
                    current.screen != MembershipScreen.Intro && current.screen != MembershipScreen.Account) {
                    membership(MembershipEvent.Back)
                } else {
                    if (container.navigation.value.route == TheatreRoute.MembershipPreview) membership(MembershipEvent.OpenIntro)
                    container.back()
                }
            }
            ShellEvent.Reset -> if (resetSelected != null) resetSelected.invoke() else {
                mutableUi.update { it.copy(query = "", libraryTab = ShellLibraryTab.Continuing,
                    timer = ShellTimer.Off, membership = it.membership.reduce(MembershipEvent.ClearSession).state) }
                container.reset()
            }
            is ShellEvent.ResumeEpisode -> if (!container.resumeEpisode(event.id.toEpisodeKey())) {
                container.navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
            }
            is ShellEvent.ResumeHistoryEpisode -> if (!container.resumeHistoryEpisode(event.id.toEpisodeKey())) {
                container.navigate(NavigationEvent.Navigate(TheatreRoute.Unavailable))
            }
            is ShellEvent.SetSaved -> container.setSaved(event.id.toWorkKey(), event.saved)
            ShellEvent.ClearHistory -> container.clearHistory()
            ShellEvent.RetryListening -> container.listening.retry()
            is ShellEvent.OpenEpisode -> container.selectEpisode(event.id.toEpisodeKey())
            is ShellEvent.OpenBookmark -> container.selectBookmark(event.bookmark.episodeId.toEpisodeKey(), event.bookmark.positionMs)
            ShellEvent.ToggleSimulation -> container.playback.toggleIntent()
            is ShellEvent.Seek -> container.playback.dispatch(PlaybackEvent.SkipRequested(
                (event.deltaSeconds.toLong() * 1000L).coerceIn(Int.MIN_VALUE.toLong(), Int.MAX_VALUE.toLong()).toInt()))
            is ShellEvent.SeekTo -> container.playback.dispatch(PlaybackEvent.SeekRequested(event.seconds.toLong() * 1000L))
            is ShellEvent.SaveWork -> container.listening.dispatch(ListeningEvent.ToggleSaved(event.id.toWorkKey()))
            ShellEvent.Bookmark -> container.bookmark()
            is ShellEvent.SelectLibrary -> mutableUi.update { it.copy(libraryTab = event.tab) }
            is ShellEvent.Search -> mutableUi.update { it.copy(query = event.query) }
            is ShellEvent.CatalogMode -> container.setCatalogStatus(when (event.mode) {
                ShellCatalogMode.Ready -> com.cantos.theatre.application.catalog.CatalogStatus.Ready
                ShellCatalogMode.Loading -> com.cantos.theatre.application.catalog.CatalogStatus.Loading
                ShellCatalogMode.Empty -> com.cantos.theatre.application.catalog.CatalogStatus.Empty
                ShellCatalogMode.Error -> com.cantos.theatre.application.catalog.CatalogStatus.Unavailable
            })
            ShellEvent.Retry -> container.retryCatalog()
            // The fixed sample inventory has one complete page: nothing to extend or re-read.
            ShellEvent.LoadMore, ShellEvent.Refresh -> Unit
            is ShellEvent.SetTheme -> container.preferences.setTheme(
                if (event.theme == ShellTheme.Dark) PreferenceTheme.Dark else PreferenceTheme.Light)
            is ShellEvent.SetLocale -> container.preferences.setLocale(
                if (event.locale == ShellLocale.English) PreferenceLocale.English else PreferenceLocale.Vietnamese)
            is ShellEvent.SetSpeed -> container.playback.dispatch(PlaybackEvent.SpeedRequested(event.percent * 10))
            is ShellEvent.SetTimer -> mutableUi.update { it.copy(timer = event.timer) }
        }
    }

    /** Invalidates a UI binding without closing an externally injected owner. */
    fun close() { closed = true }
}
