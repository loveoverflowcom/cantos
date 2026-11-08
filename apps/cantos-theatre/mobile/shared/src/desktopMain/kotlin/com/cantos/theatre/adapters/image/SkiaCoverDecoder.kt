package com.cantos.theatre.adapters.image

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap
import org.jetbrains.skia.Image
import org.jetbrains.skia.Rect
import org.jetbrains.skia.Surface

internal actual val platformCoverDecoder: CoverDecoder = SkiaCoverDecoder

/** Render the verified VP8 frame to a bounded bitmap with the Compose host's existing Skia. */
object SkiaCoverDecoder : CoverDecoder {
    override fun decode(bytes: ByteArray, maxEdgePx: Int): ImageBitmap? {
        val source = webpGeometry(bytes) ?: return null
        if (maxEdgePx !in 1..MaxWebpEdge) return null
        val target = source.fit(maxEdgePx)
        return try {
            Image.makeFromEncoded(bytes).use { image ->
                if (image.width != source.width || image.height != source.height) return null
                Surface.makeRasterN32Premul(target.width, target.height).use { surface ->
                    surface.canvas.drawImageRect(image, Rect.makeWH(target.width.toFloat(), target.height.toFloat()))
                    surface.makeImageSnapshot().toComposeImageBitmap()
                }
            }
        } catch (_: Exception) {
            null
        }
    }
}
