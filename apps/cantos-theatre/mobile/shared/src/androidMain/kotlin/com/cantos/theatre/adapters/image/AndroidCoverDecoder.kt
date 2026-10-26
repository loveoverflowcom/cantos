package com.cantos.theatre.adapters.image

import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap

/** Android decoding with a power-of-two downsample that keeps the larger edge at least `maxEdgePx`. */
object AndroidCoverDecoder : CoverDecoder {
    override fun decode(bytes: ByteArray, maxEdgePx: Int): ImageBitmap? = try {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        val edge = maxOf(bounds.outWidth, bounds.outHeight)
        var sample = 1
        while (edge / (sample * 2) >= maxEdgePx) sample *= 2
        val options = BitmapFactory.Options().apply { inSampleSize = sample }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)?.asImageBitmap()
    } catch (_: Exception) {
        null
    }
}
