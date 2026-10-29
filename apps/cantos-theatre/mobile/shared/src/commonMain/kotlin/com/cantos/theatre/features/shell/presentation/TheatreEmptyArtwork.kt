package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.Image
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.resources.Res
import com.cantos.theatre.resources.cantos_empty_history_dark
import com.cantos.theatre.resources.cantos_empty_history_light
import com.cantos.theatre.resources.cantos_empty_works_dark
import com.cantos.theatre.resources.cantos_empty_works_light
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.jetbrains.compose.resources.painterResource

/** Platform hosts supply a decoder; an absent capability retains the native vector. */
val LocalBundledImageDecoder = staticCompositionLocalOf<(ByteArray) -> ImageBitmap?> { { null } }

internal enum class TheatreEmptyArtworkKind { Library, Works, History }

/** Bundled artwork, chosen from the injected app preference; no request, tint or extra spoken name. */
@Composable
internal fun TheatreEmptyArtwork(kind: TheatreEmptyArtworkKind, theme: ShellTheme, modifier: Modifier = Modifier) {
    // AVIF is a raw resource: the pinned Compose drawable generator does not admit .avif.
    // A decoder failure uses the existing native vector, never a legacy raster fallback.
    val decoder = LocalBundledImageDecoder.current
    val library by produceState<ImageBitmap?>(null, kind, decoder) {
        if (kind == TheatreEmptyArtworkKind.Library) value = loadLibraryArtwork(decoder)
    }
    val bitmap = library
    if (kind == TheatreEmptyArtworkKind.Library && bitmap != null) {
        Image(bitmap, contentDescription = null, modifier = modifier, contentScale = ContentScale.Fit)
        return
    }
    val resource = when (kind) {
        TheatreEmptyArtworkKind.Library, TheatreEmptyArtworkKind.Works -> when (theme) {
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

internal suspend fun loadLibraryArtwork(decoder: (ByteArray) -> ImageBitmap?): ImageBitmap? = try {
    withContext(Dispatchers.Default) { decoder(Res.readBytes("files/artwork/library_expressive.avif")) }
} catch (cancelled: CancellationException) {
    throw cancelled
} catch (_: Exception) {
    null
}
