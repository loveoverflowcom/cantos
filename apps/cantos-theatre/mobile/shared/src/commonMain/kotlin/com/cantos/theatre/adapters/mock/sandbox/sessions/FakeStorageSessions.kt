package com.cantos.theatre.adapters.mock.sandbox.sessions

import com.cantos.theatre.demo.DemoBookmark
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoProgress
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoWorkId

/**
 * Static diagnostic presets over the fake storage database; none is durable persistence.
 * [Demo] is the default seed of `AppContainer.mock` used by every diagnostic host.
 */
enum class FakeStorageSession { Demo, Fresh, Restored }

/** Demo is the diagnostic default; Fresh holds no listening record; Restored holds every kind. */
fun FakeStorageSession.seed(): DemoState = when (this) {
    FakeStorageSession.Demo -> DemoState()
    FakeStorageSession.Fresh -> DemoState(
        stack = listOf(DemoRoute.Library),
        selectedEpisode = DemoEpisodeId.SonHa01,
        positionSeconds = 0,
        savedWorks = emptySet(),
        progress = emptyList(),
    )
    FakeStorageSession.Restored -> {
        val points = listOf(DemoProgress(DemoEpisodeId.DenKhua01, 91), DemoProgress(DemoEpisodeId.SonHa03, 768))
        DemoState(
            stack = listOf(DemoRoute.Library),
            selectedEpisode = DemoEpisodeId.SonHa03,
            positionSeconds = 768,
            savedWorks = setOf(DemoWorkId.SonHa, DemoWorkId.DenKhua),
            bookmarks = listOf(DemoBookmark(DemoEpisodeId.SonHa03, 600)),
            progress = points,
            history = points.reversed(),
        )
    }
}
