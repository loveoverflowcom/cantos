package com.cantos.theatre.app

import com.cantos.theatre.adapters.mock.toEpisodeKey
import com.cantos.theatre.adapters.mock.toWorkKey
import com.cantos.theatre.demo.DemoEvent
import com.cantos.theatre.demo.DemoLibraryTab
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.demo.DemoTimer
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
    val libraryTab: DemoLibraryTab,
    val timer: DemoTimer,
    val membership: MembershipState,
)

/**
 * The shared route dispatcher used by the app and the desktop workbench. Closing this binding
 * fences captured callbacks; the composition root alone owns [container] and its lifetime.
 */
class DemoAppSession(
    val container: AppContainer,
    val seed: DemoState = DemoState(),
    membership: MembershipState = MembershipState.unavailable(),
    private val resetSelected: (() -> Unit)? = null,
    private val onEvent: (DemoEvent) -> Unit = {},
) {
    private val mutableUi = MutableStateFlow(DemoUiInputs(seed.query, seed.libraryTab, seed.timer, membership))
    val ui: StateFlow<DemoUiInputs> = mutableUi.asStateFlow()
    private var closed = false

    fun snapshot(): DemoState = mutableUi.value.let {
        projectDemoState(container.navigation.value, container.catalog.state.value,
            container.listening.state.value, container.playback.state.value, container.preferences.state.value,
            it.query, it.libraryTab, it.timer, seed)
    }

    fun membership(event: MembershipEvent) {
        if (!closed) mutableUi.update { it.copy(membership = it.membership.reduce(event).state) }
    }

    fun dispatch(event: DemoEvent) {
        if (closed) return
        onEvent(event)
        when (event) {
            is DemoEvent.Navigate -> {
                if (container.navigation.value.route == TheatreRoute.MembershipPreview || event.route == DemoRoute.MembershipPreview) {
                    membership(MembershipEvent.OpenIntro)
                }
                container.navigate(NavigationEvent.Navigate(event.route.toTheatreRoute()))
            }
            DemoEvent.Back -> {
                val current = mutableUi.value.membership
                if (container.navigation.value.route == TheatreRoute.MembershipPreview &&
                    current.screen != MembershipScreen.Intro && current.screen != MembershipScreen.Account) {
                    membership(MembershipEvent.Back)
                } else {
                    if (container.navigation.value.route == TheatreRoute.MembershipPreview) membership(MembershipEvent.OpenIntro)
                    container.navigate(NavigationEvent.Back)
                }
            }
            DemoEvent.Reset -> if (resetSelected != null) resetSelected.invoke() else {
                mutableUi.update { it.copy(query = "", libraryTab = DemoLibraryTab.Continuing,
                    timer = DemoTimer.Off, membership = it.membership.reduce(MembershipEvent.ClearSession).state) }
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
            is DemoEvent.OpenBookmark -> container.selectBookmark(event.bookmark.episodeId.toEpisodeKey(), event.bookmark.positionMs)
            DemoEvent.ToggleSimulation -> container.playback.toggleIntent()
            is DemoEvent.Seek -> container.playback.dispatch(PlaybackEvent.SkipRequested(
                (event.deltaSeconds.toLong() * 1000L).coerceIn(Int.MIN_VALUE.toLong(), Int.MAX_VALUE.toLong()).toInt()))
            is DemoEvent.SeekTo -> container.playback.dispatch(PlaybackEvent.SeekRequested(event.seconds.toLong() * 1000L))
            is DemoEvent.SaveWork -> container.listening.dispatch(ListeningEvent.ToggleSaved(event.id.toWorkKey()))
            DemoEvent.Bookmark -> container.bookmark()
            is DemoEvent.SelectLibrary -> mutableUi.update { it.copy(libraryTab = event.tab) }
            is DemoEvent.Search -> mutableUi.update { it.copy(query = event.query) }
            is DemoEvent.CatalogMode -> container.setCatalogScenario(event.mode)
            DemoEvent.Retry -> container.retryCatalog()
            is DemoEvent.SetTheme -> container.preferences.setTheme(
                if (event.theme == DemoTheme.Dark) PreferenceTheme.Dark else PreferenceTheme.Light)
            is DemoEvent.SetLocale -> container.preferences.setLocale(
                if (event.locale == DemoLocale.English) PreferenceLocale.English else PreferenceLocale.Vietnamese)
            is DemoEvent.SetSpeed -> container.playback.dispatch(PlaybackEvent.SpeedRequested(event.percent * 10))
            is DemoEvent.SetTimer -> mutableUi.update { it.copy(timer = event.timer) }
        }
    }

    /** Invalidates a UI binding without closing an externally injected owner. */
    fun close() { closed = true }
}
