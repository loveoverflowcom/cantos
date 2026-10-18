package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellBookmark
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEpisode
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.ui.CantosShapes
import com.cantos.ui.tokenDp
import com.cantos.ui.rememberCantosIconVector

@Composable
internal fun PlayerControls(state: ShellState, episode: ShellEpisode, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    var panel by remember { mutableStateOf<PreferencePanel?>(null) }
    var restoreFocus by remember { mutableStateOf<PreferencePanel?>(null) }
    val speedFocus = remember { FocusRequester() }
    val timerFocus = remember { FocusRequester() }
    LaunchedEffect(panel, restoreFocus) {
        if (panel == null && restoreFocus != null) {
            withFrameNanos { }
            when (restoreFocus) {
                PreferencePanel.Speed -> speedFocus.requestFocus()
                PreferencePanel.Timer -> timerFocus.requestFocus()
                PreferencePanel.Theme, PreferencePanel.Language, PreferencePanel.Diagnostics, null -> Unit
            }
            restoreFocus = null
        }
    }
    Column(horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        // Icon geometry is independent of text scaling; all three targets keep their order.
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                FilledTonalIconButton(shape = CantosShapes.full, onClick = { dispatch(ShellEvent.Seek(-15)) },
                    modifier = Modifier.size(tokenDp("size.control")).testTag("rewind")) {
                    Icon(rememberCantosIconVector("fast_rewind"), copy.text(CopyKey.Rewind))
                }
                Text(copy.text(CopyKey.SeekStep), style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.clearAndSetSemantics { })
            }
            FilledIconButton(shape = CantosShapes.full, onClick = { dispatch(ShellEvent.ToggleSimulation) },
                modifier = Modifier.size(tokenDp("size.control") + tokenDp("space.xl"))
                    .testTag("transport").semantics {
                        contentDescription = copy.text(if (state.transportRequested) CopyKey.PauseEpisode else CopyKey.PlayEpisode,
                            "title" to episode.title)
                    }) {
                Icon(if (state.transportRequested) rememberCantosIconVector("pause") else rememberCantosIconVector("play_arrow"), null,
                    modifier = Modifier.size(tokenDp("size.icon") + tokenDp("space.sm")))
            }
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                FilledTonalIconButton(shape = CantosShapes.full, onClick = { dispatch(ShellEvent.Seek(15)) },
                    modifier = Modifier.size(tokenDp("size.control")).testTag("forward")) {
                    Icon(rememberCantosIconVector("fast_forward"), copy.text(CopyKey.Forward))
                }
                Text(copy.text(CopyKey.SeekStep), style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.clearAndSetSemantics { })
            }
        }
        Text(copy.text(if (state.transportRequested) CopyKey.Pause else CopyKey.Play),
            style = MaterialTheme.typography.labelLarge)
    }
    FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        FilledTonalButton(shape = MaterialTheme.shapes.medium, onClick = { panel = PreferencePanel.Speed },
            modifier = Modifier.testTag("player-speed").focusRequester(speedFocus).semantics {
                contentDescription = copy.text(CopyKey.SpeedCaption, "speed" to copy.speed(state.speedPercent))
            }) {
            Icon(rememberCantosIconVector("speed"), null)
            Text(copy.speed(state.speedPercent))
        }
        FilledTonalButton(shape = MaterialTheme.shapes.medium, onClick = { panel = PreferencePanel.Timer },
            modifier = Modifier.testTag("player-timer").focusRequester(timerFocus).semantics {
                contentDescription = copy.text(CopyKey.Timer)
                stateDescription = copy.text(timerKey(state.timer))
            }) {
            Icon(rememberCantosIconVector("timer"), null)
            Text(copy.text(timerKey(state.timer)))
        }
        if (LocalShellCapabilities.current.bookmarks) {
            val bookmarked = ShellBookmark(state.selectedEpisode, state.positionSeconds, state.positionMs) in state.bookmarks
            FilledTonalButton(shape = MaterialTheme.shapes.medium, onClick = { dispatch(ShellEvent.Bookmark) }, modifier = Modifier.testTag("bookmark")) {
                Icon(rememberCantosIconVector("star"), null)
                Text(copy.text(if (bookmarked) CopyKey.Bookmarked else CopyKey.Bookmark))
            }
        }
        FilledTonalButton(shape = MaterialTheme.shapes.medium, onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Queue)) }, modifier = Modifier.testTag("queue")) {
            Icon(rememberCantosIconVector("queue_music"), null)
            Text(copy.text(CopyKey.Queue))
        }
    }
    panel?.let { current ->
        PreferenceDialog(current, state, copy, dispatch) { panel = null; restoreFocus = current }
    }
}
