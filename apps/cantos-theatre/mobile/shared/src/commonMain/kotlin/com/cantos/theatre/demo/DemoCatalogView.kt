package com.cantos.theatre.demo

enum class DemoWorkId { SonHa, DenKhua, CanhBuom }

enum class DemoEpisodeId { SonHa01, SonHa02, SonHa03, DenKhua01, DenKhua02, CanhBuom01 }

val SupportedUiSpeedPercents: List<Int> get() = listOf(75, 100, 125, 150, 200)

/** Read-only screen metadata, mapped by the composition root from the catalog boundary. */
data class DemoEpisode(
    val id: DemoEpisodeId,
    val workId: DemoWorkId,
    val index: Int,
    val title: String,
    val durationSeconds: Int,
)

data class DemoWork(
    val id: DemoWorkId,
    val title: String,
    val category: String,
    val description: String,
    val episodes: List<DemoEpisode>,
)

/** Owns snapshots of supplied metadata. Missing selections remain nullable presentation facts. */
class DemoCatalogView(works: List<DemoWork> = emptyList()) {
    private val snapshot: List<DemoWork> = works.map { work -> work.copy(episodes = work.episodes.toList()) }

    val works: List<DemoWork> get() = snapshot.map { work -> detached(work) }

    fun work(id: DemoWorkId): DemoWork? = snapshot.firstOrNull { it.id == id }?.let { detached(it) }

    fun episode(id: DemoEpisodeId): DemoEpisode? = snapshot
        .flatMap { it.episodes }
        .firstOrNull { it.id == id }

    override fun equals(other: Any?): Boolean = other is DemoCatalogView && snapshot == other.snapshot

    override fun hashCode(): Int = snapshot.hashCode()

    private fun detached(work: DemoWork): DemoWork = work.copy(episodes = work.episodes.toList())
}
