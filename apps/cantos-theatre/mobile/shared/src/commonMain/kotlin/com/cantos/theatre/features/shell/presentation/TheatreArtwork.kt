package com.cantos.theatre.features.shell.presentation

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.produceState
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
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
import com.cantos.ui.LocalUiPreferences
import com.cantos.ui.loadingRevealFor
import com.cantos.ui.toAnimationSpec
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

internal const val CoverFallbackResourcePath: String = "files/artwork/cantos_cover_fallback.avif"

private sealed interface CoverFallbackArt {
    data object Loading : CoverFallbackArt
    data class Ready(val image: ImageBitmap) : CoverFallbackArt

    /** The platform cannot decode the packaged AVIF: the vector backdrop is the fallback. */
    data object Unavailable : CoverFallbackArt
}

@Composable
fun ArtworkFallback(modifier: Modifier, shape: Shape) {
    val art by produceState<CoverFallbackArt>(CoverFallbackArt.Loading) {
        value = try {
            CoverFallbackArt.Ready(Res.readBytes(CoverFallbackResourcePath).decodeToImageBitmap())
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            CoverFallbackArt.Unavailable
        }
    }
    Surface(modifier, color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = shape) {
        when (val resolved = art) {
            CoverFallbackArt.Loading -> Unit
            is CoverFallbackArt.Ready -> Image(
                bitmap = resolved.image,
                contentDescription = null,
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
            CoverFallbackArt.Unavailable -> CelestialCoverBackdrop(Modifier.fillMaxSize())
        }
    }
}

@Composable
private fun CelestialCoverBackdrop(modifier: Modifier) {
    val nightTop = Color(0xFF141129)
    val nightMid = Color(0xFF221A3F)
    val nightBottom = Color(0xFF0E0B1D)
    val gold = Color(0xFFE4B869)
    val softGold = Color(0x59E4B869)
    val warmGlow = Color(0x33D99B4E)
    Canvas(modifier) {
        val w = size.width
        val h = size.height
        drawRect(
            brush = Brush.verticalGradient(
                0f to nightTop,
                0.58f to nightMid,
                1f to nightBottom,
            ),
        )
        drawCircle(
            brush = Brush.radialGradient(
                colors = listOf(warmGlow, Color.Transparent),
                center = Offset(w * 0.5f, h * 0.36f),
                radius = w * 0.56f,
            ),
            radius = w * 0.56f,
            center = Offset(w * 0.5f, h * 0.36f),
        )
        drawArc(
            color = softGold,
            startAngle = 200f,
            sweepAngle = 140f,
            useCenter = false,
            topLeft = Offset(w * 0.14f, h * 0.14f),
            size = Size(w * 0.72f, w * 0.72f),
            style = Stroke(width = (w * 0.012f).coerceAtLeast(1f)),
        )
        val moonRadius = w * 0.13f
        val moonCenter = Offset(w * 0.5f, h * 0.33f)
        drawCircle(color = gold, radius = moonRadius, center = moonCenter)
        drawCircle(
            color = nightMid,
            radius = moonRadius * 0.84f,
            center = Offset(moonCenter.x + moonRadius * 0.36f, moonCenter.y - moonRadius * 0.18f),
        )
        val stars = listOf(
            Triple(0.24f, 0.22f, 0.014f),
            Triple(0.76f, 0.24f, 0.012f),
            Triple(0.32f, 0.46f, 0.010f),
            Triple(0.70f, 0.44f, 0.013f),
            Triple(0.50f, 0.18f, 0.011f),
        )
        stars.forEach { (sx, sy, sr) ->
            drawCircle(
                color = gold.copy(alpha = 0.78f),
                radius = (w * sr).coerceAtLeast(1f),
                center = Offset(w * sx, h * sy),
            )
        }
        val bookPath = Path().apply {
            moveTo(w * 0.18f, h * 0.80f)
            quadraticTo(w * 0.34f, h * 0.73f, w * 0.5f, h * 0.79f)
            quadraticTo(w * 0.66f, h * 0.73f, w * 0.82f, h * 0.80f)
            lineTo(w * 0.86f, h * 0.85f)
            quadraticTo(w * 0.67f, h * 0.79f, w * 0.5f, h * 0.84f)
            quadraticTo(w * 0.33f, h * 0.79f, w * 0.14f, h * 0.85f)
            close()
        }
        drawPath(path = bookPath, color = softGold)
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
    val revealed by animateFloatAsState(if (loaded is CoverLoad.Ready) 1f else 0f,
        loadingRevealFor(LocalUiPreferences.current.motion).toAnimationSpec(), label = "cover-reveal")
    when (val result = loaded) {
        is CoverLoad.Ready -> Surface(modifier, color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = shape) {
            Image(
                bitmap = result.bitmap,
                contentDescription = cover.altText,
                modifier = Modifier.fillMaxSize().graphicsLayer { alpha = revealed }.testTag("cover-image"),
                contentScale = contentScale,
            )
        }
        null -> RequestPulse(modifier, shape = shape)
        CoverLoad.Unavailable -> ArtworkFallback(modifier, shape)
    }
}
