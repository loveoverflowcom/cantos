package com.cantos.theatre.adapters.image

import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap

/** Platform decoding with bounded dimensions; software AVIF covers the API23 floor. */
object AndroidCoverDecoder : CoverDecoder {
    override fun decode(bytes: ByteArray, maxEdgePx: Int): ImageBitmap? {
        if (!hasStillAvifSignature(bytes) || maxEdgePx !in 1..4096) return null
        return try {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
            if (bundledDecodeDimensions(bounds.outWidth, bounds.outHeight, maxEdgePx) == null) {
                return decodeAvifSoftware(bytes, maxEdgePx)
            }
            val edge = maxOf(bounds.outWidth, bounds.outHeight)
            var sample = 1
            while (edge / (sample * 2) >= maxEdgePx) sample *= 2
            val options = BitmapFactory.Options().apply { inSampleSize = sample }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)?.asImageBitmap()
                ?: decodeAvifSoftware(bytes, maxEdgePx)
        } catch (_: Exception) {
            null
        } catch (_: OutOfMemoryError) {
            null
        }
    }
}
