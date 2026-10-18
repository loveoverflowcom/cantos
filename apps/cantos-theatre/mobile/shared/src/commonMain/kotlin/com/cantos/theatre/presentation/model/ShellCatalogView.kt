package com.cantos.theatre.presentation.model

/** Opaque presentation identity; supplied metadata defines its meaning. */
data class ShellWorkId(val value: String) {
    init { require(value.isNotBlank()) }
    override fun toString(): String = value
}

data class ShellEpisodeId(val value: String, val workId: ShellWorkId) {
    init { require(value.isNotBlank()) }
    override fun toString(): String = value
}

val SupportedUiSpeedPercents: List<Int> get() = listOf(75, 100, 125, 150, 200)

/** Chapters belong to novels and stories, episodes to podcasts; sample data is episodic. */
enum class ShellUnitKind { Episode, Chapter }

/** Where a paged list stands. [Complete] is the end of a list and the default for fixed samples. */
enum class ShellListPhase { Complete, Loading, More, Failed }

/** The authoritative work read behind a detail screen. [Ready] is the default for fixed samples. */
enum class ShellDetailPhase { Ready, Loading, Failed, NotFound }

/**
 * Read-only screen metadata, mapped by the composition root from the catalog boundary.
 * [numbered] is false when the source gives only a display order: a gap or duplicate in it is
 * never shown to the listener as an episode number.
 */
data class ShellEpisode(
    val id: ShellEpisodeId,
    val workId: ShellWorkId,
    val index: Int,
    val title: String,
    val durationSeconds: Int,
    val numbered: Boolean = true,
)

/**
 * [unitsPhase] describes the episode list: only [ShellListPhase.Complete] makes
 * [episodes] a total, so a count is shown only then. [detailPhase] describes the authoritative read.
 */
data class ShellWork(
    val id: ShellWorkId,
    val title: String,
    val category: String,
    val description: String,
    val episodes: List<ShellEpisode>,
    val unitKind: ShellUnitKind = ShellUnitKind.Episode,
    val unitsPhase: ShellListPhase = ShellListPhase.Complete,
    val detailPhase: ShellDetailPhase = ShellDetailPhase.Ready,
)

/** What this host can honestly do. Absent capabilities are not offered, never simulated. */
data class ShellCapabilities(
    val bookmarks: Boolean = true,
)

/**
 * Owns snapshots of supplied metadata. Missing selections remain nullable presentation facts.
 * [works] is what a listener browses. [referenced] works and [referencedEpisodes] exist only so a
 * Library row, the mini-player or a Saved shelf can be named; they never join a browsing list.
 */
class ShellCatalogView(
    works: List<ShellWork> = emptyList(),
    /** The state of the browsing list; [ShellListPhase.Complete] for a fixed sample. */
    val worksPhase: ShellListPhase = ShellListPhase.Complete,
    /** True when the source already applied the search text, so it is not filtered again locally. */
    val searchedBySource: Boolean = false,
    referenced: List<ShellWork> = emptyList(),
    referencedEpisodes: List<ShellEpisode> = emptyList(),
) {
    private val snapshot: List<ShellWork> = works.map { work -> work.copy(episodes = work.episodes.toList()) }
    private val referencedSnapshot: List<ShellWork> = referenced.map { work -> work.copy(episodes = work.episodes.toList()) }
    private val referencedEpisodeSnapshot: List<ShellEpisode> = referencedEpisodes.toList()

    val works: List<ShellWork> get() = snapshot.map { work -> detached(work) }

    /** Browsing and referenced works together, browsing first. */
    val knownWorks: List<ShellWork> get() = (snapshot + referencedSnapshot).map { work -> detached(work) }

    fun work(id: ShellWorkId): ShellWork? = (snapshot + referencedSnapshot).firstOrNull { it.id == id }?.let { detached(it) }

    fun episode(id: ShellEpisodeId): ShellEpisode? = (snapshot + referencedSnapshot)
        .flatMap { it.episodes }
        .firstOrNull { it.id == id }
        ?: referencedEpisodeSnapshot.firstOrNull { it.id == id }

    override fun equals(other: Any?): Boolean = other is ShellCatalogView && snapshot == other.snapshot &&
        worksPhase == other.worksPhase && searchedBySource == other.searchedBySource &&
        referencedSnapshot == other.referencedSnapshot && referencedEpisodeSnapshot == other.referencedEpisodeSnapshot

    override fun hashCode(): Int = listOf(snapshot, worksPhase, searchedBySource, referencedSnapshot, referencedEpisodeSnapshot).hashCode()

    private fun detached(work: ShellWork): ShellWork = work.copy(episodes = work.episodes.toList())
}
