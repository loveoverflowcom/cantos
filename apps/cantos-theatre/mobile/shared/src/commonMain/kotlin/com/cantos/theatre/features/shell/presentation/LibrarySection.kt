package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.PlayArrow
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.clockTime
import com.cantos.theatre.presentation.model.continuingProgress
import com.cantos.ui.CantosShapes
import com.cantos.ui.tokenDp

@Composable
internal fun LibraryScreen(state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    Text(copy.text(CopyKey.Library), style = MaterialTheme.typography.headlineSmall,
        modifier = Modifier.semantics { heading() })
    Text(copy.text(CopyKey.LibrarySession), style = MaterialTheme.typography.bodySmall)
    LocalShellLibraryHeader.current?.invoke()
    Column(Modifier.testTag("library-continue"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        SectionHeading(CopyKey.Continuing, copy)
        val points = continuingProgress(state)
        if (points.isEmpty()) SectionEmpty(CopyKey.NoContinue, CopyKey.NoContinueHelp, copy, dispatch)
        points.forEach { ListeningPoint(it, catalog, copy, dispatch, fromHistory = false, tag = "resume-${it.episodeId}") }
    }
    Column(Modifier.testTag("library-saved"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        SectionHeading(CopyKey.SavedTab, copy)
        val works = catalog.knownWorks.filter { it.id in state.savedWorks }
        if (works.isEmpty()) SectionEmpty(CopyKey.NoSaved, CopyKey.NoSavedHelp, copy, dispatch)
        else WorkShelf(works, state, copy, dispatch)
        if (works.size < state.savedWorks.size) Text(copy.text(CopyKey.SelectionUnavailableNote))
    }
    Column(Modifier.testTag("library-history"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        SectionHeading(CopyKey.History, copy)
        TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(ShellEvent.Navigate(ShellRoute.History)) }, modifier = Modifier.testTag("history-all")) {
            Text(copy.text(CopyKey.ViewAllHistory))
        }
        if (state.history.isEmpty()) SectionEmpty(CopyKey.NoHistory, CopyKey.NoHistoryHelp, copy, dispatch)
        state.history.take(3).forEach {
            ListeningPoint(it, catalog, copy, dispatch, fromHistory = true, tag = "recent-${it.episodeId}")
        }
    }
}

@Composable
private fun SectionEmpty(title: CopyKey, help: CopyKey, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    Text(copy.text(title), style = MaterialTheme.typography.titleMedium)
    Text(copy.text(help), style = MaterialTheme.typography.bodySmall)
    TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Discovery)) }) { Text(copy.text(CopyKey.Discover)) }
}

@Composable
internal fun ListeningPoint(
    point: ShellProgress,
    catalog: ShellCatalogView,
    copy: ShellCopy,
    dispatch: (ShellEvent) -> Unit,
    fromHistory: Boolean,
    tag: String,
) {
    val episode = catalog.episode(point.episodeId)
    if (episode == null || episode.durationSeconds <= 0) {
        Text(copy.text(CopyKey.SelectionUnavailableNote))
        return
    }
    Surface(
        onClick = {
            dispatch(if (fromHistory) ShellEvent.ResumeHistoryEpisode(point.episodeId) else ShellEvent.ResumeEpisode(point.episodeId))
        },
        modifier = Modifier.fillMaxWidth().testTag(tag).semantics {
            role = Role.Button
            contentDescription = copy.text(CopyKey.ResumeSampleName, "title" to episode.title, "time" to copy.spokenTime(point.positionSeconds))
        },
        color = MaterialTheme.colorScheme.primaryContainer,
        shape = MaterialTheme.shapes.large,
    ) {
        Row(Modifier.padding(tokenDp("space.inset")), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.inset"))) {
            WorkArtwork(episode.workId, Modifier.size(tokenDp("size.control")), shape = CantosShapes.thumbnail)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                Text(episodeLabel(episode, catalog.work(episode.workId)?.title.orEmpty(), copy), style = MaterialTheme.typography.bodySmall)
                Text(episode.title, style = MaterialTheme.typography.labelLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(copy.text(CopyKey.PositionCaption, "elapsed" to clockTime(point.positionSeconds),
                    "total" to clockTime(episode.durationSeconds)), style = MaterialTheme.typography.bodySmall)
            }
            Icon(Icons.Outlined.PlayArrow, null)
        }
    }
}

@Composable
internal fun HistoryScreen(state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    var confirming by remember { mutableStateOf(false) }
    SectionHeading(CopyKey.History, copy)
    Text(copy.text(CopyKey.HistoryHelp))
    if (state.history.isEmpty()) SectionEmpty(CopyKey.NoHistory, CopyKey.NoHistoryHelp, copy, dispatch)
    else {
        TextButton(shape = MaterialTheme.shapes.small, onClick = { confirming = true }, modifier = Modifier.testTag("history-clear")) { Text(copy.text(CopyKey.ClearHistory)) }
        state.history.forEach { ListeningPoint(it, catalog, copy, dispatch, fromHistory = true, tag = "history-${it.episodeId}") }
    }
    if (confirming) AlertDialog(
        shape = MaterialTheme.shapes.extraLarge,
        modifier = Modifier.testTag("history-dialog"),
        onDismissRequest = { confirming = false },
        title = { Text(copy.text(CopyKey.ClearHistoryConfirmTitle)) },
        text = { Text(copy.text(CopyKey.ClearHistoryHelp)) },
        confirmButton = {
            TextButton(shape = MaterialTheme.shapes.small, onClick = { confirming = false; dispatch(ShellEvent.ClearHistory) }, modifier = Modifier.testTag("history-confirm")) {
                Text(copy.text(CopyKey.ConfirmClearHistory))
            }
        },
        dismissButton = {
            TextButton(shape = MaterialTheme.shapes.small, onClick = { confirming = false }, modifier = Modifier.testTag("history-cancel")) { Text(copy.text(CopyKey.Cancel)) }
        },
    )
}
