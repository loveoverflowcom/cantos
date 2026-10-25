package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellBookmark
import com.cantos.theatre.presentation.model.ShellBookmarkSync
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellResume
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.clockTime
import com.cantos.ui.tokenDp

/**
 * The listener's bookmarks, newest first, in the player and in the Library. Each row names the exact release it was
 * made on with that release's own titles; a replaced or unavailable release says so and is never swapped for a newer
 * one. Pending, offline and failed rows are marked as such, and only Synced is silent.
 */
@Composable
internal fun BookmarkRows(state: ShellState, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit, tag: String) {
    state.bookmarks.forEach { bookmark ->
        key(bookmark.id.ifEmpty { "${bookmark.episodeId}:${bookmark.positionMs}" }) { BookmarkRow(bookmark, catalog, copy, dispatch, tag) }
    }
}

@Composable
private fun BookmarkRow(bookmark: ShellBookmark, catalog: ShellCatalogView, copy: ShellCopy, dispatch: (ShellEvent) -> Unit, tag: String) {
    val episode = catalog.episode(bookmark.episodeId)
    val title = bookmark.titles?.unitTitle ?: episode?.title ?: return
    val rowTag = "$tag-${bookmark.id.ifEmpty { bookmark.positionMs.toString() }}"
    val reason = when (bookmark.resume) {
        ShellResume.Available -> null
        ShellResume.NeedsRefresh -> CopyKey.ResumeNeedsRefresh
        ShellResume.ReleaseReplaced, ShellResume.Unavailable -> CopyKey.ReleaseUnavailable
    }
    val status = when (bookmark.sync) {
        ShellBookmarkSync.Synced -> null
        ShellBookmarkSync.Pending -> CopyKey.SyncPending
        ShellBookmarkSync.Offline -> CopyKey.BookmarkOffline
        ShellBookmarkSync.Failed -> CopyKey.BookmarkFailed
        ShellBookmarkSync.DeleteFailed -> CopyKey.BookmarkDeleteFailed
    }
    Column(Modifier.fillMaxWidth().testTag(rowTag), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(ShellEvent.OpenBookmark(bookmark)) },
                modifier = Modifier.weight(1f).heightIn(min = 48.dp).testTag("$rowTag-open").semantics {
                    contentDescription = copy.text(CopyKey.OpenBookmark, "title" to title, "time" to copy.spokenTime(bookmark.positionSeconds))
                }) {
                Text(copy.text(CopyKey.BookmarkCaption, "title" to title, "time" to clockTime(bookmark.positionSeconds)))
            }
            if (bookmark.id.isNotEmpty()) TextButton(shape = MaterialTheme.shapes.small, onClick = { dispatch(ShellEvent.DeleteBookmark(bookmark.id)) },
                modifier = Modifier.heightIn(min = 48.dp).testTag("$rowTag-delete").semantics {
                    // The spoken name is the visible one: a bookmark the server never accepted is discarded, not deleted.
                    contentDescription = copy.text(
                        when (bookmark.sync) {
                            ShellBookmarkSync.Failed -> CopyKey.DiscardBookmarkName
                            ShellBookmarkSync.DeleteFailed -> CopyKey.RetryDeleteBookmarkName
                            else -> CopyKey.DeleteBookmarkName
                        },
                        "title" to title, "time" to copy.spokenTime(bookmark.positionSeconds),
                    )
                }) {
                Text(copy.text(when (bookmark.sync) {
                    ShellBookmarkSync.Failed -> CopyKey.DiscardBookmark
                    ShellBookmarkSync.DeleteFailed -> CopyKey.RetryDeleteBookmark
                    else -> CopyKey.DeleteBookmark
                }))
            }
        }
        if (reason != null) Text(copy.text(reason), style = MaterialTheme.typography.bodySmall, modifier = Modifier.testTag("$rowTag-reason"))
        if (status != null) Text(copy.text(status), style = MaterialTheme.typography.bodySmall,
            color = if (bookmark.sync == ShellBookmarkSync.Failed || bookmark.sync == ShellBookmarkSync.DeleteFailed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.testTag("$rowTag-status").semantics { liveRegion = LiveRegionMode.Polite })
    }
}
