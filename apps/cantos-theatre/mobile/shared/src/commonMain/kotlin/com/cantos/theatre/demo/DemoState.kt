package com.cantos.theatre.demo

sealed interface DemoRoute {
    data object Discovery : DemoRoute
    data object Search : DemoRoute
    data object Library : DemoRoute
    data object Settings : DemoRoute
    data object Queue : DemoRoute
    data object Player : DemoRoute
    data class Work(val id: DemoWorkId) : DemoRoute
}

enum class DemoLibraryTab { Continuing, Saved, Bookmarks }

enum class DemoCatalogMode { Ready, Loading, Empty, Error }

enum class DemoTheme { Light, Dark }

enum class DemoLocale { Vietnamese, English }

enum class DemoTimer { Off, Fifteen, Thirty, EndOfEpisode }

/** A pinned demo position. It carries no account, publication, or synchronization claim. */
data class DemoBookmark(
    val episodeId: DemoEpisodeId,
    val positionSeconds: Int,
) {
    init {
        require(positionSeconds in 0..DemoFixtures.episode(episodeId).durationSeconds) {
            "Demo bookmark position must be within its episode"
        }
    }
}

/** Memory-only presentation state, separate from future listener contracts and audio adapters. */
class DemoState(
    stack: List<DemoRoute> = listOf(DemoRoute.Discovery),
    val selectedEpisode: DemoEpisodeId = DemoFixtures.initialEpisodeId,
    val positionSeconds: Int = DemoFixtures.initialPositionSeconds,
    val simulating: Boolean = false,
    savedWorks: Set<DemoWorkId> = setOf(DemoWorkId.SonHa),
    bookmarks: List<DemoBookmark> = emptyList(),
    val libraryTab: DemoLibraryTab = DemoLibraryTab.Continuing,
    val query: String = "",
    val catalogMode: DemoCatalogMode = DemoCatalogMode.Ready,
    val theme: DemoTheme = DemoTheme.Light,
    val locale: DemoLocale = DemoLocale.Vietnamese,
    val speedPercent: Int = 100,
    val timer: DemoTimer = DemoTimer.Off,
) {
    val stack: List<DemoRoute> = stack.toList()
    val savedWorks: Set<DemoWorkId> = savedWorks.toSet()
    val bookmarks: List<DemoBookmark> = bookmarks.toList()

    init {
        require(this.stack.isNotEmpty()) { "Demo navigation must have a root" }
        require(positionSeconds in 0..DemoFixtures.episode(selectedEpisode).durationSeconds) {
            "Demo position must be within the selected episode"
        }
        require(speedPercent in DemoFixtures.speedPercents) { "Unsupported demo speed" }
    }

    val route: DemoRoute get() = stack.last()

    fun copy(
        stack: List<DemoRoute> = this.stack,
        selectedEpisode: DemoEpisodeId = this.selectedEpisode,
        positionSeconds: Int = this.positionSeconds,
        simulating: Boolean = this.simulating,
        savedWorks: Set<DemoWorkId> = this.savedWorks,
        bookmarks: List<DemoBookmark> = this.bookmarks,
        libraryTab: DemoLibraryTab = this.libraryTab,
        query: String = this.query,
        catalogMode: DemoCatalogMode = this.catalogMode,
        theme: DemoTheme = this.theme,
        locale: DemoLocale = this.locale,
        speedPercent: Int = this.speedPercent,
        timer: DemoTimer = this.timer,
    ): DemoState = DemoState(
        stack = stack,
        selectedEpisode = selectedEpisode,
        positionSeconds = positionSeconds,
        simulating = simulating,
        savedWorks = savedWorks,
        bookmarks = bookmarks,
        libraryTab = libraryTab,
        query = query,
        catalogMode = catalogMode,
        theme = theme,
        locale = locale,
        speedPercent = speedPercent,
        timer = timer,
    )

    override fun equals(other: Any?): Boolean = other is DemoState &&
        stack == other.stack &&
        selectedEpisode == other.selectedEpisode &&
        positionSeconds == other.positionSeconds &&
        simulating == other.simulating &&
        savedWorks == other.savedWorks &&
        bookmarks == other.bookmarks &&
        libraryTab == other.libraryTab &&
        query == other.query &&
        catalogMode == other.catalogMode &&
        theme == other.theme &&
        locale == other.locale &&
        speedPercent == other.speedPercent &&
        timer == other.timer

    override fun hashCode(): Int {
        var result = stack.hashCode()
        result = 31 * result + selectedEpisode.hashCode()
        result = 31 * result + positionSeconds
        result = 31 * result + simulating.hashCode()
        result = 31 * result + savedWorks.hashCode()
        result = 31 * result + bookmarks.hashCode()
        result = 31 * result + libraryTab.hashCode()
        result = 31 * result + query.hashCode()
        result = 31 * result + catalogMode.hashCode()
        result = 31 * result + theme.hashCode()
        result = 31 * result + locale.hashCode()
        result = 31 * result + speedPercent
        result = 31 * result + timer.hashCode()
        return result
    }

    override fun toString(): String = "DemoState(" +
        "stack=$stack, selectedEpisode=$selectedEpisode, positionSeconds=$positionSeconds, " +
        "simulating=$simulating, savedWorks=$savedWorks, bookmarks=$bookmarks, " +
        "libraryTab=$libraryTab, query=$query, catalogMode=$catalogMode, theme=$theme, " +
        "locale=$locale, speedPercent=$speedPercent, timer=$timer)"
}

