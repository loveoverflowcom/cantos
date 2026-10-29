package com.cantos.theatre.adapters.image

import androidx.compose.ui.graphics.ImageBitmap

/** Trusted, locally bundled artwork only. Remote cover admission stays with its own contract. */
fun decodeBundledAvif(bytes: ByteArray): ImageBitmap? =
    if (hasStillAvifSignature(bytes)) decodeBundledAvifPixels(bytes) else null

internal expect fun decodeBundledAvifPixels(bytes: ByteArray): ImageBitmap?

/** Format identification only; successful full decoding is still required by each adapter. */
internal fun hasStillAvifSignature(bytes: ByteArray): Boolean {
    if (bytes.size !in 16..8_388_608) return false
    fun matches(offset: Int, text: String): Boolean = text.indices.all { index ->
        bytes[offset + index] == text[index].code.toByte()
    }
    if (!matches(4, "ftyp")) return false
    val length = (0..3).fold(0L) { value, index -> (value shl 8) or (bytes[index].toLong() and 255) }
    if (length !in 16..bytes.size.toLong() || length % 4 != 0L) return false
    val brands = listOf(8) + (16 until length.toInt() step 4)
    return brands.any { matches(it, "avif") } && brands.none { matches(it, "avis") }
}

/** Pixel/allocation plan checked before the JNI decode; it is not a total native-RSS bound. */
internal fun bundledDecodeDimensions(width: Int, height: Int, maxEdge: Int): Pair<Int, Int>? {
    if (width !in 1..4096 || height !in 1..4096 || maxEdge !in 1..4096) return null
    if (width.toLong() * height > 16_777_216L) return null
    val edge = maxOf(width, height)
    return if (edge <= maxEdge) width to height
    else maxOf(1, (width.toLong() * maxEdge / edge).toInt()) to
        maxOf(1, (height.toLong() * maxEdge / edge).toInt())
}
