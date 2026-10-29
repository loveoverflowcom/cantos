package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.PreviewWorkIds
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.features.shell.presentation.ArtworkFallback
import com.cantos.theatre.features.shell.presentation.LocalWorkArtwork
import com.cantos.theatre.preview.resources.Res
import kotlinx.coroutines.CancellationException
import org.jetbrains.compose.resources.decodeToImageBitmap

@Composable
fun WithPreviewArtwork(content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalWorkArtwork provides { id, modifier, shape -> PreviewWorkArtwork(id, modifier, shape) }, content = content)
}

@Composable
private fun PreviewWorkArtwork(id: ShellWorkId, modifier: Modifier, shape: Shape) {
    val path = when (id) {
        PreviewWorkIds.SonHa -> "drawable/son_ha_moon.webp"
        PreviewWorkIds.DenKhua -> "drawable/den_khuya.webp"
        else -> null
    }
    val bitmap by produceState<ImageBitmap?>(null, path) {
        value = if (path == null) null else loadArtwork(path)
    }
    val image = bitmap
    if (image == null) {
        ArtworkFallback(modifier, shape)
    } else {
        Surface(modifier, color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = shape) {
            Image(image, contentDescription = null, modifier = Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        }
    }
}

internal suspend fun loadArtwork(path: String): ImageBitmap? = try {
    Res.readBytes(path).decodeToImageBitmap()
} catch (cancelled: CancellationException) {
    throw cancelled
} catch (_: Exception) {
    null
}
