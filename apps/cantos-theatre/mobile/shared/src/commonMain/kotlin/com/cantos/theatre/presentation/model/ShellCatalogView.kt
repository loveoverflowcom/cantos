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

/** Read-only screen metadata, mapped by the composition root from the catalog boundary. */
data class ShellEpisode(
    val id: ShellEpisodeId,
    val workId: ShellWorkId,
    val index: Int,
    val title: String,
    val durationSeconds: Int,
)

data class ShellWork(
    val id: ShellWorkId,
    val title: String,
    val category: String,
    val description: String,
    val episodes: List<ShellEpisode>,
)

/** Owns snapshots of supplied metadata. Missing selections remain nullable presentation facts. */
class ShellCatalogView(works: List<ShellWork> = emptyList()) {
    private val snapshot: List<ShellWork> = works.map { work -> work.copy(episodes = work.episodes.toList()) }

    val works: List<ShellWork> get() = snapshot.map { work -> detached(work) }

    fun work(id: ShellWorkId): ShellWork? = snapshot.firstOrNull { it.id == id }?.let { detached(it) }

    fun episode(id: ShellEpisodeId): ShellEpisode? = snapshot
        .flatMap { it.episodes }
        .firstOrNull { it.id == id }

    override fun equals(other: Any?): Boolean = other is ShellCatalogView && snapshot == other.snapshot

    override fun hashCode(): Int = snapshot.hashCode()

    private fun detached(work: ShellWork): ShellWork = work.copy(episodes = work.episodes.toList())
}
