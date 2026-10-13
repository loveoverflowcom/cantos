package com.cantos.theatre.demo

sealed interface DemoRoute {
    data object Discovery : DemoRoute
    data object Search : DemoRoute
    data object Library : DemoRoute
    data object Settings : DemoRoute
    data object Account : DemoRoute
    data object History : DemoRoute
    data object Queue : DemoRoute
    data object Player : DemoRoute
    data object Unavailable : DemoRoute
    data class Work(val id: DemoWorkId) : DemoRoute
}

enum class DemoLibraryTab { Continuing, Saved, Bookmarks }

enum class DemoCatalogMode { Ready, Loading, Empty, Error }

enum class DemoTheme { Light, Dark }

enum class DemoLocale { Vietnamese, English }

enum class DemoTimer { Off, Fifteen, Thirty, EndOfEpisode }

enum class DemoListeningFailure { MemoryStoreUnavailable }

enum class PlaybackUiStatus { Idle, Loading, Paused, Simulating, Buffering, Ended }

/** A pinned demo position. It carries no account, publication, or synchronization claim. */
data class DemoBookmark(
    val episodeId: DemoEpisodeId,
    val positionSeconds: Int,
    val positionMs: Long = positionSeconds.toLong() * 1000L,
) {
    init {
        require(positionSeconds >= 0) {
            "Demo bookmark position must be nonnegative"
        }
        require(positionMs >= 0 && positionSeconds.toLong() == positionMs / 1000L) {
            "Demo bookmark display must match its exact position"
        }
    }
}

/** A position-only projection. Progress and accepted-play history have distinct owners. */
data class DemoProgress(
    val episodeId: DemoEpisodeId,
    val positionSeconds: Int,
    val positionMs: Long = positionSeconds.toLong() * 1000L,
) {
    init {
        require(positionSeconds >= 0 && positionMs >= 0 && positionSeconds.toLong() == positionMs / 1000L) {
            "Demo listening display must match its exact nonnegative position"
        }
    }
}

/** Immutable diagnostic seed and read-only projection; application owners decide transitions.
 * The simulating constructor argument preserves diagnostic seeds only. Runtime projections
 * supply playbackStatus and transportRequested independently of engine observation.
 */
