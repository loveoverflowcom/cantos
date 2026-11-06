package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalLayoutDirection
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
import androidx.compose.ui.unit.LayoutDirection
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
import com.cantos.ui.Announcement
import com.cantos.ui.CantosShapes
import com.cantos.ui.LoadingState
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
        val status = copy.text(playbackStatusKey(state.playbackStatus))
        // Waiting on the engine shows the shared loading state; every other status stays plain text.
        if (state.playbackStatus == PlaybackUiStatus.Loading || state.playbackStatus == PlaybackUiStatus.Buffering) {
            LoadingState(status, Modifier.testTag("simulation-status").semantics(mergeDescendants = true) {}, announcement = Announcement.Polite)
        } else {
            Text(status, modifier = Modifier.testTag("simulation-status").semantics { liveRegion = LiveRegionMode.Polite })
        }
        // A different release gets a fresh input lifetime; position ticks and appearance retain focus.
        key(episode.id, episode.publicationId, episode.durationSeconds) {
            PlayerTimeline(state, episode, copy, dispatch)
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

private data class SeekDraft(val seconds: Float, val generation: Long)

/** An app-owned draft only; the existing controller remains the sole playback/ack authority. */
@Composable
internal fun PlayerTimeline(state: ShellState, episode: ShellEpisode, copy: ShellCopy, dispatch: (ShellEvent) -> Unit,
    interactions: MutableInteractionSource = remember { MutableInteractionSource() },
) {
    var draft by remember { mutableStateOf<SeekDraft?>(null) }
    val currentState by rememberUpdatedState(state)
    val currentDispatch by rememberUpdatedState(dispatch)
    var mounted by remember { mutableStateOf(true) }
    DisposableEffect(Unit) { onDispose { mounted = false } }
    var dragging by remember { mutableStateOf(false) }
    var abandoned by remember { mutableStateOf(false) }
    var hadFocus by remember { mutableStateOf(false) }
    val commit = {
        val target = draft
        draft = null
        if (mounted && !abandoned && target != null && target.generation == currentState.playbackGeneration) {
            currentDispatch(ShellEvent.SeekTo(target.seconds.toInt(), target.generation))
        }
    }
    LaunchedEffect(interactions) {
        interactions.interactions.collect { interaction ->
            when (interaction) {
                is DragInteraction.Start -> { dragging = true; abandoned = false }
                is DragInteraction.Stop -> { commit(); dragging = false; abandoned = false }
                is DragInteraction.Cancel -> { draft = null; dragging = false; abandoned = false }
            }
        }
    }
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val shown = draft?.takeIf { it.generation == state.playbackGeneration }?.seconds
        ?: (state.pendingSeekSeconds ?: state.positionSeconds).toFloat()
    Slider(value = shown, onValueChange = {
        if (!abandoned && (draft == null || draft?.generation == state.playbackGeneration)) {
            draft = SeekDraft(it, state.playbackGeneration)
        }
    },
        // Material invokes this for taps and accessibility SetProgress too. Drag Stop/Cancel owns drag completion.
        onValueChangeFinished = { if (!dragging) commit() }, interactionSource = interactions,
        valueRange = 0f..episode.durationSeconds.toFloat(), modifier = Modifier.fillMaxWidth().testTag("seek")
            .onPreviewKeyEvent { event ->
                val base = draft?.takeIf { it.generation == state.playbackGeneration }?.seconds
                    ?: (state.pendingSeekSeconds ?: state.positionSeconds).toFloat()
                val target = seekKeyTarget(event.key, base, episode.durationSeconds, rtl)
                if (target == null) false else {
                    when (event.type) {
                        KeyEventType.KeyDown -> if (!abandoned && (draft == null || draft?.generation == state.playbackGeneration)) {
                            draft = SeekDraft(target, state.playbackGeneration)
                        }
                        KeyEventType.KeyUp -> commit()
                    }
                    true
                }
            }
            .onFocusChanged {
                if (hadFocus && !it.isFocused) { draft = null; abandoned = dragging }
                hadFocus = it.isFocused
            }.semantics {
                contentDescription = copy.text(CopyKey.Position)
                stateDescription = copy.text(ShellMessage.PositionSpoken(elapsed = copy.spokenTime(shown.toInt()), total = copy.spokenTime(episode.durationSeconds)))
            })
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Text(clockTime(shown.toInt()), modifier = Modifier.testTag("elapsed"))
        Text(clockTime(episode.durationSeconds))
    }
}

/** Match the continuous slider's percent steps, with explicit keyboard completion and RTL direction. */
private fun seekKeyTarget(key: Key, current: Float, duration: Int, rtl: Boolean): Float? {
    val step = (duration / 100f).coerceAtLeast(1f)
    val target = when (key) {
        Key.DirectionRight -> current + if (rtl) -step else step
        Key.DirectionLeft -> current + if (rtl) step else -step
        Key.DirectionUp -> current + step
        Key.DirectionDown -> current - step
        Key.PageUp -> current + step * 10
        Key.PageDown -> current - step * 10
        Key.MoveHome -> 0f
        Key.MoveEnd -> duration.toFloat()
        else -> return null
    }
    return target.coerceIn(0f, duration.toFloat())
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
                        Text(episode.title, style = MaterialTheme.typography.labelLarge, modifier = Modifier.testTag("mini-title"),
                            maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(clockTime(state.positionSeconds), style = MaterialTheme.typography.bodySmall, modifier = Modifier.testTag("mini-time"))
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
