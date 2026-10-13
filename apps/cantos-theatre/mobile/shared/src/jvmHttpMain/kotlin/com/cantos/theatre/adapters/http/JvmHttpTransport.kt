package com.cantos.theatre.adapters.http

import java.net.CookieHandler
import java.net.HttpURLConnection
import java.net.URI
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** Platform TLS verification is unchanged. Refuse ambient cookies rather than sharing them. */
class JvmHttpTransport : HttpTransport {
    override suspend fun get(request: ProbeRequest): ProbeResponse = withContext(Dispatchers.IO) {
        if (CookieHandler.getDefault() != null) throw CookieRuntimeException()
        val connection = URI(request.url).toURL().openConnection() as HttpURLConnection
        suspendCancellableCoroutine { continuation ->
            continuation.invokeOnCancellation { connection.disconnect() }
            try {
                connection.instanceFollowRedirects = false
                connection.useCaches = false
                connection.connectTimeout = 15_000
                connection.readTimeout = 15_000
                connection.requestMethod = "GET"
                connection.setRequestProperty("Accept", "application/json")
                connection.setRequestProperty("Cookie", "")
                val status = connection.responseCode
                val stream = if (status >= 400) connection.errorStream else connection.inputStream
                val bytes = stream?.use { it.readBytesBounded() } ?: ByteArray(0)
                if (continuation.isActive) continuation.resume(ProbeResponse(status, connection.contentType, bytes.decodeToString(throwOnInvalidSequence = true)))
            } catch (failure: Exception) {
                if (continuation.isActive) continuation.resumeWithException(failure)
            } finally { connection.disconnect() }
        }
    }
}
private fun java.io.InputStream.readBytesBounded(): ByteArray {
    val buffer = ByteArray(8193)
    var count = 0
    while (count < buffer.size) {
        val read = read(buffer, count, buffer.size - count)
        if (read == -1) return buffer.copyOf(count)
        if (read == 0) throw IllegalStateException("No progress reading listener response")
        count += read
    }
    throw IllegalStateException("Listener response exceeds 8 KiB")
}
