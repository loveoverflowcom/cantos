package com.cantos.theatre.adapters.mock

import com.cantos.theatre.demo.DemoBookmark
import com.cantos.theatre.demo.DemoCatalogView
import com.cantos.theatre.demo.DemoEpisode
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoWork
import com.cantos.theatre.demo.DemoWorkId
import com.cantos.theatre.domain.listening.Bookmark
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.listening.SavedWork
import com.cantos.theatre.domain.playback.PlaybackSource
import com.cantos.theatre.domain.publication.CatalogEpisode
import com.cantos.theatre.domain.publication.CatalogInventory
import com.cantos.theatre.domain.publication.CatalogWork
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey

/** These six presentation identifiers belong only to the diagnostic mock, never an HTTP DTO. */
fun DemoWorkId.toWorkKey(): WorkKey = WorkKey(when (this) {
    DemoWorkId.SonHa -> "son-ha-ky"
    DemoWorkId.DenKhua -> "den-khuya"
    DemoWorkId.CanhBuom -> "mong-ho-diep"
})

fun WorkKey.toDemoWorkId(): DemoWorkId? = when (value) {
    "son-ha-ky" -> DemoWorkId.SonHa
    "den-khuya" -> DemoWorkId.DenKhua
    "mong-ho-diep" -> DemoWorkId.CanhBuom
    else -> null
}

fun DemoEpisodeId.toEpisodeKey(): EpisodeKey = when (this) {
    DemoEpisodeId.SonHa01 -> EpisodeKey(DemoWorkId.SonHa.toWorkKey(), 0)
    DemoEpisodeId.SonHa02 -> EpisodeKey(DemoWorkId.SonHa.toWorkKey(), 1)
    DemoEpisodeId.SonHa03 -> EpisodeKey(DemoWorkId.SonHa.toWorkKey(), 2)
    DemoEpisodeId.DenKhua01 -> EpisodeKey(DemoWorkId.DenKhua.toWorkKey(), 0)
    DemoEpisodeId.DenKhua02 -> EpisodeKey(DemoWorkId.DenKhua.toWorkKey(), 1)
    DemoEpisodeId.CanhBuom01 -> EpisodeKey(DemoWorkId.CanhBuom.toWorkKey(), 0)
}

fun EpisodeKey.toDemoEpisodeId(): DemoEpisodeId? = DemoEpisodeId.entries
    .firstOrNull { it.toEpisodeKey() == this }

fun mockInventory(): CatalogInventory = CatalogInventory(DemoFixtures.works.map { work ->
    CatalogWork(
        key = work.id.toWorkKey(), title = work.title, category = work.category,
        description = work.description,
        episodes = work.episodes.map { episode ->
            CatalogEpisode(episode.id.toEpisodeKey(), episode.title, episode.durationSeconds.toLong() * 1000L)
        },
    )
})

fun CatalogEpisode.toPlaybackSource(): PlaybackSource? =
    PlaybackSource.of(key.work.value, key.index, durationMs)

fun CatalogInventory.toDemoView(): DemoCatalogView = DemoCatalogView(works.mapNotNull { work ->
    val id = work.key.toDemoWorkId() ?: return@mapNotNull null
    val episodes = work.episodes.mapNotNull { episode ->
        val episodeId = episode.key.toDemoEpisodeId() ?: return@mapNotNull null
        DemoEpisode(episodeId, id, episode.key.index + 1, episode.title, (episode.durationMs / 1000L).toInt())
    }
    DemoWork(id, work.title, work.category, work.description, episodes)
})

fun seedListening(
    inventory: CatalogInventory,
    selected: EpisodeKey,
    positionMs: Long,
    saved: Set<DemoWorkId>,
    bookmarks: List<DemoBookmark>,
): ListeningSnapshot {
    val progress = inventory.episode(selected)?.let { episode ->
        listOf(ProgressRecord(selected, positionMs.coerceIn(0L, episode.durationMs), episode.durationMs))
    } ?: emptyList()
    return ListeningSnapshot(
        savedWorks = saved.map { SavedWork(it.toWorkKey()) }.toSet(),
        bookmarks = bookmarks.mapNotNull { bookmark ->
            val key = bookmark.episodeId.toEpisodeKey()
            inventory.episode(key)?.let { episode ->
                Bookmark(key, bookmark.positionMs.coerceIn(0L, episode.durationMs), episode.durationMs)
            }
        }.distinctBy { it.episode to it.positionMs },
        progress = progress,
    )
}
