package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.PreviewWorkIds
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.features.shell.presentation.LocalBundledImageDecoder
import com.cantos.theatre.features.shell.presentation.LocalWorkArtwork
import com.cantos.theatre.preview.resources.Res
import kotlinx.coroutines.CancellationException
import com.cantos.theatre.adapters.image.decodeBundledAvif
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import com.cantos.ui.rememberCantosIconVector

@Composable
fun WithPreviewArtwork(content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalBundledImageDecoder provides ::decodeBundledAvif, LocalWorkArtwork provides { id, modifier, shape -> PreviewWorkArtwork(id, modifier, shape) }, content = content)
}

@Composable
private fun PreviewWorkArtwork(id: ShellWorkId, modifier: Modifier, shape: Shape) {
    val path = when (id) {
        PreviewWorkIds.SonHa -> "files/artwork/son_ha_moon.avif"
        PreviewWorkIds.DenKhua -> "files/artwork/den_khuya.avif"
        else -> null
    }
    val bitmap by produceState<ImageBitmap?>(null, path) {
        value = if (path == null) null else loadArtwork(path)
    }
    Surface(modifier, color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = shape) {
        val image = bitmap
        if (image == null) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Icon(rememberCantosIconVector("menu_book"), null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        } else {
            Image(image, contentDescription = null, modifier = Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        }
    }
}

internal suspend fun loadArtwork(path: String): ImageBitmap? = try {
    withContext(Dispatchers.Default) { decodeBundledAvif(Res.readBytes(path)) }
} catch (cancelled: CancellationException) {
    throw cancelled
} catch (_: Exception) {
    null
}
