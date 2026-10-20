package com.cantos.theatre.adapters.http

import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.net.CookieHandler
import java.net.HttpURLConnection
import java.net.URI
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/**
 * HttpURLConnection catalog transport for desktop JVM and Android (shared `jvmHttpMain`).
 * Platform TLS is unchanged; ambient cookies are refused rather than shared; no redirect is
 * followed; nothing is cached; no Authorization header exists. Cancellation disconnects the
 * owned connection. On Android a missing INTERNET permission surfaces as a SecurityException,
 * which the adapter reports as a transport failure.
 */
class JvmCatalogTransport(
    private val connectTimeoutMs: Int = 10_000,
    private val readTimeoutMs: Int = 15_000,
) : CatalogHttpTransport {
    override suspend fun get(request: CatalogHttpRequest): CatalogHttpResponse = withContext(Dispatchers.IO) {
        if (CookieHandler.getDefault() != null) throw CookieRuntimeException()
        val connection = URI(request.url).toURL().openConnection() as HttpURLConnection
        suspendCancellableCoroutine { continuation ->
            continuation.invokeOnCancellation { connection.disconnect() }
            try {
                connection.instanceFollowRedirects = false
                connection.useCaches = false
                connection.connectTimeout = connectTimeoutMs
                connection.readTimeout = readTimeoutMs
                connection.requestMethod = "GET"
                connection.setRequestProperty("Accept", "application/json")
                connection.setRequestProperty("Cookie", "")
                val status = connection.responseCode
                if (connection.contentLengthLong > request.maxBodyBytes) throw CatalogResponseTooLargeException()
                val stream = if (status >= 400) connection.errorStream else connection.inputStream
                val bytes = stream?.use { it.readAtMost(request.maxBodyBytes) } ?: ByteArray(0)
                if (continuation.isActive) continuation.resume(CatalogHttpResponse(status, connection.contentType, bytes))
            } catch (failure: Exception) {
                if (continuation.isActive) continuation.resumeWithException(if (failure is java.net.SocketTimeoutException) CatalogTimeoutException() else failure)
            } finally {
                connection.disconnect()
            }
        }
    }
}

private fun InputStream.readAtMost(limit: Int): ByteArray {
    val output = ByteArrayOutputStream(minOf(limit, 64 * 1024))
    val buffer = ByteArray(16 * 1024)
    while (true) {
        val read = read(buffer)
        if (read == -1) return output.toByteArray()
        if (read == 0) throw IOException("No progress reading catalog response")
        if (output.size() + read > limit) throw CatalogResponseTooLargeException()
        output.write(buffer, 0, read)
    }
}
