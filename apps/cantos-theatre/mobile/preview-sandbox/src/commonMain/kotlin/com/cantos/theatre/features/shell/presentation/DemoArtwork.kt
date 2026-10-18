package com.cantos.theatre.features.shell.presentation

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
import com.cantos.theatre.demo.DemoWorkIds
import com.cantos.theatre.preview.resources.Res
import kotlinx.coroutines.CancellationException
import org.jetbrains.compose.resources.decodeToImageBitmap
import com.cantos.ui.rememberCantosIconVector

@Composable
fun WithDemoArtwork(content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalWorkArtwork provides { id, modifier, shape -> DemoWorkArtwork(id, modifier, shape) }, content = content)
}

@Composable
private fun DemoWorkArtwork(id: ShellWorkId, modifier: Modifier, shape: Shape) {
    val path = when (id) {
        DemoWorkIds.SonHa -> "drawable/son_ha_moon.webp"
        DemoWorkIds.DenKhua -> "drawable/den_khuya.webp"
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
    Res.readBytes(path).decodeToImageBitmap()
} catch (cancelled: CancellationException) {
    throw cancelled
} catch (_: Exception) {
    null
}
