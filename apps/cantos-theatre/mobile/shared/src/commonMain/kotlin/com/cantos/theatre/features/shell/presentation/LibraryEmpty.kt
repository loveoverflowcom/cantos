package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.LibraryEmptyKind
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.ui.CantosShapes
import com.cantos.ui.LocalEditorialFontFamily
import com.cantos.ui.rememberCantosIconVector
import com.cantos.ui.tokenDp

/** Library-owned composition shared by its sections and full History; no effects or read authority. */
@Composable
internal fun LibraryEmpty(
    kind: LibraryEmptyKind,
    copy: ShellCopy,
    dispatch: (ShellEvent) -> Unit,
    modifier: Modifier = Modifier,
    discover: Boolean = false,
    compact: Boolean = false,
) {
    val hero = kind == LibraryEmptyKind.Library
    val foreground = if (hero) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurface
    // Keep the reading measure when either the preview preference or the real OS enlarges text.
    val stackSymbol = LocalDensity.current.fontScale > 1.3f
    Surface(
        modifier.fillMaxWidth(),
        color = if (hero) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.background,
        contentColor = foreground,
        shape = MaterialTheme.shapes.extraLarge,
    ) {
        Column(
            Modifier.padding(if (hero) tokenDp("space.section") else 0.dp),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.inset")),
        ) {
            if (stackSymbol) {
                LibraryEmptySymbol(kind)
                LibraryEmptyText(kind, copy, compact)
            } else Row(
                horizontalArrangement = Arrangement.spacedBy(tokenDp("space.inset")),
                verticalAlignment = Alignment.Top,
            ) {
                LibraryEmptySymbol(kind)
                LibraryEmptyText(kind, copy, compact, Modifier.weight(1f))
            }
            if (discover) Button(
                onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Discovery)) },
                modifier = Modifier.heightIn(min = 48.dp).testTag("library-empty-discover"),
                shape = CantosShapes.full,
            ) {
                Text(copy.text(CopyKey.Discover))
                Icon(rememberCantosIconVector("arrow_forward"), null, Modifier.padding(start = tokenDp("space.sm")))
            }
        }
    }
}

@Composable
private fun LibraryEmptySymbol(kind: LibraryEmptyKind) {
    Surface(
        color = MaterialTheme.colorScheme.secondaryContainer,
        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
        shape = CantosShapes.full,
    ) {
        // Adjacent localized text carries the meaning; the symbol is decorative.
        Icon(rememberCantosIconVector(kind.icon), null,
            Modifier.padding(tokenDp("space.inset")).size(tokenDp("size.icon")))
    }
}

@Composable
private fun LibraryEmptyText(kind: LibraryEmptyKind, copy: ShellCopy, compact: Boolean, modifier: Modifier = Modifier) {
    val hero = kind == LibraryEmptyKind.Library
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        Text(
            copy.text(if (compact) kind.label else kind.title),
            style = if (hero) MaterialTheme.typography.headlineSmall.copy(fontFamily = LocalEditorialFontFamily.current)
                else MaterialTheme.typography.titleMedium,
            modifier = Modifier.semantics { heading() },
        )
        Text(
            copy.text(if (compact) kind.summary else kind.help),
            style = if (compact) MaterialTheme.typography.bodySmall else MaterialTheme.typography.bodyMedium,
            color = if (hero) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
