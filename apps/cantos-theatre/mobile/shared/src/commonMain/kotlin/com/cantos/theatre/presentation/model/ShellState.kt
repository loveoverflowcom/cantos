package com.cantos.theatre.presentation.model

sealed interface ShellRoute {
    data object Discovery : ShellRoute
    data object Search : ShellRoute
    data object Library : ShellRoute
    data object Settings : ShellRoute
    data object Account : ShellRoute
    data object MembershipPreview : ShellRoute
    /** Optional account intents. Their origin is the route below them in the stack. */
    data object Login : ShellRoute
    data object Register : ShellRoute
    data object History : ShellRoute
    data object Queue : ShellRoute
    data object Player : ShellRoute
    data object Unavailable : ShellRoute
    data class Work(val id: ShellWorkId) : ShellRoute
}

enum class ShellLibraryTab { Continuing, Saved, Bookmarks }

/** The paged Library sections. They read, fail and retry independently. */
enum class ShellLibrarySection { Continue, Saved, History }

enum class ShellCatalogMode { Ready, Loading, Empty, Error }

enum class ShellTheme { Light, Dark }

enum class ShellLocale { Vietnamese, English }

enum class ShellTimer { Off, Fifteen, Thirty, EndOfEpisode }

enum class ShellListeningFailure { MemoryStoreUnavailable, ReleaseUnavailable }

enum class PlaybackUiStatus { Idle, Loading, Paused, Simulating, Playing, Buffering, Ended, Failed }

/** A pinned demo position. It carries no account, publication, or synchronization claim. */
data class ShellBookmark(
    val episodeId: ShellEpisodeId,
    val positionSeconds: Int,
    val positionMs: Long = positionSeconds.toLong() * 1000L,
) {
    init {
        require(positionSeconds >= 0) {
            "Presentation bookmark position must be nonnegative"
        }
        require(positionMs >= 0 && positionSeconds.toLong() == positionMs / 1000L) {
            "Presentation bookmark display must match its exact position"
        }
    }
}

/** What a stored Library row can still do. Samples and fixtures are [Available]. */
enum class ShellResume { Available, NeedsRefresh, ReleaseReplaced, Unavailable }

/** Titles the API confirmed for the exact stored release of one row; never a neighbouring release's. */
data class ShellRowTitles(val workTitle: String, val unitTitle: String)

/**
 * A position-only projection. Progress and accepted-play history have distinct owners.
 * [titles] are present only when the API confirmed them for this exact release; [resume] says
 * whether this exact release can still be resumed.
 */
data class ShellProgress(
    val episodeId: ShellEpisodeId,
    val positionSeconds: Int,
    val positionMs: Long = positionSeconds.toLong() * 1000L,
    val resume: ShellResume = ShellResume.Available,
    val titles: ShellRowTitles? = null,
    /** Duration of the exact stored release; null in samples, which read it from the episode. */
    val durationSeconds: Int? = null,
) {
    init {
        require(positionSeconds >= 0 && positionMs >= 0 && positionSeconds.toLong() == positionMs / 1000L) {
            "Presentation listening display must match its exact nonnegative position"
        }
    }
}

/** Immutable read-only presentation projection; application owners decide transitions.
 * The simulating constructor argument preserves diagnostic seeds only. Runtime projections
 * supply playbackStatus and transportRequested independently of engine observation.
 */
