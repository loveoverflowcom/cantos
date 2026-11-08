package com.cantos.theatre.adapters.image

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap

internal actual val platformCoverDecoder: CoverDecoder = AndroidCoverDecoder

/** Platform WebP decode, first sampled in the codec and then bounded to the display edge. */
object AndroidCoverDecoder : CoverDecoder {
    override fun decode(bytes: ByteArray, maxEdgePx: Int): ImageBitmap? {
        val source = webpGeometry(bytes) ?: return null
        if (maxEdgePx !in 1..MaxWebpEdge) return null
        return try {
            var sample = 1
            while (source.edge / (sample * 2) >= maxEdgePx) sample *= 2
            val options = BitmapFactory.Options().apply {
                inSampleSize = sample
                inPreferredConfig = Bitmap.Config.ARGB_8888
                inScaled = false
            }
            val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options) ?: return null
            val target = source.fit(maxEdgePx)
            if (bitmap.width == target.width && bitmap.height == target.height) bitmap.asImageBitmap()
            else try {
                Bitmap.createScaledBitmap(bitmap, target.width, target.height, true).asImageBitmap()
            } finally {
                bitmap.recycle()
            }
        } catch (_: Exception) {
            null
        } catch (_: OutOfMemoryError) {
            null
        }
    }
}
