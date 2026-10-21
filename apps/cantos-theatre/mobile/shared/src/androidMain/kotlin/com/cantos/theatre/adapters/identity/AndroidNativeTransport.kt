package com.cantos.theatre.adapters.identity

import android.content.Context
import android.system.Os
import android.system.OsConstants
import android.system.ErrnoException
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.net.CookieHandler
import java.net.HttpURLConnection
import java.net.URI
import java.util.concurrent.atomic.AtomicReference
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.ensureActive

/** Nonsecret denial marker survives process death; no credentials or account data enter it. */
class AndroidLogoutBarrier(context: Context) : LogoutBarrier {
    private val marker = File(context.noBackupFilesDir, "theatre-logout-pending-v1")
    private fun syncDirectory() {
        // O_DIRECTORY is hidden from the public Android SDK. Check the opened descriptor
        // instead; every API here is public since API 21, below this host's API 23 floor.
        val directory = Os.open(marker.parentFile?.path ?: throw IOException("Missing identity directory"), OsConstants.O_RDONLY, 0)
        try {
            if (!OsConstants.S_ISDIR(Os.fstat(directory).st_mode)) throw IOException("Identity parent is not a directory")
            Os.fsync(directory)
        } finally { Os.close(directory) }
    }
    @Synchronized override fun read(): LogoutBarrierRead = try {
        if (marker.exists()) LogoutBarrierRead.Pending else if (marker.parentFile?.canWrite() == true) LogoutBarrierRead.Clear else LogoutBarrierRead.Unavailable
    } catch (_: SecurityException) { LogoutBarrierRead.Unavailable }
    @Synchronized override fun mark(): CredentialFailure? = try {
        FileOutputStream(marker).use { it.write(byteArrayOf(1)); it.fd.sync() }
        syncDirectory()
        null
    } catch (_: IOException) { CredentialFailure.Unavailable }
      catch (_: ErrnoException) { CredentialFailure.Unavailable }
      catch (_: SecurityException) { CredentialFailure.Denied }
    @Synchronized override fun clear(): CredentialFailure? = try {
        if (marker.exists() && !marker.delete()) CredentialFailure.Unavailable else { syncDirectory(); null }
    } catch (_: SecurityException) { CredentialFailure.Denied }
      catch (_: IOException) { CredentialFailure.Unavailable }
      catch (_: ErrnoException) { CredentialFailure.Unavailable }
}

/** OS TLS, no ambient cookies, no redirects; cancellation disconnects the owned connection. */
class AndroidNativeHttp(private val scope: CoroutineScope, private val apiOrigin: String) : NativeHttpPort {
    override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
        val active = AtomicReference<HttpURLConnection?>()
        val job = scope.launch(Dispatchers.IO) {
            val response = try {
                val uri = URI(request.url)
                val base = URI(apiOrigin)
                if (uri.scheme != "https" || uri.scheme != base.scheme || uri.rawAuthority != base.rawAuthority ||
                    uri.rawUserInfo != null || uri.fragment != null || CookieHandler.getDefault() != null ||
                    request.method !in setOf("GET", "POST", "PUT", "DELETE")) NativeHttpResponse(0, null, null, "configuration")
                else {
                    val connection = uri.toURL().openConnection() as HttpURLConnection
                    active.set(connection)
                    coroutineContext.ensureActive()
                    connection.instanceFollowRedirects = false
                    connection.useCaches = false
                    connection.connectTimeout = 5_000
                    connection.readTimeout = 5_000
                    connection.requestMethod = request.method
                    request.headers.forEach { (name, value) -> connection.setRequestProperty(name, value) }
                    request.body?.let {
                        val bytes = it.toByteArray(Charsets.UTF_8)
                        connection.doOutput = true
                        connection.setFixedLengthStreamingMode(bytes.size)
                        connection.outputStream.use { output -> output.write(bytes) }
                    }
                    val status = connection.responseCode
                    val input = if (status >= 400) connection.errorStream else connection.inputStream
                    val bytes = input?.use { stream ->
                        val output = ByteArrayOutputStream()
                        val buffer = ByteArray(8192)
                        while (true) {
                            coroutineContext.ensureActive()
                            val count = stream.read(buffer)
                            if (count == -1) break
                            if (count == 0 || output.size() + count > 65536) throw IOException("Native response limit")
                            output.write(buffer, 0, count)
                        }
                        output.toByteArray()
                    } ?: ByteArray(0)
                    NativeHttpResponse(status, connection.contentType, bytes.decodeToString(throwOnInvalidSequence = true), null, connection.getHeaderField("X-Cantos-Theatre-Session"), connection.getHeaderField("Retry-After"))
                }
            } catch (cancelled: CancellationException) { throw cancelled }
              catch (_: IOException) { NativeHttpResponse(0, null, null, "transport") }
              catch (_: SecurityException) { NativeHttpResponse(0, null, null, "denied") }
              catch (_: IllegalArgumentException) { NativeHttpResponse(0, null, null, "protocol") }
              finally { active.getAndSet(null)?.disconnect() }
            completion(response)
        }
        return object : NativeRequestHandle { override fun cancel() { active.getAndSet(null)?.disconnect(); job.cancel() } }
    }
}
