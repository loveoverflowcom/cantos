package com.cantos.theatre.domain.publication

/** Opaque synthetic catalog identity, never a publication authorization. */
data class WorkKey(val value: String) {
    init {
        require(value.isNotBlank()) { "Work key must not be blank" }
    }
}

/** Episode indexes follow the shared mock playback corpus: zero based. */
data class EpisodeKey(val work: WorkKey, val index: Int) {
    init {
        require(index >= 0) { "Episode index must be nonnegative" }
    }
}

enum class CatalogOrigin { SyntheticDemo }

data class CatalogEpisode(
    val key: EpisodeKey,
    val title: String,
    val durationMs: Long,
) {
    init {
        require(title.isNotBlank()) { "Episode title must not be blank" }
        require(durationMs > 0) { "Episode duration must be positive" }
    }

    val origin: CatalogOrigin get() = CatalogOrigin.SyntheticDemo
}

/** A read model for original demo text; it carries no asset, rights or backend claim. */
class CatalogWork(
    val key: WorkKey,
    val title: String,
    val category: String,
    val description: String,
    episodes: List<CatalogEpisode>,
) {
    private val episodeSnapshot = episodes.toList()
    val episodes: List<CatalogEpisode> get() = episodeSnapshot.toList()
    val origin: CatalogOrigin get() = CatalogOrigin.SyntheticDemo

    init {
        require(title.isNotBlank()) { "Work title must not be blank" }
        require(episodeSnapshot.isNotEmpty()) { "Demo work must contain an episode" }
        require(episodeSnapshot.all { it.key.work == key }) { "Episode must belong to its work" }
        require(episodeSnapshot.map { it.key }.distinct().size == episodeSnapshot.size) {
            "Episode keys must be unique within a work"
        }
    }

    override fun equals(other: Any?): Boolean = other is CatalogWork &&
        key == other.key && title == other.title && category == other.category &&
        description == other.description && episodeSnapshot == other.episodeSnapshot

    override fun hashCode(): Int {
        var result = key.hashCode()
        result = 31 * result + title.hashCode()
        result = 31 * result + category.hashCode()
        result = 31 * result + description.hashCode()
        return 31 * result + episodeSnapshot.hashCode()
    }

    override fun toString(): String = "CatalogWork(key=$key, origin=$origin)"
}

/** Closed inventory snapshot: callers can mutate their returned copy, never this value. */
class CatalogInventory(works: List<CatalogWork> = emptyList()) {
    private val workSnapshot = works.toList()
    val works: List<CatalogWork> get() = workSnapshot.toList()

    init {
        require(workSnapshot.map { it.key }.distinct().size == workSnapshot.size) {
            "Catalog work keys must be unique"
        }
    }

    fun work(key: WorkKey): CatalogWork? = workSnapshot.firstOrNull { it.key == key }

    fun episode(key: EpisodeKey): CatalogEpisode? = work(key.work)?.episodes
        ?.firstOrNull { it.key == key }

    override fun equals(other: Any?): Boolean = other is CatalogInventory && workSnapshot == other.workSnapshot
    override fun hashCode(): Int = workSnapshot.hashCode()
    override fun toString(): String = "CatalogInventory(works=${workSnapshot.size}, origin=SyntheticDemo)"
}