class DemoState(
    stack: List<DemoRoute> = listOf(DemoRoute.Discovery),
    val selectedEpisode: DemoEpisodeId = DemoEpisodeId.SonHa03,
    val positionSeconds: Int = 768,
    simulating: Boolean = false,
    savedWorks: Set<DemoWorkId> = setOf(DemoWorkId.SonHa),
    bookmarks: List<DemoBookmark> = emptyList(),
    val libraryTab: DemoLibraryTab = DemoLibraryTab.Continuing,
    val query: String = "",
    val catalogMode: DemoCatalogMode = DemoCatalogMode.Ready,
    val theme: DemoTheme = DemoTheme.Light,
    val locale: DemoLocale = DemoLocale.Vietnamese,
    val speedPercent: Int = 100,
    val timer: DemoTimer = DemoTimer.Off,
    val playbackStatus: PlaybackUiStatus = if (simulating) PlaybackUiStatus.Simulating else PlaybackUiStatus.Paused,
    val transportRequested: Boolean = simulating,
    val positionMs: Long = positionSeconds.toLong() * 1000L,
    progress: List<DemoProgress> = listOf(DemoProgress(selectedEpisode, positionSeconds, positionMs)),
    history: List<DemoProgress> = emptyList(),
    val listeningFailure: DemoListeningFailure? = null,
) {
    private val stackSnapshot: List<DemoRoute> = stack.toList()
    private val savedWorksSnapshot: Set<DemoWorkId> = savedWorks.toSet()
    private val bookmarksSnapshot: List<DemoBookmark> = bookmarks.toList()
    private val progressSnapshot = progress.toList()
    private val historySnapshot = history.toList()

    val stack: List<DemoRoute> get() = stackSnapshot.toList()
    val savedWorks: Set<DemoWorkId> get() = savedWorksSnapshot.toSet()
    val bookmarks: List<DemoBookmark> get() = bookmarksSnapshot.toList()
    val progress: List<DemoProgress> get() = progressSnapshot.toList()
    val history: List<DemoProgress> get() = historySnapshot.toList()

    init {
        require(stackSnapshot.isNotEmpty()) { "Demo navigation must have a root" }
        require(positionSeconds >= 0) {
            "Demo projection position must be nonnegative"
        }
        require(positionMs >= 0 && positionSeconds.toLong() == positionMs / 1000L) {
            "Demo projection display must match its exact position"
        }
        require(speedPercent in SupportedUiSpeedPercents) { "Unsupported demo speed" }
    }

    val route: DemoRoute get() = stackSnapshot.last()

    val simulating: Boolean get() = playbackStatus == PlaybackUiStatus.Simulating

    fun copy(
        stack: List<DemoRoute> = this.stack,
        selectedEpisode: DemoEpisodeId = this.selectedEpisode,
        positionSeconds: Int = this.positionSeconds,
        savedWorks: Set<DemoWorkId> = this.savedWorks,
        bookmarks: List<DemoBookmark> = this.bookmarks,
        libraryTab: DemoLibraryTab = this.libraryTab,
        query: String = this.query,
        catalogMode: DemoCatalogMode = this.catalogMode,
        theme: DemoTheme = this.theme,
        locale: DemoLocale = this.locale,
        speedPercent: Int = this.speedPercent,
        timer: DemoTimer = this.timer,
        playbackStatus: PlaybackUiStatus = this.playbackStatus,
        transportRequested: Boolean = this.transportRequested,
        positionMs: Long = if (positionSeconds == this.positionSeconds) this.positionMs else positionSeconds.toLong() * 1000L,
        progress: List<DemoProgress> = this.progress,
        history: List<DemoProgress> = this.history,
        listeningFailure: DemoListeningFailure? = this.listeningFailure,
    ): DemoState = DemoState(
        stack = stack,
        selectedEpisode = selectedEpisode,
        positionSeconds = positionSeconds,
        savedWorks = savedWorks,
        bookmarks = bookmarks,
        libraryTab = libraryTab,
        query = query,
        catalogMode = catalogMode,
        theme = theme,
        locale = locale,
        speedPercent = speedPercent,
        timer = timer,
        playbackStatus = playbackStatus,
        transportRequested = transportRequested,
        positionMs = positionMs,
        progress = progress,
        history = history,
        listeningFailure = listeningFailure,
    )

    override fun equals(other: Any?): Boolean = other is DemoState &&
        stackSnapshot == other.stackSnapshot &&
        selectedEpisode == other.selectedEpisode &&
        positionSeconds == other.positionSeconds &&
        positionMs == other.positionMs &&
        savedWorksSnapshot == other.savedWorksSnapshot &&
        bookmarksSnapshot == other.bookmarksSnapshot &&
        progressSnapshot == other.progressSnapshot &&
        historySnapshot == other.historySnapshot &&
        listeningFailure == other.listeningFailure &&
        libraryTab == other.libraryTab &&
        query == other.query &&
        catalogMode == other.catalogMode &&
        theme == other.theme &&
        locale == other.locale &&
        speedPercent == other.speedPercent &&
        timer == other.timer &&
        playbackStatus == other.playbackStatus &&
        transportRequested == other.transportRequested

    override fun hashCode(): Int {
        var result = stackSnapshot.hashCode()
        result = 31 * result + selectedEpisode.hashCode()
        result = 31 * result + positionSeconds
        result = 31 * result + positionMs.hashCode()
        result = 31 * result + savedWorksSnapshot.hashCode()
        result = 31 * result + bookmarksSnapshot.hashCode()
        result = 31 * result + progressSnapshot.hashCode()
        result = 31 * result + historySnapshot.hashCode()
        result = 31 * result + (listeningFailure?.hashCode() ?: 0)
        result = 31 * result + libraryTab.hashCode()
        result = 31 * result + query.hashCode()
        result = 31 * result + catalogMode.hashCode()
        result = 31 * result + theme.hashCode()
        result = 31 * result + locale.hashCode()
        result = 31 * result + speedPercent
        result = 31 * result + timer.hashCode()
        result = 31 * result + playbackStatus.hashCode()
        result = 31 * result + transportRequested.hashCode()
        return result
    }

    override fun toString(): String = "DemoState(" +
        "stack=$stack, selectedEpisode=$selectedEpisode, positionSeconds=$positionSeconds, positionMs=$positionMs, " +
        "savedWorks=$savedWorks, bookmarks=$bookmarks, progress=$progress, history=$history, listeningFailure=$listeningFailure, " +
        "libraryTab=$libraryTab, query=$query, catalogMode=$catalogMode, theme=$theme, " +
        "locale=$locale, speedPercent=$speedPercent, timer=$timer, " +
        "playbackStatus=$playbackStatus, transportRequested=$transportRequested)"
}

sealed interface DemoEvent {
    data class Navigate(val route: DemoRoute) : DemoEvent
    data object Back : DemoEvent
    data object Reset : DemoEvent
    data class OpenEpisode(val id: DemoEpisodeId) : DemoEvent
    data class ResumeEpisode(val id: DemoEpisodeId) : DemoEvent
    data class ResumeHistoryEpisode(val id: DemoEpisodeId) : DemoEvent
    data class SetSaved(val id: DemoWorkId, val saved: Boolean) : DemoEvent
    data object ClearHistory : DemoEvent
    data object RetryListening : DemoEvent
    data class OpenBookmark(val bookmark: DemoBookmark) : DemoEvent
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

fun filteredWorks(state: DemoState, catalog: DemoCatalogView): List<DemoWork> {
    if (state.catalogMode != DemoCatalogMode.Ready) return emptyList()
    val query = state.query.trim().lowercase()
    if (query.isEmpty()) return catalog.works
    return catalog.works.filter { work ->
        work.title.lowercase().contains(query) ||
            work.category.lowercase().contains(query) ||
            work.description.lowercase().contains(query) ||
            work.episodes.any { it.title.lowercase().contains(query) }
    }
}

/** Core orders progress oldest first. Missing metadata never falls back to an older episode. */
fun continuingProgress(state: DemoState): List<DemoProgress> {
    val seen = mutableSetOf<DemoWorkId>()
    return state.progress.asReversed().filter { point ->
        seen.add(point.episodeId.workId)
    }
}
