package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.AlertDialog
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
import com.cantos.theatre.presentation.model.LibraryEmptyKind
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryPhase
import com.cantos.theatre.presentation.model.ShellLibrarySection
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellResume
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.clockTime
import com.cantos.theatre.presentation.model.continuingProgress
import com.cantos.theatre.presentation.model.emptyLibrarySections
import com.cantos.theatre.presentation.model.isLibraryCompletelyEmpty
import com.cantos.ui.CantosShapes
import com.cantos.ui.rememberCantosIconVector
import com.cantos.ui.tokenDp

@Composable
internal fun LibraryScreen(state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    Text(copy.text(CopyKey.Library), style = MaterialTheme.typography.headlineSmall,
        modifier = Modifier.semantics { heading() })
    Text(copy.text(CopyKey.LibrarySession), style = MaterialTheme.typography.bodySmall)
    LocalShellLibraryHeader.current?.invoke()
    val capabilities = LocalShellCapabilities.current
    val empty = emptyLibrarySections(state, catalog.library, capabilities)
    val allEmpty = isLibraryCompletelyEmpty(state, catalog.library, capabilities)
    val discoverSection = if (allEmpty) null else listOf(ShellLibrarySection.Continue, ShellLibrarySection.Saved,
        ShellLibrarySection.Bookmarks, ShellLibrarySection.History).firstOrNull { it in empty }
    if (allEmpty) LibraryEmpty(LibraryEmptyKind.Library, copy, dispatch, Modifier.testTag("library-empty-hero"), discover = true)
    Column(Modifier.testTag("library-continue"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        if (!allEmpty) SectionHeading(CopyKey.Continuing, copy)
        val points = continuingProgress(state)
        val phase = catalog.library.continuing
        if (ShellLibrarySection.Continue in empty) LibraryEmpty(
            LibraryEmptyKind.Continue, copy, dispatch, Modifier.testTag("library-continue-empty"),
            discover = discoverSection == ShellLibrarySection.Continue, compact = allEmpty,
        )
        points.forEach { ListeningPoint(it, catalog, copy, dispatch, fromHistory = false, tag = "resume-${it.episodeId}") }
        LibraryFooter(phase, ShellLibrarySection.Continue, "library-continue", copy, dispatch)
    }
    Column(Modifier.testTag("library-saved"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        if (!allEmpty) SectionHeading(CopyKey.SavedTab, copy)
        // The API's order (newest save first), not the browsing catalog's.
        val works = state.savedWorks.mapNotNull { catalog.work(it) }
        val phase = catalog.library.saved
        if (ShellLibrarySection.Saved in empty) LibraryEmpty(
            LibraryEmptyKind.Saved, copy, dispatch, Modifier.testTag("library-saved-empty"),
            discover = discoverSection == ShellLibrarySection.Saved, compact = allEmpty,
        )
        else {
            val (bare, described) = works.partition { it.unavailable }
            if (described.isNotEmpty()) WorkShelf(described, state, copy, dispatch)
            bare.forEach { UndescribedSaved(it, state, copy, dispatch) }
        }
        if (works.size < state.savedWorks.size) Text(copy.text(CopyKey.SelectionUnavailableNote))
        LibraryFooter(phase, ShellLibrarySection.Saved, "library-saved", copy, dispatch)
    }
    Column(Modifier.testTag("library-bookmarks"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        if (!allEmpty) SectionHeading(CopyKey.Bookmarks, copy)
        val phase = catalog.library.bookmarks
        if (!capabilities.bookmarks) {
            if (allEmpty) SectionHeading(CopyKey.Bookmarks, copy)
            Text(copy.text(CopyKey.BookmarksUnavailable), Modifier.testTag("library-bookmarks-unavailable"))
        }
        else {
            if (ShellLibrarySection.Bookmarks in empty) LibraryEmpty(LibraryEmptyKind.Bookmarks, copy, dispatch,
                Modifier.testTag("library-bookmarks-empty"), discover = discoverSection == ShellLibrarySection.Bookmarks, compact = allEmpty)
            BookmarkRows(state, catalog, copy, dispatch, "library-bookmark")
            LibraryFooter(phase, ShellLibrarySection.Bookmarks, "library-bookmarks", copy, dispatch)
        }
    }
    Column(Modifier.testTag("library-history"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        if (!allEmpty) SectionHeading(CopyKey.History, copy)
        val phase = catalog.library.history
        if (ShellLibrarySection.History !in empty) TextButton(shape = MaterialTheme.shapes.small,
            onClick = { dispatch(ShellEvent.Navigate(ShellRoute.History)) }, modifier = Modifier.testTag("history-all")) {
            Text(copy.text(CopyKey.ViewAllHistory))
        }
        if (ShellLibrarySection.History in empty) LibraryEmpty(LibraryEmptyKind.History, copy, dispatch,
            Modifier.testTag("library-history-empty"), discover = discoverSection == ShellLibrarySection.History, compact = allEmpty)
        state.history.take(3).forEach {
            ListeningPoint(it, catalog, copy, dispatch, fromHistory = true, tag = "recent-${it.episodeId}")
        }
        // Only the full History list pages; here a read in flight or a failure is still shown honestly.
        if (phase == ShellLibraryPhase.Loading || phase == ShellLibraryPhase.Failed || phase == ShellLibraryPhase.Stale) {
            LibraryFooter(phase, ShellLibrarySection.History, "library-history", copy, dispatch)
        }
    }
}

/** A saved Work the API gave no released metadata for: kept, named neutrally, with the reason and its own Unsave. */
@Composable
private fun UndescribedSaved(work: ShellWork, state: ShellState, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    Surface(modifier = Modifier.fillMaxWidth().testTag("saved-undescribed-${work.id}"),
        color = MaterialTheme.colorScheme.surfaceContainer, shape = MaterialTheme.shapes.large) {
        Column(Modifier.padding(tokenDp("space.inset")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Text(work.title, style = MaterialTheme.typography.titleMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(copy.text(CopyKey.SavedUndescribed), style = MaterialTheme.typography.bodySmall)
            SaveWorkButton(work, state, copy, dispatch, "save-card-${work.id}")
        }
    }
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
    // Row titles and duration belong to the exact stored release; samples read them from the episode.
    val unitTitle = point.titles?.unitTitle ?: episode?.title
    val workTitle = point.titles?.workTitle ?: catalog.work(point.episodeId.workId)?.title.orEmpty()
    val durationSeconds = point.durationSeconds ?: episode?.durationSeconds ?: 0
    if (unitTitle == null || durationSeconds <= 0) {
        Text(copy.text(CopyKey.SelectionUnavailableNote))
        return
    }
    val reason = when (point.resume) {
        ShellResume.Available -> null
        ShellResume.NeedsRefresh -> CopyKey.ResumeNeedsRefresh
        ShellResume.ReleaseReplaced, ShellResume.Unavailable -> CopyKey.ReleaseUnavailable
    }
    Surface(
        onClick = {
            dispatch(if (fromHistory) ShellEvent.ResumeHistoryEpisode(point.episodeId) else ShellEvent.ResumeEpisode(point.episodeId))
        },
        modifier = Modifier.fillMaxWidth().testTag(tag).semantics {
            role = Role.Button
            contentDescription = copy.text(CopyKey.ResumeSampleName, "title" to unitTitle, "time" to copy.spokenTime(point.positionSeconds))
        },
        color = if (reason == null) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceVariant,
        shape = MaterialTheme.shapes.large,
    ) {
        Row(Modifier.padding(tokenDp("space.inset")), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.inset"))) {
            WorkArtwork(point.episodeId.workId, Modifier.size(tokenDp("size.control")), shape = CantosShapes.thumbnail)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                Text(if (episode != null && point.titles == null) episodeLabel(episode, workTitle, copy) else workTitle, style = MaterialTheme.typography.bodySmall,
                    maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(unitTitle, style = MaterialTheme.typography.labelLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(copy.text(CopyKey.PositionCaption, "elapsed" to clockTime(point.positionSeconds),
                    "total" to clockTime(durationSeconds)), style = MaterialTheme.typography.bodySmall)
                if (reason != null) Text(copy.text(reason), style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.testTag("$tag-reason"))
            }
            if (reason == null) Icon(rememberCantosIconVector("play_arrow"), null)
        }
    }
}

@Composable
internal fun HistoryScreen(state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    var confirming by remember { mutableStateOf(false) }
    SectionHeading(CopyKey.History, copy)
    Text(copy.text(CopyKey.HistoryHelp))
    val phase = catalog.library.history
    if (state.history.isEmpty()) {
        if (phase == ShellLibraryPhase.Complete) LibraryEmpty(LibraryEmptyKind.History, copy, dispatch, Modifier.testTag("history-empty"), discover = true)
    }
    else {
        TextButton(shape = MaterialTheme.shapes.small, onClick = { confirming = true }, modifier = Modifier.testTag("history-clear")) { Text(copy.text(CopyKey.ClearHistory)) }
        state.history.forEach { ListeningPoint(it, catalog, copy, dispatch, fromHistory = true, tag = "history-${it.episodeId}") }
    }
    LibraryFooter(phase, ShellLibrarySection.History, "history", copy, dispatch)
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