sealed interface DemoEvent {
    data class Navigate(val route: DemoRoute) : DemoEvent
    data object Back : DemoEvent
    data object Reset : DemoEvent
    data class OpenEpisode(val id: DemoEpisodeId) : DemoEvent
    data object ToggleSimulation : DemoEvent
    data class Seek(val deltaSeconds: Int) : DemoEvent
    data class SeekTo(val seconds: Int) : DemoEvent
    data class SaveWork(val id: DemoWorkId) : DemoEvent
    data object Bookmark : DemoEvent
    data class SelectLibrary(val tab: DemoLibraryTab) : DemoEvent
    data class Search(val query: String) : DemoEvent
    data class CatalogMode(val mode: DemoCatalogMode) : DemoEvent
    data object Retry : DemoEvent
    data class SetTheme(val theme: DemoTheme) : DemoEvent
    data class SetLocale(val locale: DemoLocale) : DemoEvent
    data class SetSpeed(val percent: Int) : DemoEvent
    data class SetTimer(val timer: DemoTimer) : DemoEvent
}

fun canGoBack(state: DemoState): Boolean = state.stack.size > 1

fun filteredWorks(state: DemoState): List<DemoWork> {
    if (state.catalogMode != DemoCatalogMode.Ready) return emptyList()
    val query = state.query.trim().lowercase()
    if (query.isEmpty()) return DemoFixtures.works
    return DemoFixtures.works.filter { work ->
        work.title.lowercase().contains(query) ||
            work.category.lowercase().contains(query) ||
            work.description.lowercase().contains(query) ||
            work.episodes.any { it.title.lowercase().contains(query) }
    }
}

/** A deterministic reducer: no clock, network, storage, timer scheduling, or media effects. */
fun reduceDemo(state: DemoState, event: DemoEvent): DemoState {
    return when (event) {
        is DemoEvent.Navigate -> navigate(state, event.route)
        DemoEvent.Back -> if (canGoBack(state)) state.copy(stack = state.stack.dropLast(1)) else state
        DemoEvent.Reset -> DemoState()
        is DemoEvent.OpenEpisode -> openEpisode(state, event.id)
        DemoEvent.ToggleSimulation -> state.copy(simulating = !state.simulating)
        is DemoEvent.Seek -> seekTo(state, state.positionSeconds.toLong() + event.deltaSeconds.toLong())
        is DemoEvent.SeekTo -> seekTo(state, event.seconds.toLong())
        is DemoEvent.SaveWork -> state.copy(
            savedWorks = if (event.id in state.savedWorks) {
                state.savedWorks - event.id
            } else {
                state.savedWorks + event.id
            },
        )
        DemoEvent.Bookmark -> addBookmark(state)
        is DemoEvent.SelectLibrary -> state.copy(libraryTab = event.tab)
        is DemoEvent.Search -> state.copy(query = event.query)
        is DemoEvent.CatalogMode -> state.copy(catalogMode = event.mode)
        DemoEvent.Retry -> state.copy(catalogMode = DemoCatalogMode.Ready)
        is DemoEvent.SetTheme -> state.copy(theme = event.theme)
        is DemoEvent.SetLocale -> state.copy(locale = event.locale)
        is DemoEvent.SetSpeed -> if (event.percent in DemoFixtures.speedPercents) {
            state.copy(speedPercent = event.percent)
        } else {
            state
        }
        is DemoEvent.SetTimer -> state.copy(timer = event.timer)
    }
}

private fun navigate(state: DemoState, route: DemoRoute): DemoState = when (route) {
    DemoRoute.Discovery, DemoRoute.Search, DemoRoute.Library -> state.copy(
        stack = listOf(route),
    )
    DemoRoute.Settings, DemoRoute.Queue, DemoRoute.Player, is DemoRoute.Work -> {
        val existingIndex = state.stack.indexOf(route)
        val nextStack = if (existingIndex >= 0) {
            state.stack.take(existingIndex + 1)
        } else {
            state.stack + route
        }
        state.copy(stack = nextStack)
    }
}

private fun openEpisode(state: DemoState, id: DemoEpisodeId): DemoState {
    val position = if (id == state.selectedEpisode) state.positionSeconds else 0
    val stack = if (state.route == DemoRoute.Queue && canGoBack(state)) {
        state.stack.dropLast(1)
    } else {
        state.stack
    }
    val selected = state.copy(
        stack = stack,
        selectedEpisode = id,
        positionSeconds = position,
        simulating = true,
    )
    return navigate(selected, DemoRoute.Player)
}

private fun seekTo(state: DemoState, seconds: Long): DemoState {
    val duration = DemoFixtures.episode(state.selectedEpisode).durationSeconds.toLong()
    return state.copy(positionSeconds = seconds.coerceIn(0L, duration).toInt())
}

private fun addBookmark(state: DemoState): DemoState {
    val bookmark = DemoBookmark(state.selectedEpisode, state.positionSeconds)
    return if (bookmark in state.bookmarks) {
        state
    } else {
        state.copy(bookmarks = state.bookmarks + bookmark)
    }
}
