package com.cantos.theatre.adapters.image

import android.graphics.Bitmap
import android.os.Build
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import java.nio.ByteBuffer
import org.aomedia.avif.android.AvifDecoder

private const val MaxBundledAvifBytes = 8_388_608
private val bundledAvifLock = Any()
// One lazy, bounded direct buffer, reused under the same lock as the native decoder.
// No per-scroll direct-buffer allocation or concurrent full YUV frames on older devices.
private var bundledAvifInput: ByteBuffer? = null

/** Native baseline AVIF on API31+, with a bounded software path covering the API23 floor. */
internal actual fun decodeBundledAvifPixels(bytes: ByteArray): ImageBitmap? = try {
    val platform = if (Build.VERSION.SDK_INT >= 31) AndroidCoverDecoder.decode(bytes, CoverDecodeMaxEdgePx) else null
    platform ?: synchronized(bundledAvifLock) {
        if (bytes.isEmpty() || bytes.size > MaxBundledAvifBytes) return@synchronized null
        val input = bundledAvifInput ?: ByteBuffer.allocateDirect(MaxBundledAvifBytes).also { bundledAvifInput = it }
        input.clear()
        input.put(bytes)
        input.flip()
        // create() parses metadata. Dimensions/frame count are checked before nextFrame().
        val decoder = AvifDecoder.create(input, 1) ?: return@synchronized null
        try {
            if (decoder.frameCount != 1) return@synchronized null
            val dimensions = bundledDecodeDimensions(decoder.width, decoder.height, CoverDecodeMaxEdgePx)
                ?: return@synchronized null
            val bitmap = Bitmap.createBitmap(dimensions.first, dimensions.second, Bitmap.Config.ARGB_8888)
            if (decoder.nextFrame(bitmap) == 0) bitmap.asImageBitmap()
            else {
                bitmap.recycle()
                null
            }
        } finally {
            decoder.release()
        }
    }
} catch (_: Exception) {
    null
} catch (_: LinkageError) {
    null
} catch (_: OutOfMemoryError) {
    // A bounded native allocation can still fail under system pressure; keep the vector.
    null
}
