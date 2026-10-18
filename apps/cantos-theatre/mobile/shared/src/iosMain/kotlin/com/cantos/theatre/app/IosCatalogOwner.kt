package com.cantos.theatre.app

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.window.ComposeUIViewController
import com.cantos.theatre.adapters.http.CatalogHttpRequest
import com.cantos.theatre.adapters.http.CatalogHttpResponse
import com.cantos.theatre.adapters.http.CatalogHttpTransport
import com.cantos.theatre.adapters.http.CatalogOriginSelection
import com.cantos.theatre.adapters.http.CatalogResponseTooLargeException
import com.cantos.theatre.adapters.http.PublicCatalogHttpAdapter
import com.cantos.theatre.adapters.http.selectCatalogOrigin
import com.cantos.theatre.adapters.preferences.IosLocalePreferenceStore
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.features.catalog.presentation.PublicCatalogScreen
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
class IosCatalogOwner(apiOrigin: String?, platform: IosCatalogPlatform) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val preferences = PreferencesController(localePreferenceStore = IosLocalePreferenceStore())
    private val catalog = when (val origin = selectCatalogOrigin(apiOrigin, allowLoopbackHttp = false)) {
        is CatalogOriginSelection.Configured -> PublicCatalogController(scope,
            PublicCatalogHttpAdapter(origin.origin, IosCatalogTransport(platform)))
            .also { it.dispatch(PublicCatalogIntent.Start) }
        CatalogOriginSelection.NotConfigured, CatalogOriginSelection.Rejected -> null
    }

    fun viewController(): UIViewController = ComposeUIViewController {
        val locale by preferences.state.collectAsState()
        val ui = UiPreferences(theme = ThemeMode.System.resolve(systemIsDark = isSystemInDarkTheme()))
        val controller = catalog
        if (controller == null) CantosListeningUnavailable(ui, locale = locale.locale.toShellLocale())
        else {
            val state by controller.state.collectAsState()
            PublicCatalogScreen(state, controller::dispatch, ui, locale = locale.locale.toShellLocale())
        }
    }

    fun close() {
        catalog?.close()
        scope.cancel()
    }
}
