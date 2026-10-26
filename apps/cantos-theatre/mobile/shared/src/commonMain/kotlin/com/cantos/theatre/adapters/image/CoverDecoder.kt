package com.cantos.theatre.adapters.image

import androidx.compose.ui.graphics.ImageBitmap

/** Largest edge a decoded cover needs; the Server admits up to 4096 px and ships no thumbnail. */
const val CoverDecodeMaxEdgePx: Int = 1536

/**
 * Platform image decoding of bytes whose type, length and SHA-256 were already verified.
 * Returns null for anything that does not decode; it never throws for bad data. Implementations
 * may downsample so that neither edge falls below [maxEdgePx] needlessly.
 */
fun interface CoverDecoder {
    fun decode(bytes: ByteArray, maxEdgePx: Int): ImageBitmap?
}