class ShellState(
    stack: List<ShellRoute> = listOf(ShellRoute.Discovery),
    val selectedEpisode: ShellEpisodeId,
    val positionSeconds: Int = 0,
    simulating: Boolean = false,
    savedWorks: Set<ShellWorkId> = emptySet(),
    bookmarks: List<ShellBookmark> = emptyList(),
    val libraryTab: ShellLibraryTab = ShellLibraryTab.Continuing,
    val query: String = "",
    val catalogMode: ShellCatalogMode = ShellCatalogMode.Ready,
    val theme: ShellTheme = ShellTheme.Light,
    val locale: ShellLocale = ShellLocale.Vietnamese,
    val speedPercent: Int = 100,
    val timer: ShellTimer = ShellTimer.Off,
    val playbackStatus: PlaybackUiStatus = if (simulating) PlaybackUiStatus.Simulating else PlaybackUiStatus.Idle,
    val transportRequested: Boolean = simulating,
    val positionMs: Long = positionSeconds.toLong() * 1000L,
    progress: List<ShellProgress> = emptyList(),
    history: List<ShellProgress> = emptyList(),
    val listeningFailure: ShellListeningFailure? = null,
) {
    private val stackSnapshot: List<ShellRoute> = stack.toList()
    private val savedWorksSnapshot: Set<ShellWorkId> = savedWorks.toSet()
    private val bookmarksSnapshot: List<ShellBookmark> = bookmarks.toList()
    private val progressSnapshot = progress.toList()
    private val historySnapshot = history.toList()

    val stack: List<ShellRoute> get() = stackSnapshot.toList()
    val savedWorks: Set<ShellWorkId> get() = savedWorksSnapshot.toSet()
    val bookmarks: List<ShellBookmark> get() = bookmarksSnapshot.toList()
    val progress: List<ShellProgress> get() = progressSnapshot.toList()
    val history: List<ShellProgress> get() = historySnapshot.toList()

    init {
        require(stackSnapshot.isNotEmpty()) { "Presentation navigation must have a root" }
        require(positionSeconds >= 0) {
            "Presentation projection position must be nonnegative"
        }
        require(positionMs >= 0 && positionSeconds.toLong() == positionMs / 1000L) {
            "Presentation projection display must match its exact position"
        }
        require(speedPercent in SupportedUiSpeedPercents) { "Unsupported demo speed" }
    }

    val route: ShellRoute get() = stackSnapshot.last()

    val simulating: Boolean get() = playbackStatus == PlaybackUiStatus.Simulating

    fun copy(
        stack: List<ShellRoute> = this.stack,
        selectedEpisode: ShellEpisodeId = this.selectedEpisode,
        positionSeconds: Int = this.positionSeconds,
        savedWorks: Set<ShellWorkId> = this.savedWorks,
        bookmarks: List<ShellBookmark> = this.bookmarks,
        libraryTab: ShellLibraryTab = this.libraryTab,
        query: String = this.query,
        catalogMode: ShellCatalogMode = this.catalogMode,
        theme: ShellTheme = this.theme,
        locale: ShellLocale = this.locale,
        speedPercent: Int = this.speedPercent,
        timer: ShellTimer = this.timer,
        playbackStatus: PlaybackUiStatus = this.playbackStatus,
        transportRequested: Boolean = this.transportRequested,
        positionMs: Long = if (positionSeconds == this.positionSeconds) this.positionMs else positionSeconds.toLong() * 1000L,
        progress: List<ShellProgress> = this.progress,
        history: List<ShellProgress> = this.history,
        listeningFailure: ShellListeningFailure? = this.listeningFailure,
    ): ShellState = ShellState(
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

    override fun equals(other: Any?): Boolean = other is ShellState &&
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

    override fun toString(): String = "ShellState(" +
        "stack=$stack, selectedEpisode=$selectedEpisode, positionSeconds=$positionSeconds, positionMs=$positionMs, " +
        "savedWorks=$savedWorks, bookmarks=$bookmarks, progress=$progress, history=$history, listeningFailure=$listeningFailure, " +
        "libraryTab=$libraryTab, query=$query, catalogMode=$catalogMode, theme=$theme, " +
        "locale=$locale, speedPercent=$speedPercent, timer=$timer, " +
        "playbackStatus=$playbackStatus, transportRequested=$transportRequested)"
}

sealed interface ShellEvent {
    data class Navigate(val route: ShellRoute) : ShellEvent
    data object Back : ShellEvent
    data object Reset : ShellEvent
    data class OpenEpisode(val id: ShellEpisodeId) : ShellEvent
    data class ResumeEpisode(val id: ShellEpisodeId) : ShellEvent
    data class ResumeHistoryEpisode(val id: ShellEpisodeId) : ShellEvent
    data class SetSaved(val id: ShellWorkId, val saved: Boolean) : ShellEvent
    data object ClearHistory : ShellEvent
    data object RetryListening : ShellEvent
    data class OpenBookmark(val bookmark: ShellBookmark) : ShellEvent
    data object ToggleSimulation : ShellEvent
    data class Seek(val deltaSeconds: Int) : ShellEvent
    data class SeekTo(val seconds: Int) : ShellEvent
    data class SaveWork(val id: ShellWorkId) : ShellEvent
    data object Bookmark : ShellEvent
    data class SelectLibrary(val tab: ShellLibraryTab) : ShellEvent
    data class Search(val query: String) : ShellEvent
    data class CatalogMode(val mode: ShellCatalogMode) : ShellEvent
    data object Retry : ShellEvent
    /** Asks the source for the next page of whatever list the current route shows. */
    data object LoadMore : ShellEvent
    /** Asks the source to re-read the current list from its start. */
    data object Refresh : ShellEvent
    data class SetTheme(val theme: ShellTheme) : ShellEvent
    data class SetLocale(val locale: ShellLocale) : ShellEvent
    data class SetSpeed(val percent: Int) : ShellEvent
    data class SetTimer(val timer: ShellTimer) : ShellEvent
    /** Reads the next page of one Library section only; the other sections are untouched. */
    data class LoadMoreLibrary(val section: ShellLibrarySection) : ShellEvent
    /** Repeats the failed read of one Library section: its next page, or its first page. */
    data class RetryLibrary(val section: ShellLibrarySection) : ShellEvent
}

fun canGoBack(state: ShellState): Boolean = state.stack.size > 1

fun ShellRoute.isAuth(): Boolean = this == ShellRoute.Login || this == ShellRoute.Register

fun filteredWorks(state: ShellState, catalog: ShellCatalogView): List<ShellWork> {
    if (state.catalogMode != ShellCatalogMode.Ready) return emptyList()
    val query = state.query.trim().lowercase()
    if (query.isEmpty() || catalog.searchedBySource) return catalog.works
    return catalog.works.filter { work ->
        work.title.lowercase().contains(query) ||
            work.category.lowercase().contains(query) ||
            work.description.lowercase().contains(query) ||
            work.episodes.any { it.title.lowercase().contains(query) }
    }
}

/** Core orders progress oldest first. Missing metadata never falls back to an older episode. */
fun continuingProgress(state: ShellState): List<ShellProgress> {
    val seen = mutableSetOf<ShellWorkId>()
    return state.progress.asReversed().filter { point ->
        seen.add(point.episodeId.workId)
    }
}
