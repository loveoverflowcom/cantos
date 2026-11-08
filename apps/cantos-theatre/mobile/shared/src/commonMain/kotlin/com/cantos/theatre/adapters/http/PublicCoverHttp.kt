package com.cantos.theatre.adapters.http

import com.cantos.theatre.adapters.image.webpGeometry
import com.cantos.theatre.domain.catalog.CoverVariant
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** A byte request for the exact variant named by the admitted Work metadata. */
data class CoverRequest(
    val url: String,
    val sha256: String,
    val byteLength: Int,
    val contentType: String,
    val variant: CoverVariant,
    val widthPx: Int,
    val heightPx: Int,
    val ifNoneMatch: String? = null,
) {
    internal val expectedEtag: String get() = "\"$sha256\""
}

sealed interface CoverBytes {
    class Verified200(val bytes: ByteArray, val etag: String) : CoverBytes
    class NotModified304(val etag: String) : CoverBytes
    data object Denied : CoverBytes
    data object Unavailable : CoverBytes
    data object Refused : CoverBytes
}

fun interface CoverBytesSource {
    suspend fun fetch(request: CoverRequest): CoverBytes
}

/** Anonymous, bounded GET. Only matching, current Server admission can authorize cached pixels. */
class PublicCoverHttp(private val transport: CatalogHttpTransport) : CoverBytesSource {
    override suspend fun fetch(request: CoverRequest): CoverBytes {
        if (request.contentType != "image/webp" ||
            request.byteLength !in 1..request.variant.maxBytes ||
            request.widthPx !in 1..request.variant.maxWidth || request.heightPx !in 1..request.variant.maxHeight ||
            request.sha256.length != 64 || request.sha256.any { it !in '0'..'9' && it !in 'a'..'f' } ||
            (request.ifNoneMatch != null && request.ifNoneMatch != request.expectedEtag)
        ) return CoverBytes.Refused
        val response = try {
            transport.get(CatalogHttpRequest(request.url, request.byteLength, request.contentType, request.ifNoneMatch))
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: CatalogResponseTooLargeException) {
            return CoverBytes.Refused
        } catch (_: Exception) {
            return CoverBytes.Unavailable
        }
        return when (response.status) {
            200 -> verify(request, response)
            304 -> if (request.ifNoneMatch != null && response.etag == request.ifNoneMatch && response.body.isEmpty())
                CoverBytes.NotModified304(request.ifNoneMatch) else CoverBytes.Refused
            404 -> CoverBytes.Denied
            408, 500, 502, 503, 504 -> CoverBytes.Unavailable
            else -> CoverBytes.Refused
        }
    }

    private suspend fun verify(request: CoverRequest, response: CatalogHttpResponse): CoverBytes {
        val type = response.contentType?.substringBefore(';')?.trim()?.lowercase()
        if (type != request.contentType || response.body.size != request.byteLength ||
            response.etag != request.expectedEtag
        ) return CoverBytes.Refused
        return withContext(Dispatchers.Default) {
            val geometry = webpGeometry(response.body, request.variant)
            if (geometry != null && geometry.width == request.widthPx && geometry.height == request.heightPx &&
                sha256Hex(response.body) == request.sha256
            ) CoverBytes.Verified200(response.body, request.expectedEtag) else CoverBytes.Refused
        }
    }
}
