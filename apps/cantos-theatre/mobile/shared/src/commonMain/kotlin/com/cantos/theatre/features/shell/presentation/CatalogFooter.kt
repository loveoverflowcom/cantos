package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellListPhase
import com.cantos.ui.tokenDp

/**
 * The end of a paged list that a source reads in pages. A fixed sample is [ShellListPhase.Complete]
 * and draws nothing. Rows already shown are kept above a failure, so it never replaces the list.
 */
@Composable
internal fun ListFooter(phase: ShellListPhase, copy: ShellCopy, dispatch: (ShellEvent) -> Unit, first: Boolean = false) {
    when (phase) {
        ShellListPhase.Complete -> Unit
        ShellListPhase.Loading -> Text(copy.text(if (first) CopyKey.Loading else CopyKey.LoadingMore),
            Modifier.testTag("list-loading").semantics { liveRegion = LiveRegionMode.Polite },
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        ShellListPhase.More -> FilledTonalButton(shape = MaterialTheme.shapes.medium,
            onClick = { dispatch(ShellEvent.LoadMore) }, modifier = Modifier.testTag("list-load-more")) {
            Text(copy.text(CopyKey.LoadMore))
        }
        ShellListPhase.Failed -> Column(Modifier.testTag("list-failed").semantics { liveRegion = LiveRegionMode.Polite },
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Text(copy.text(if (first) CopyKey.Error else CopyKey.NextPageFailed), color = MaterialTheme.colorScheme.error)
            Button(shape = MaterialTheme.shapes.medium, onClick = { dispatch(ShellEvent.Retry) },
                modifier = Modifier.testTag("list-retry")) { Text(copy.text(CopyKey.Retry)) }
        }
    }
}

/** Under a minute the old whole-minute caption would read "0 minutes", so it names seconds. */
internal fun durationCaption(durationSeconds: Int, copy: ShellCopy): String =
    if (durationSeconds in 1..59) {
        copy.text(if (durationSeconds == 1) CopyKey.SecondOne else CopyKey.SecondsOther, "count" to durationSeconds.toString())
    } else copy.text(CopyKey.Duration, "minutes" to (durationSeconds / 60).toString())
