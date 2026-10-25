package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.Image
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.ContentScale
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.resources.Res
import com.cantos.theatre.resources.cantos_empty_history_dark
import com.cantos.theatre.resources.cantos_empty_history_light
import com.cantos.theatre.resources.cantos_empty_works_dark
import com.cantos.theatre.resources.cantos_empty_works_light
import com.cantos.theatre.resources.library_expressive
import org.jetbrains.compose.resources.painterResource

internal enum class TheatreEmptyArtworkKind { Library, Works, History }

/** Bundled artwork, chosen from the injected app preference; no request, tint or extra spoken name. */
@Composable
internal fun TheatreEmptyArtwork(kind: TheatreEmptyArtworkKind, theme: ShellTheme, modifier: Modifier = Modifier) {
    val resource = when (kind) {
        TheatreEmptyArtworkKind.Library -> Res.drawable.library_expressive
        TheatreEmptyArtworkKind.Works -> when (theme) {
            ShellTheme.Light -> Res.drawable.cantos_empty_works_light
            ShellTheme.Dark -> Res.drawable.cantos_empty_works_dark
        }
        TheatreEmptyArtworkKind.History -> when (theme) {
            ShellTheme.Light -> Res.drawable.cantos_empty_history_light
            ShellTheme.Dark -> Res.drawable.cantos_empty_history_dark
        }
    }
    Image(painterResource(resource), contentDescription = null, modifier = modifier, contentScale = ContentScale.Fit)
}
