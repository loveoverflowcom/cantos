package com.cantos.theatre.app

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.JvmCatalogTransport
import com.cantos.theatre.adapters.http.PlaybackAccessHttp
import com.cantos.theatre.adapters.http.PublicCatalogHttpAdapter
import com.cantos.theatre.adapters.identity.SecureNativeCredentialJournal
import com.cantos.ui.CantosTheme
import com.cantos.ui.UiPreferences
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.RegistrationController
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackFailure
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.CredentialRead
import com.cantos.theatre.ports.CredentialStore
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.StreamingMediaEngine
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.net.CookieHandler
import java.net.HttpURLConnection
import java.net.URI
import java.util.UUID
import java.util.concurrent.atomic.AtomicReference
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch

/** Desktop adapter only. Exact configured origin, bounded bytes, no ambient cookies/redirects. */
class PreviewNativeHttp(private val scope: CoroutineScope, private val origin: ApiOrigin) : NativeHttpPort {
    override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
        val active = AtomicReference<HttpURLConnection?>()
        val job = scope.launch(Dispatchers.IO) {
            val response = try {
                val uri = URI(request.url)
                val base = URI(origin.toString())
                if (uri.scheme != base.scheme || uri.rawAuthority != base.rawAuthority || uri.rawUserInfo != null ||
                    uri.rawFragment != null || CookieHandler.getDefault() != null) NativeHttpResponse(0, null, null, "configuration")
                else {
                    val connection = uri.toURL().openConnection() as HttpURLConnection
                    active.set(connection)
                    coroutineContext.ensureActive()
                    connection.instanceFollowRedirects = false
                    connection.useCaches = false
                    connection.connectTimeout = 5_000
                    connection.readTimeout = 5_000
                    connection.requestMethod = request.method
                    connection.setRequestProperty("Accept", "application/json")
                    request.bearer?.let { connection.setRequestProperty("Authorization", "Bearer $it") }
                    request.expectedSession?.let { connection.setRequestProperty("X-Cantos-Theatre-Session", it) }
                    request.body?.let { body ->
                        val bytes = body.toByteArray(Charsets.UTF_8)
                        connection.doOutput = true
                        connection.setRequestProperty("Content-Type", "application/json")
                        connection.setFixedLengthStreamingMode(bytes.size)
                        connection.outputStream.use { it.write(bytes) }
                    }
                    val status = connection.responseCode
                    val input = if (status >= 400) connection.errorStream else connection.inputStream
                    val bytes = input?.use { stream ->
                        val output = ByteArrayOutputStream()
                        val buffer = ByteArray(8192)
                        while (true) {
                            coroutineContext.ensureActive()
                            val count = stream.read(buffer)
                            if (count < 0) break
                            if (count == 0 || output.size() + count > 65_536) throw IOException("Preview HTTP response limit")
                            output.write(buffer, 0, count)
                        }
                        output.toByteArray()
                    } ?: byteArrayOf()
                    NativeHttpResponse(status, connection.contentType, bytes.decodeToString(throwOnInvalidSequence = true), null,
                        connection.getHeaderField("X-Cantos-Theatre-Session"), connection.getHeaderField("Retry-After"))
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

/** Real v1 transport by default; injectable HTTP/clock are external test ports, never a fallback. */
class LiveEnrollmentOwners(
    scope: CoroutineScope,
    private val origin: ApiOrigin,
    http: NativeHttpPort = PreviewNativeHttp(scope, origin),
    now: () -> Long = { System.currentTimeMillis() / 1000 },
) {
    private var credential: String? = null
    private var logoutPending = false
    private val api = com.cantos.theatre.adapters.http.NativeIdentityV1Client(origin, http, now)
    val account = NativeAccountController(scope, api,
        SecureNativeCredentialJournal(object : CredentialStore {
            override fun read(): CredentialRead = credential?.let(CredentialRead::Found) ?: CredentialRead.Missing
            override fun write(value: String): CredentialFailure? { credential = value; return null }
            override fun clear(): CredentialFailure? { credential = null; return null }
        }, origin), object : LogoutBarrier {
            override fun read() = if (logoutPending) LogoutBarrierRead.Pending else LogoutBarrierRead.Clear
            override fun mark(): CredentialFailure? { logoutPending = true; return null }
            override fun clear(): CredentialFailure? { logoutPending = false; return null }
        }, now)
    val registration = RegistrationController(scope, api)
    private val catalog = PublicCatalogController(scope, PublicCatalogHttpAdapter(origin, JvmCatalogTransport()))
    private val runtime = NativeSessionRuntime(scope, { UnavailableDesktopAudio() }, PlaybackAccessHttp(origin, http) { now() * 1000 },
        account, null, { UUID.randomUUID().toString() }, { now() * 1000 })
    val shell = TheatreShell(scope, catalog, runtime, PreferencesController(), registration = registration)
        .also { it.openRegister() }
    fun close() { shell.close(); registration.close(); runtime.close(); catalog.close(); credential = null }
}

/** No decoder is installed in the auth preview. Requests fail honestly instead of simulating audio. */
private class UnavailableDesktopAudio : StreamingMediaEngine {
    private val observations = MutableSharedFlow<PlaybackObservation>(extraBufferCapacity = 4)
    override val events = observations.asSharedFlow()
    private fun unavailable(command: PlaybackEngineCommand) { observations.tryEmit(PlaybackEvent.EngineFailed(command.identity, PlaybackFailure.Unavailable)) }
    override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = unavailable(command)
    override suspend fun prepare(command: PlaybackEngineCommand) = unavailable(command)
    override suspend fun start(command: PlaybackEngineCommand) = unavailable(command)
    override suspend fun pause(command: PlaybackEngineCommand) = Unit
    override suspend fun seek(command: PlaybackEngineCommand) = Unit
    override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
    override fun close() = Unit
}

@Composable
fun LiveEnrollmentPreview(origin: ApiOrigin) {
    val scope = rememberCoroutineScope()
    val owners = remember(origin) { LiveEnrollmentOwners(scope, origin) }
    DisposableEffect(owners) { onDispose { owners.close() } }
    CantosTheme(UiPreferences()) {
        CantosTheatreApp(owners.shell)
    }
}
