package com.cantos.theatre.app

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.window.ComposeUIViewController
import com.cantos.theatre.adapters.http.CatalogHttpRequest
import com.cantos.theatre.adapters.http.CatalogHttpResponse
import com.cantos.theatre.adapters.http.CatalogHttpTransport
import com.cantos.theatre.adapters.http.CatalogResponseTooLargeException
import com.cantos.theatre.adapters.http.PublicCatalogHttpAdapter
import com.cantos.theatre.adapters.http.selectPackagedApiOrigin
import com.cantos.theatre.adapters.preferences.IosLocalePreferenceStore
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.adapters.http.PlaybackAccessHttp
import com.cantos.theatre.adapters.http.NativeAuthorizedHttpPort
import com.cantos.theatre.adapters.library.AccountCatalogCache
import com.cantos.theatre.adapters.library.NativeLibraryClient
import com.cantos.theatre.adapters.library.iosLibraryStore
import com.cantos.theatre.adapters.playback.IosMediaDriver
import com.cantos.theatre.adapters.playback.IosMediaEngine
import com.cantos.theatre.adapters.library.nativeLibraryController
import platform.Foundation.NSDate
import platform.Foundation.NSUUID
import com.cantos.theatre.features.launch.presentation.CantosListeningUnavailable
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.ui.ThemeMode
import com.cantos.ui.UiPreferences
import com.cantos.ui.resolve
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.suspendCancellableCoroutine
import platform.UIKit.UIViewController
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** Only the Swift URLSession boundary supplies these foreign outcomes. No credentials. */
class IosCatalogReply(val status: Int, val contentType: String?, val body: String?, val failure: String?)

interface IosCatalogPlatform {
    fun sendCatalog(request: CatalogHttpRequest, completion: (IosCatalogReply) -> Unit): NativeRequestHandle
}

private class IosCatalogTransport(private val platform: IosCatalogPlatform) : CatalogHttpTransport {
    override suspend fun get(request: CatalogHttpRequest): CatalogHttpResponse = suspendCancellableCoroutine { continuation ->
        val handle = platform.sendCatalog(request) { reply ->
            if (continuation.isActive) {
                when (reply.failure) {
                    "oversized" -> continuation.resumeWithException(CatalogResponseTooLargeException())
                    "malformed" -> continuation.resume(CatalogHttpResponse(reply.status, reply.contentType, byteArrayOf(-1)))
                    null -> {
                        val body = reply.body
                        if (body == null) continuation.resumeWithException(IllegalStateException("Missing catalog body"))
                        else continuation.resume(CatalogHttpResponse(reply.status, reply.contentType, body.encodeToByteArray()))
                    }
                    else -> continuation.resumeWithException(IllegalStateException("Catalog transport unavailable"))
                }
            }
        }
        continuation.invokeOnCancellation { handle.cancel() }
    }
}

/** Process-owned production catalog. A cold empty catalog creates no playback or demo state. */
class IosCatalogOwner(apiOrigin: String?, platform: IosCatalogPlatform,
                      identity: IosNativeIdentityOwner, identityPlatform: IosIdentityPlatform,
                      mediaDriver: IosMediaDriver, applicationSupportDirectory: String) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val preferences = PreferencesController(localePreferenceStore = IosLocalePreferenceStore())
    // The packaged explicit override, else the production default; a refusal is typed and shown, never a mock.
    private val selectedOrigin = hostOrigin(selectPackagedApiOrigin(apiOrigin, allowLoopbackHttp = false))
    private var accountScope: String? = null
    private val store = (selectedOrigin as? HostOrigin.Ready)?.let {
        // Foreign database initialization is contained at the host; public discovery remains usable.
        try { iosLibraryStore(applicationSupportDirectory) } catch (_: Exception) { null }
    }
    private val cache = store?.let { AccountCatalogCache(it) { accountScope } }
    private val catalog = when (val origin = selectedOrigin) {
        is HostOrigin.Ready -> PublicCatalogController(scope,
            PublicCatalogHttpAdapter(origin.origin, IosCatalogTransport(platform), cache), { cache?.invalidate() })
            .also { it.dispatch(PublicCatalogIntent.Start) }
        is HostOrigin.Refused -> null
    }
    private val runtime = (selectedOrigin as? HostOrigin.Ready)?.let { configured ->
        val origin = configured.origin
        val authorizedHttp = identity.controller?.let { NativeAuthorizedHttpPort(scope, origin, it, identityPlatform) }
            ?: identityPlatform
        val library = store?.let { nativeLibraryController(scope, NativeLibraryClient(origin, authorizedHttp), it,
            { NSUUID().UUIDString.lowercase() }, onSessionInvalidated = { expected -> identity.controller?.revalidate(expected) },
            ownsAuthorization = { owner -> identity.controller?.let { account ->
                account.belongsToCurrent(owner.bearer, owner.session) &&
                    account.verifiedSession.value?.session?.actor == owner.account
            } == true },
            nowMs = { (NSDate().timeIntervalSince1970 * 1000).toLong() }) }
        NativeSessionRuntime(scope, { onControl -> IosMediaEngine(mediaDriver, onControl) },
            PlaybackAccessHttp(origin, identityPlatform) { (NSDate().timeIntervalSince1970 * 1000).toLong() },
            identity.controller, library, { NSUUID().UUIDString.lowercase() },
            { (NSDate().timeIntervalSince1970 * 1000).toLong() }, onAccountChanged = { actor ->
                if (actor != accountScope) {
                    accountScope = actor
                    reloadCatalogForAccount()
                }
            })
    }


    /** Navigation and typed route text live with the process owners, like the catalog and runtime. */
    private val shell = if (catalog != null && runtime != null) TheatreShell(scope, catalog, runtime, preferences, registration = identity.registration) else null

    fun viewController(): UIViewController = ComposeUIViewController {
        val locale by preferences.state.collectAsState()
        val dark = isSystemInDarkTheme()
        // The theme starts from the system and follows it when it changes; otherwise it is the listener's choice.
        remember(dark) { preferences.setTheme(if (dark) PreferenceTheme.Dark else PreferenceTheme.Light) }
        val ui = UiPreferences(theme = ThemeMode.System.resolve(systemIsDark = dark))
        val owner = shell
        if (owner == null) CantosListeningUnavailable(ui, locale = locale.locale.toShellLocale(),
            fault = (selectedOrigin as? HostOrigin.Refused)?.fault)
        else CantosTheatreApp(owner, cache = cache)
    }

    private fun reloadCatalogForAccount() { catalog?.dispatch(PublicCatalogIntent.AccountChanged) }

    fun backgrounded() { runtime?.backgrounded() }

    fun close() {
        shell?.close()
        runtime?.close()
        catalog?.close()
        scope.cancel()
    }
}
