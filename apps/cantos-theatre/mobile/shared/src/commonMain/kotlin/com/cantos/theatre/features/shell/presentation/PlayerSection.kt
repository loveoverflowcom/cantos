package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.LegacyCopyKey
import com.cantos.theatre.presentation.model.PlaybackUiStatus
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEpisode
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryPhase
import com.cantos.theatre.presentation.model.ShellLibrarySection
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.clockTime
import com.cantos.theatre.presentation.model.generated.ShellMessage
import com.cantos.ui.CantosShapes
import com.cantos.ui.rememberCantosIconVector
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp

@Composable
internal fun PlayerScreen(state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    val episode = catalog.episode(state.selectedEpisode)
    val work = episode?.let { catalog.work(it.workId) }
    if (episode == null || work == null || episode.durationSeconds <= 0 || state.positionSeconds > episode.durationSeconds) {
        SelectionUnavailable(copy, dispatch)
        return
    }
    WorkArtwork(work.id, Modifier.fillMaxWidth().widthIn(max = tokenDp("breakpoint.medium")).aspectRatio(1.32f))
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(episodeLabel(episode, work.title, copy), style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.tertiary)
        Text(episode.title, style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.semantics { heading() })
        Text(copy.text(playbackStatusKey(state.playbackStatus)),
            modifier = Modifier.testTag("simulation-status").semantics { liveRegion = LiveRegionMode.Polite })
        Slider(value = state.positionSeconds.toFloat(), onValueChange = { dispatch(ShellEvent.SeekTo(it.toInt())) },
            valueRange = 0f..episode.durationSeconds.toFloat(), modifier = Modifier.fillMaxWidth().testTag("seek").semantics {
                contentDescription = copy.text(CopyKey.Position)
                stateDescription = copy.text(ShellMessage.PositionSpoken(elapsed = copy.spokenTime(state.positionSeconds), total = copy.spokenTime(episode.durationSeconds)))
            })
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(clockTime(state.positionSeconds), modifier = Modifier.testTag("elapsed"))
            Text(clockTime(episode.durationSeconds))
        }
    }
    PlayerControls(state, episode, copy, dispatch)
    Text(copy.text(CopyKey.PlayerExplanation), style = MaterialTheme.typography.bodySmall)
    val bookmarkPhase = catalog.library.bookmarks
    if (state.bookmarks.isNotEmpty() || bookmarkPhase != ShellLibraryPhase.Complete) {
        Column(Modifier.testTag("player-bookmarks"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            SectionHeading(CopyKey.Bookmarks, copy)
            BookmarkRows(state, catalog, copy, dispatch, "player-bookmark")
            LibraryFooter(bookmarkPhase, ShellLibrarySection.Bookmarks, "player-bookmarks", copy, dispatch)
        }
    }
}

@Composable
internal fun QueueScreen(state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    Text(copy.text(CopyKey.Queue), style = MaterialTheme.typography.headlineSmall, modifier = Modifier.semantics { heading() })
    Text(copy.text(CopyKey.NextExplanation))
    val episode = catalog.episode(state.selectedEpisode)
    val work = episode?.let { catalog.work(it.workId) }
    if (work == null || work.episodes.isEmpty()) {
        SelectionUnavailable(copy, dispatch)
        return
    }
    work.episodes.forEach {
        EpisodeSelection(it, state, copy, dispatch)
    }
}

@Composable
internal fun MiniPlayer(state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    val episode = catalog.episode(state.selectedEpisode) ?: return
    Surface(color = tokenColor("color.playerSurface"), contentColor = tokenColor("color.onPlayerSurface"),
        shape = MaterialTheme.shapes.medium, modifier = Modifier.fillMaxWidth()
            .padding(horizontal = tokenDp("space.md"), vertical = tokenDp("space.sm")).testTag("mini-player")) {
        Row(Modifier.padding(tokenDp("space.sm")), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Surface(onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Player)) },
                color = tokenColor("color.playerSurface"), contentColor = tokenColor("color.onPlayerSurface"),
                modifier = Modifier.weight(1f).testTag("mini-open").semantics {
                    role = Role.Button
                    contentDescription = copy.text(ShellMessage.OpenPlayer(title = episode.title))
                    stateDescription = copy.text(playbackStatusKey(state.playbackStatus))
                }) {
                Row(verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                    WorkArtwork(episode.workId, Modifier.size(tokenDp("size.control")), shape = CantosShapes.thumbnail)
                    Column(Modifier.weight(1f)) {
                        Text(episode.title, style = MaterialTheme.typography.labelLarge,
                            maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(clockTime(state.positionSeconds), style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
            FilledTonalIconButton(shape = CantosShapes.full, onClick = { dispatch(ShellEvent.ToggleSimulation) },
                colors = IconButtonDefaults.filledTonalIconButtonColors(
                    containerColor = tokenColor("color.playerAccent"), contentColor = tokenColor("color.playerSurface")),
                modifier = Modifier.size(tokenDp("size.control")).testTag("mini-transport")) {
                Icon(if (state.transportRequested) rememberCantosIconVector("pause") else rememberCantosIconVector("play_arrow"),
                    copy.text(if (state.transportRequested) ShellMessage.PauseEpisode(title = episode.title) else ShellMessage.PlayEpisode(title = episode.title)))
            }
        }
    }
}

private fun playbackStatusKey(status: PlaybackUiStatus): CopyKey = when (status) {
    PlaybackUiStatus.Idle -> CopyKey.PlaybackIdle
    PlaybackUiStatus.Loading -> CopyKey.PlaybackLoading
    PlaybackUiStatus.Paused -> CopyKey.PlaybackPaused
    PlaybackUiStatus.Simulating -> CopyKey.PlaybackSimulating
    PlaybackUiStatus.Playing -> CopyKey.PlaybackPlaying
    PlaybackUiStatus.Failed -> CopyKey.PlaybackFailed
    PlaybackUiStatus.Buffering -> CopyKey.PlaybackBuffering
    PlaybackUiStatus.Ended -> CopyKey.PlaybackEnded
}

/** "Episode 3 · Work" for numbered samples; only the work title when the source has no real number. */
internal fun episodeLabel(episode: ShellEpisode, workTitle: String, copy: ShellCopy): String =
    if (episode.numbered) copy.text(LegacyCopyKey.EpisodeLabel, "index" to episode.index.toString(), "title" to workTitle) else workTitle
