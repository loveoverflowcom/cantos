package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
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
import com.cantos.theatre.presentation.model.ShellTheme
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
    theme: ShellTheme,
    modifier: Modifier = Modifier,
    discover: Boolean = false,
    compact: Boolean = false,
    illustrated: Boolean = false,
) {
    val hero = kind == LibraryEmptyKind.Library
    val foreground = if (hero) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurface
    Surface(
        modifier.fillMaxWidth(),
        color = if (hero) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.background,
        contentColor = foreground,
        shape = MaterialTheme.shapes.extraLarge,
    ) {
        BoxWithConstraints(
            Modifier.padding(if (hero) tokenDp("space.section") else 0.dp),
        ) {
            val wide = hero && maxWidth >= tokenDp("breakpoint.medium") && LocalDensity.current.fontScale <= 1.3f
            if (wide) Row(
                horizontalArrangement = Arrangement.spacedBy(tokenDp("space.section")),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TheatreEmptyArtwork(TheatreEmptyArtworkKind.Library, theme,
                    Modifier.width(224.dp).aspectRatio(1.2f).testTag("library-empty-artwork"))
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.inset"))) {
                    LibraryEmptyText(kind, copy, compact)
                    if (discover) DiscoverEmptyAction(copy, dispatch)
                }
            } else Column(
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.inset")),
            ) {
                if (hero || illustrated) TheatreEmptyArtwork(
                    if (hero) TheatreEmptyArtworkKind.Library else TheatreEmptyArtworkKind.History,
                    theme, Modifier.widthIn(max = 240.dp).fillMaxWidth().aspectRatio(1.2f)
                        .align(Alignment.CenterHorizontally).testTag(if (hero) "library-empty-artwork" else "history-empty-artwork"),
                )
                LibraryEmptyText(kind, copy, compact)
                if (discover) DiscoverEmptyAction(copy, dispatch)
            }
        }
    }
}

@Composable
private fun DiscoverEmptyAction(copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    Button(
        onClick = { dispatch(ShellEvent.Navigate(ShellRoute.Discovery)) },
        modifier = Modifier.heightIn(min = 48.dp).testTag("library-empty-discover"),
        shape = CantosShapes.full,
    ) {
        Icon(rememberCantosIconVector("arrow_forward"), null)
        Text(copy.text(CopyKey.Discover), Modifier.padding(start = tokenDp("space.sm")))
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
