package com.cantos.theatre.adapters.image

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap
import com.seiko.avif.AvifDecoder
import com.seiko.avif.createPlatformBitmap
import com.seiko.avif.use
import org.jetbrains.skia.Image

/** The pinned JNI codec is confined to desktop, with both native owners closed on every exit. */
internal actual fun decodeBundledAvifPixels(bytes: ByteArray): ImageBitmap? = try {
    if (bytes.isEmpty() || bytes.size > 8_388_608 || !AvifDecoder.isAvifImage(bytes)) null
    else AvifDecoder.create(bytes, threads = 1).use { decoder ->
        if (decoder.getFrameCount() != 1 || !decoder.nextFrame()) null
        else {
            val frame = decoder.getFrame()
            if (frame.getWidth() !in 1..4096 || frame.getHeight() !in 1..4096) null
            else frame.createPlatformBitmap().use { bitmap ->
                frame.decodeFrame(bitmap)
                Image.makeFromBitmap(bitmap).toComposeImageBitmap()
            }
        }
    }
} catch (_: Exception) {
    null
} catch (_: LinkageError) {
    // Unsupported desktop architecture: the existing native-vector placeholder remains visible.
    null
}
