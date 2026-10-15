package com.cantos.theatre.adapters.mock.sandbox.sessions

import com.cantos.theatre.demo.demoState
import com.cantos.theatre.demo.DemoWorkIds
import com.cantos.theatre.demo.DemoEpisodeIds
import com.cantos.theatre.presentation.model.ShellBookmark
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellWorkId

/**
 * Static diagnostic presets over the mock storage database; none is durable persistence.
 * [Demo] is the default seed of `AppContainer.mock` used by every diagnostic host.
 */
enum class MockStorageSession { Demo, Fresh, Restored }

/** Demo is the diagnostic default; Fresh holds no listening record; Restored holds every kind. */
fun MockStorageSession.seed(): ShellState = when (this) {
    MockStorageSession.Demo -> demoState()
    MockStorageSession.Fresh -> demoState(
        stack = listOf(ShellRoute.Library),
        selectedEpisode = DemoEpisodeIds.SonHa01,
        positionSeconds = 0,
        savedWorks = emptySet(),
        progress = emptyList(),
    )
    MockStorageSession.Restored -> {
        val points = listOf(ShellProgress(DemoEpisodeIds.DenKhua01, 91), ShellProgress(DemoEpisodeIds.SonHa03, 768))
        demoState(
            stack = listOf(ShellRoute.Library),
            selectedEpisode = DemoEpisodeIds.SonHa03,
            positionSeconds = 768,
            savedWorks = setOf(DemoWorkIds.SonHa, DemoWorkIds.DenKhua),
            bookmarks = listOf(ShellBookmark(DemoEpisodeIds.SonHa03, 600)),
            progress = points,
            history = points.reversed(),
        )
    }
}
