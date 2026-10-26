package com.cantos.theatre.adapters.image

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap

/** Desktop JVM decoding through Skia (the renderer Compose already uses). */
object SkiaCoverDecoder : CoverDecoder {
    override fun decode(bytes: ByteArray, maxEdgePx: Int): ImageBitmap? = try {
        org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap()
    } catch (_: Exception) {
        null
    }
}
