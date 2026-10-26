package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import com.cantos.theatre.presentation.model.ShellCover
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.resources.Res
import com.cantos.theatre.resources.noto_sans_bold
import com.cantos.theatre.resources.noto_sans_regular
import com.cantos.theatre.resources.noto_serif_bold
import com.cantos.theatre.resources.noto_serif_regular
import kotlinx.coroutines.CancellationException
import org.jetbrains.compose.resources.Font
import com.cantos.ui.rememberCantosIconVector

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

/** A host supplies approved artwork. Missing artwork keeps the accessible tonal fallback. */
typealias WorkArtworkContent = @Composable (ShellWorkId, Modifier, Shape) -> Unit
val LocalWorkArtwork = androidx.compose.runtime.staticCompositionLocalOf<WorkArtworkContent?> { null }

/** One cover read: a decoded image, or [Unavailable] for every kind of failure (never an error UI). */
sealed interface CoverLoad {
    class Ready(val bitmap: ImageBitmap) : CoverLoad
    data object Unavailable : CoverLoad
}

/**
 * Loads the bytes the Server authorized for [ShellCover], verified and decoded by the host.
 * Implementations throw only [kotlinx.coroutines.CancellationException]; a host without an image
 * pipeline supplies none and every cover keeps the fallback.
 */
fun interface CoverImageSource {
    suspend fun load(cover: ShellCover): CoverLoad
}

val LocalCoverImages = androidx.compose.runtime.staticCompositionLocalOf<CoverImageSource?> { null }

/**
 * A Work's artwork. A Server-authorized [cover] is shown when the host can load it; while it
 * loads and whenever it cannot be shown (revoked, missing, unavailable, undecodable) the
 * decorative tonal fallback stays, so the title, navigation and playback never wait on an image.
 * The fallback never borrows another Work's image and a supplied preview artwork is not a
 * stand-in for a cover that failed.
 */
@Composable
internal fun WorkArtwork(
    id: ShellWorkId,
    modifier: Modifier = Modifier,
    shape: Shape = MaterialTheme.shapes.large,
    cover: ShellCover? = null,
    contentScale: ContentScale = ContentScale.Crop,
) {
    val source = LocalCoverImages.current
    if (cover != null && source != null) {
        CoverArtwork(cover, source, modifier, shape, contentScale)
        return
    }
    val supplied = LocalWorkArtwork.current
    if (supplied != null && cover == null) supplied(id, modifier, shape) else ArtworkFallback(modifier, shape)
}

@Composable
private fun ArtworkFallback(modifier: Modifier, shape: Shape) {
    Surface(modifier, color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = shape) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Icon(rememberCantosIconVector("menu_book"), null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun CoverArtwork(
    cover: ShellCover,
    source: CoverImageSource,
    modifier: Modifier,
    shape: Shape,
    contentScale: ContentScale,
) {
    // A digest does not grant another Work's authorization. Recreate the state when any
    // disclosed fact or the host source changes, so no old image survives the next pending read.
    val loaded by key(cover, source) {
        produceState<CoverLoad?>(null) {
            value = try {
                source.load(cover)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) {
                CoverLoad.Unavailable
            }
        }
    }
    when (val result = loaded) {
        is CoverLoad.Ready -> Surface(modifier, color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = shape) {
            Image(
                bitmap = result.bitmap,
                contentDescription = cover.altText,
                modifier = Modifier.fillMaxSize().testTag("cover-image"),
                contentScale = contentScale,
            )
        }
        else -> ArtworkFallback(modifier, shape)
    }
}
