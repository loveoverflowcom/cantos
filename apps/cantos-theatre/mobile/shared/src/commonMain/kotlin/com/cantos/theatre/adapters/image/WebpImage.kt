package com.cantos.theatre.adapters.image

import com.cantos.theatre.domain.catalog.CoverVariant

internal data class ImageGeometry(val width: Int, val height: Int) {
    val edge: Int get() = maxOf(width, height)

    fun fit(maxEdgePx: Int): ImageGeometry {
        if (edge <= maxEdgePx) return this
        return ImageGeometry(
            (width * maxEdgePx / edge).coerceAtLeast(1),
            (height * maxEdgePx / edge).coerceAtLeast(1),
        )
    }
}

private const val MaxWebpBytes = 2_097_152
internal const val MaxWebpEdge = 1200

/** Bounds before any platform allocation. Accepts the Server's exact profile marker or a bundled VP8 file. */
internal fun webpGeometry(bytes: ByteArray, expected: CoverVariant? = null): ImageGeometry? {
    if (bytes.size !in 30..MaxWebpBytes ||
        !bytes.hasAscii(0, "RIFF") || !bytes.hasAscii(8, "WEBPVP8 ") ||
        bytes.le32(4) != bytes.size.toLong() - 8
    ) return null
    val chunkLength = bytes.le32(16)
    if (chunkLength < 10 || chunkLength > bytes.size - 20) return null
    val end = 20 + chunkLength.toInt()
    val padded = end + (chunkLength.toInt() and 1)
    if (padded > bytes.size || (end != padded && bytes[end] != 0.toByte())) return null
    if ((bytes[20].toInt() and 1) != 0 || bytes[23] != 0x9d.toByte() ||
        bytes[24] != 1.toByte() || bytes[25] != 0x2a.toByte()
    ) return null

    val variant = if (bytes.size == padded) null else {
        if (bytes.size - padded != 10 || !bytes.hasAscii(padded, "CTVR") ||
            bytes.le32(padded + 4) != 2L || bytes[padded + 8] != 1.toByte()
        ) return null
        when (bytes[padded + 9].toInt()) {
            'T'.code -> CoverVariant.Thumbnail
            'F'.code -> CoverVariant.Full
            else -> return null
        }
    }
    if (expected != null && variant != expected) return null
    val width = bytes.le16(26) and 0x3fff
    val height = bytes.le16(28) and 0x3fff
    if (width !in 1..(variant?.maxWidth ?: MaxWebpEdge) ||
        height !in 1..(variant?.maxHeight ?: MaxWebpEdge) ||
        bytes.size > (variant?.maxBytes ?: MaxWebpBytes)
    ) return null
    return ImageGeometry(width, height)
}

private fun ByteArray.hasAscii(offset: Int, text: String): Boolean =
    text.indices.all { this[offset + it].toInt() == text[it].code }

private fun ByteArray.le16(offset: Int): Int =
    (this[offset].toInt() and 255) or ((this[offset + 1].toInt() and 255) shl 8)

private fun ByteArray.le32(offset: Int): Long =
    le16(offset).toLong() or (le16(offset + 2).toLong() shl 16)

fun decodeBundledWebp(bytes: ByteArray): androidx.compose.ui.graphics.ImageBitmap? =
    webpGeometry(bytes)?.let { platformCoverDecoder.decode(bytes, it.edge) }

internal expect val platformCoverDecoder: CoverDecoder
