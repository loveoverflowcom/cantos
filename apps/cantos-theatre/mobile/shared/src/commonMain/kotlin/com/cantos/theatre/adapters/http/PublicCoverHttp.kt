package com.cantos.theatre.adapters.http

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * What the Server authorized for one cover, used exactly as given. Built only from a validated
 * [com.cantos.theatre.domain.catalog.PublicCover]; the URL is never derived or rewritten here.
 */
class CoverRequest(val url: String, val sha256: String, val byteLength: Int, val contentType: String)

/** Closed outcome of one anonymous cover read. Every non-[Verified] value shows the fallback. */
sealed interface CoverBytes {
    /** Status 200, the exact pinned type and length, and a SHA-256 equal to the disclosed one. */
    class Verified(val bytes: ByteArray) : CoverBytes
    /** Current 404 denial: stale, revoked, expired, withdrawn or uncomposed. Never retained. */
    data object NotFound : CoverBytes
    /** 5xx, timeout or no response: keep the fallback; a later composition may retry. */
    data object Unavailable : CoverBytes
    /** A response that contradicts the pinned facts (type, length, digest, status, redirect). */
    data object Refused : CoverBytes
}

fun interface CoverBytesSource {
    suspend fun fetch(request: CoverRequest): CoverBytes
}

/**
 * Anonymous GET of one public cover (Discovery v1.2): no cookie, credential or redirect, a body
 * bound of exactly the disclosed `byte_length`, then an exact content-type, length and SHA-256
 * comparison before any byte is decoded. Cancellation propagates; nothing else throws.
 */
class PublicCoverHttp(private val transport: CatalogHttpTransport) : CoverBytesSource {
    override suspend fun fetch(request: CoverRequest): CoverBytes {
        if (request.contentType != "image/avif") return CoverBytes.Refused
        val response = try {
            transport.get(CatalogHttpRequest(request.url, request.byteLength, accept = request.contentType))
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: CatalogResponseTooLargeException) {
            return CoverBytes.Refused
        } catch (_: Exception) {
            return CoverBytes.Unavailable
        }
        return when (response.status) {
            200 -> verify(request, response)
            404 -> CoverBytes.NotFound
            408, 500, 502, 503, 504 -> CoverBytes.Unavailable
            else -> CoverBytes.Refused
        }
    }

    private suspend fun verify(request: CoverRequest, response: CatalogHttpResponse): CoverBytes {
        val type = response.contentType?.substringBefore(';')?.trim()?.lowercase()
        if (type != request.contentType || response.body.size != request.byteLength) return CoverBytes.Refused
        val digest = withContext(Dispatchers.Default) { sha256Hex(response.body) }
        return if (digest == request.sha256) CoverBytes.Verified(response.body) else CoverBytes.Refused
    }
}
