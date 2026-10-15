package com.cantos.theatre.adapters.mock

import com.cantos.theatre.demo.DemoWorkIds
import com.cantos.theatre.demo.DemoEpisodeIds
import com.cantos.theatre.presentation.model.ShellBookmark
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellEpisode
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.domain.listening.Bookmark
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.listening.SavedWork
import com.cantos.theatre.domain.playback.PlaybackSource
import com.cantos.theatre.domain.publication.CatalogOrigin
import com.cantos.theatre.domain.publication.CatalogEpisode
import com.cantos.theatre.domain.publication.CatalogInventory
import com.cantos.theatre.domain.publication.CatalogWork
import com.cantos.theatre.domain.publication.EpisodeKey
import com.cantos.theatre.domain.publication.WorkKey

/** These six presentation identifiers belong only to the diagnostic mock, never an HTTP DTO. */
fun ShellWorkId.toWorkKey(): WorkKey = WorkKey(when (this) {
    DemoWorkIds.SonHa -> "son-ha-ky"
    DemoWorkIds.DenKhua -> "den-khuya"
    DemoWorkIds.CanhBuom -> "mong-ho-diep"
    else -> error("Unknown diagnostic work identity")
})

fun WorkKey.toDemoWorkId(): ShellWorkId? = when (value) {
    "son-ha-ky" -> DemoWorkIds.SonHa
    "den-khuya" -> DemoWorkIds.DenKhua
    "mong-ho-diep" -> DemoWorkIds.CanhBuom
    else -> null
}

fun ShellEpisodeId.toEpisodeKey(): EpisodeKey = when (this) {
    DemoEpisodeIds.SonHa01 -> EpisodeKey(DemoWorkIds.SonHa.toWorkKey(), 0)
    DemoEpisodeIds.SonHa02 -> EpisodeKey(DemoWorkIds.SonHa.toWorkKey(), 1)
    DemoEpisodeIds.SonHa03 -> EpisodeKey(DemoWorkIds.SonHa.toWorkKey(), 2)
    DemoEpisodeIds.DenKhua01 -> EpisodeKey(DemoWorkIds.DenKhua.toWorkKey(), 0)
    DemoEpisodeIds.DenKhua02 -> EpisodeKey(DemoWorkIds.DenKhua.toWorkKey(), 1)
    DemoEpisodeIds.CanhBuom01 -> EpisodeKey(DemoWorkIds.CanhBuom.toWorkKey(), 0)
    else -> error("Unknown diagnostic episode identity")
}

fun EpisodeKey.toDemoEpisodeId(): ShellEpisodeId? = DemoEpisodeIds.entries
    .firstOrNull { it.toEpisodeKey() == this }

fun mockInventory(): CatalogInventory = CatalogInventory(DemoFixtures.works.map { work ->
    CatalogWork(
        key = work.id.toWorkKey(), title = work.title, category = work.category,
        description = work.description, origin = CatalogOrigin.SyntheticDemo,
        episodes = work.episodes.map { episode ->
            CatalogEpisode(episode.id.toEpisodeKey(), episode.title, episode.durationSeconds.toLong() * 1000L, CatalogOrigin.SyntheticDemo)
        },
    )
})

fun CatalogInventory.toDemoView(): ShellCatalogView = ShellCatalogView(works.mapNotNull { work ->
    val id = work.key.toDemoWorkId() ?: return@mapNotNull null
    val episodes = work.episodes.mapNotNull { episode ->
        val episodeId = episode.key.toDemoEpisodeId() ?: return@mapNotNull null
        ShellEpisode(episodeId, id, episode.key.index + 1, episode.title, (episode.durationMs / 1000L).toInt())
    }
    ShellWork(id, work.title, work.category, work.description, episodes)
})

fun seedListening(
    inventory: CatalogInventory,
    selected: EpisodeKey,
    positionMs: Long,
    saved: Set<ShellWorkId>,
    bookmarks: List<ShellBookmark>,
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
