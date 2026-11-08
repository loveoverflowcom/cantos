package com.cantos.theatre.adapters.image

import androidx.compose.ui.graphics.ImageBitmap

/** Decode one verified opaque WebP, returning a bitmap bounded by the physical display edge. */
fun interface CoverDecoder {
    fun decode(bytes: ByteArray, maxEdgePx: Int): ImageBitmap?
}
