package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.MenuBook
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import com.cantos.theatre.demo.DemoWorkId
import com.cantos.theatre.resources.Res
import com.cantos.theatre.resources.noto_sans_bold
import com.cantos.theatre.resources.noto_sans_regular
import com.cantos.theatre.resources.noto_serif_bold
import com.cantos.theatre.resources.noto_serif_regular
import kotlinx.coroutines.CancellationException
import org.jetbrains.compose.resources.Font
import org.jetbrains.compose.resources.decodeToImageBitmap

@Composable
internal fun theatreSans(): FontFamily = FontFamily(
    Font(Res.font.noto_sans_regular, FontWeight.Normal),
    Font(Res.font.noto_sans_bold, FontWeight.Bold),
)

@Composable
internal fun theatreSerif(): FontFamily = FontFamily(
    Font(Res.font.noto_serif_regular, FontWeight.Normal),
    Font(Res.font.noto_serif_bold, FontWeight.Bold),
)

/** Exact source bytes stay in CMP resources. Unknown/missing/undecodable art has a tonal fallback.
 * Titles outside the art identify the work; the painting is decorative and never a focus stop.
 */
@Composable
internal fun WorkArtwork(id: DemoWorkId, modifier: Modifier = Modifier) {
    val path = when (id) {
        DemoWorkId.SonHa -> "drawable/son_ha_moon.png"
        DemoWorkId.DenKhua -> "drawable/den_khuya.png"
        // The supplied third painting depicts a different work; do not relabel its provenance.
        DemoWorkId.CanhBuom -> null
    }
    val bitmap by produceState<ImageBitmap?>(null, path) {
        value = if (path == null) null else loadArtwork(path)
    }
    Surface(modifier, color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = MaterialTheme.shapes.medium) {
        val image = bitmap
        if (image == null) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Icon(Icons.AutoMirrored.Outlined.MenuBook, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
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
    // Resource loading is the outer rendering boundary; failure must not hide listening controls.
    null
}
