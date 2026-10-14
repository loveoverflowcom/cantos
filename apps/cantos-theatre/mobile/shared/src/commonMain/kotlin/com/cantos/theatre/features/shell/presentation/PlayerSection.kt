package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Pause
import androidx.compose.material.icons.outlined.PlayArrow
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
import com.cantos.theatre.app.CopyKey
import com.cantos.theatre.app.DemoCopy
import com.cantos.theatre.app.clockTime
import com.cantos.theatre.demo.DemoCatalogView
import com.cantos.theatre.demo.DemoEvent
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.PlaybackUiStatus
import com.cantos.ui.CantosShapes
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp

@Composable
internal fun PlayerScreen(state: DemoState, catalog: DemoCatalogView, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    val episode = catalog.episode(state.selectedEpisode)
    val work = episode?.let { catalog.work(it.workId) }
    if (episode == null || work == null || episode.durationSeconds <= 0 || state.positionSeconds > episode.durationSeconds) {
        SelectionUnavailable(copy, dispatch)
        return
    }
    WorkArtwork(work.id, Modifier.fillMaxWidth().widthIn(max = tokenDp("breakpoint.medium")).aspectRatio(1.32f))
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(copy.text(CopyKey.EpisodeLabel, "index" to episode.index.toString(), "title" to work.title), style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.tertiary)
        Text(episode.title, style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.semantics { heading() })
        Text(copy.text(playbackStatusKey(state.playbackStatus)),
            modifier = Modifier.testTag("simulation-status").semantics { liveRegion = LiveRegionMode.Polite })
        Slider(value = state.positionSeconds.toFloat(), onValueChange = { dispatch(DemoEvent.SeekTo(it.toInt())) },
            valueRange = 0f..episode.durationSeconds.toFloat(), modifier = Modifier.fillMaxWidth().testTag("seek").semantics {
                contentDescription = copy.text(CopyKey.Position)
                stateDescription = copy.text(CopyKey.PositionSpoken, "elapsed" to copy.spokenTime(state.positionSeconds), "total" to copy.spokenTime(episode.durationSeconds))
            })
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text(clockTime(state.positionSeconds), modifier = Modifier.testTag("elapsed"))
            Text(clockTime(episode.durationSeconds))
        }
    }
    PlayerControls(state, episode, copy, dispatch)
    Text(copy.text(CopyKey.PlayerExplanation), style = MaterialTheme.typography.bodySmall)
    if (state.bookmarks.isNotEmpty()) {
        SectionHeading(CopyKey.Bookmarks, copy)
        state.bookmarks.forEach { bookmark ->
            val markedEpisode = catalog.episode(bookmark.episodeId)
            if (markedEpisode != null) TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(DemoEvent.OpenBookmark(bookmark)) }, modifier = Modifier.semantics {
                contentDescription = copy.text(CopyKey.OpenBookmark, "title" to markedEpisode.title, "time" to copy.spokenTime(bookmark.positionSeconds))
            }) {
                Text(copy.text(CopyKey.BookmarkCaption, "title" to markedEpisode.title, "time" to clockTime(bookmark.positionSeconds)))
            }
        }
    }

}

@Composable
internal fun QueueScreen(state: DemoState, catalog: DemoCatalogView, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
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
internal fun MiniPlayer(state: DemoState, catalog: DemoCatalogView, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    val episode = catalog.episode(state.selectedEpisode) ?: return
    Surface(color = tokenColor("color.playerSurface"), contentColor = tokenColor("color.onPlayerSurface"),
        shape = MaterialTheme.shapes.medium, modifier = Modifier.fillMaxWidth()
            .padding(horizontal = tokenDp("space.md"), vertical = tokenDp("space.sm")).testTag("mini-player")) {
        Row(Modifier.padding(tokenDp("space.sm")), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Surface(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Player)) },
                color = tokenColor("color.playerSurface"), contentColor = tokenColor("color.onPlayerSurface"),
                modifier = Modifier.weight(1f).testTag("mini-open").semantics {
                    role = Role.Button
                    contentDescription = copy.text(CopyKey.OpenPlayer, "title" to episode.title)
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
            FilledTonalIconButton(shape = CantosShapes.full, onClick = { dispatch(DemoEvent.ToggleSimulation) },
                colors = IconButtonDefaults.filledTonalIconButtonColors(
                    containerColor = tokenColor("color.playerAccent"), contentColor = tokenColor("color.playerSurface")),
                modifier = Modifier.size(tokenDp("size.control")).testTag("mini-transport")) {
                Icon(if (state.transportRequested) Icons.Outlined.Pause else Icons.Outlined.PlayArrow,
                    copy.text(if (state.transportRequested) CopyKey.PauseEpisode else CopyKey.PlayEpisode, "title" to episode.title))
            }
        }
    }
}

private fun playbackStatusKey(status: PlaybackUiStatus): CopyKey = when (status) {
    PlaybackUiStatus.Idle -> CopyKey.MockIdle
    PlaybackUiStatus.Loading -> CopyKey.MockLoading
    PlaybackUiStatus.Paused -> CopyKey.MockPaused
    PlaybackUiStatus.Simulating -> CopyKey.MockPlaying
    PlaybackUiStatus.Buffering -> CopyKey.MockBuffering
    PlaybackUiStatus.Ended -> CopyKey.MockEnded
}
