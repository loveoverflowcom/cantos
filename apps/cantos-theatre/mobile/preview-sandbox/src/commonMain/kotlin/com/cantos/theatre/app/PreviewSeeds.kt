package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.PreviewEpisodeIds
import com.cantos.theatre.adapters.preview.PreviewWorkIds
import com.cantos.theatre.presentation.model.PlaybackUiStatus
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.ShellWorkId

/** The episode id shared projections use while nothing is selected. */
val PreviewNoSelection = ShellEpisodeId("none", ShellWorkId("none"))

/**
 * Seed vocabulary for the preview owners. A [ShellState] here is only a description of where a
 * scenario starts (route, theme, saved works, positions); the owners decide every later transition.
 */
fun previewState(
    stack: List<ShellRoute> = listOf(ShellRoute.Discovery),
    selectedEpisode: ShellEpisodeId = PreviewNoSelection,
    positionMs: Long = 0,
    savedWorks: Set<ShellWorkId> = emptySet(),
    progress: List<ShellProgress> = emptyList(),
    history: List<ShellProgress> = emptyList(),
    libraryTab: ShellLibraryTab = ShellLibraryTab.Continuing,
    query: String = "",
    catalogMode: ShellCatalogMode = ShellCatalogMode.Ready,
    theme: ShellTheme = ShellTheme.Light,
    locale: ShellLocale = ShellLocale.Vietnamese,
    speedPercent: Int = 100,
    timer: ShellTimer = ShellTimer.Off,
    transportRequested: Boolean = false,
    playbackStatus: PlaybackUiStatus = if (transportRequested) PlaybackUiStatus.Playing else PlaybackUiStatus.Idle,
): ShellState = ShellState(
    stack = stack, selectedEpisode = selectedEpisode, positionSeconds = (positionMs / 1000L).toInt(), positionMs = positionMs,
    savedWorks = savedWorks, libraryTab = libraryTab, query = query, catalogMode = catalogMode, theme = theme, locale = locale,
    speedPercent = speedPercent, timer = timer,
    playbackStatus = playbackStatus,
    transportRequested = transportRequested, progress = progress, history = history,
)

fun previewProgress(episode: ShellEpisodeId, positionSeconds: Int) =
    ShellProgress(episode, positionSeconds, positionSeconds * 1000L)

/** Named starting points of the Desktop Preview; none is durable persistence or an account. */
enum class PreviewSession {
    /** A guest on Discover: the first screen a new listener sees. */
    Guest,
    /** A guest on the Library tab: the account explanation and sign-in entry. */
    Fresh,
    /** A signed-in listener with saved works, continue entries and history already synced. */
    Restored,
}

fun PreviewSession.seed(): PreviewSeed = when (this) {
    PreviewSession.Guest -> PreviewSeed(previewState())
    PreviewSession.Fresh -> PreviewSeed(previewState(stack = listOf(ShellRoute.Library)))
    PreviewSession.Restored -> {
        val points = listOf(
            previewProgress(PreviewEpisodeIds.DenKhua01, 91),
            previewProgress(PreviewEpisodeIds.SonHa03, 768),
        )
        PreviewSeed(
            previewState(
                stack = listOf(ShellRoute.Library),
                savedWorks = setOf(PreviewWorkIds.SonHa, PreviewWorkIds.DenKhua),
                progress = points,
                history = points.reversed(),
            ),
            signedIn = true,
        )
    }
}
