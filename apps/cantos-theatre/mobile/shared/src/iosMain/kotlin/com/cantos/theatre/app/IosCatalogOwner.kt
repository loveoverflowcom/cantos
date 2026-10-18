package com.cantos.theatre.app

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
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.suspendCancellableCoroutine
import platform.UIKit.UIViewController
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** Native callback seam only: no credentials, cookies, redirects or sandbox dependencies. */
interface IosCatalogRequestHandle { fun cancel() }
enum class IosCatalogHttpFailure { Transport, TooLarge }
class IosCatalogHttpResponse(
    val status: Int,
    val contentType: String?,
    val body: ByteArray?,
    val failure: IosCatalogHttpFailure?,
)
interface IosCatalogPlatform {
    fun get(request: CatalogHttpRequest, completion: (IosCatalogHttpResponse) -> Unit): IosCatalogRequestHandle
}

private class IosCatalogTransport(private val platform: IosCatalogPlatform) : CatalogHttpTransport {
    override suspend fun get(request: CatalogHttpRequest): CatalogHttpResponse = suspendCancellableCoroutine { continuation ->
        val handle = platform.get(request) { response ->
            if (continuation.isActive) {
                when (response.failure) {
                    IosCatalogHttpFailure.TooLarge -> continuation.resumeWithException(CatalogResponseTooLargeException())
                    IosCatalogHttpFailure.Transport -> continuation.resumeWithException(IosCatalogTransportFailure())
                    null -> {
                        val body = response.body
                        if (body == null) continuation.resumeWithException(IosCatalogTransportFailure())
                        else continuation.resume(CatalogHttpResponse(response.status, response.contentType, body))
                    }
                }
            }
        }
        continuation.invokeOnCancellation { handle.cancel() }
    }
}

private class IosCatalogTransportFailure : IllegalStateException("Catalog transport unavailable")

/** SwiftUI retains this production-only catalog owner across view-controller remounts. */
class IosCatalogOwner(apiOrigin: String?, platform: IosCatalogPlatform) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val systemDark = MutableStateFlow(false)
    private val preferences = PreferencesController(localePreferenceStore = IosLocalePreferenceStore())
    private val origin = when (val selection = selectCatalogOrigin(apiOrigin, allowLoopbackHttp = false)) {
        is CatalogOriginSelection.Configured -> selection.origin
        CatalogOriginSelection.NotConfigured, CatalogOriginSelection.Rejected -> null
    }
    private val catalog = origin?.let {
        PublicCatalogController(scope, PublicCatalogHttpAdapter(it, IosCatalogTransport(platform)))
            .also { controller -> controller.dispatch(PublicCatalogIntent.Start) }
    }

    fun updateAppearance(systemIsDark: Boolean) { systemDark.value = systemIsDark }

    fun viewController(): UIViewController = ComposeUIViewController {
        val selected by preferences.state.collectAsState()
        val dark by systemDark.collectAsState()
        val ui = UiPreferences(theme = if (dark) Theme.Dark else Theme.Light)
        val controller = catalog
        if (controller == null) {
            CantosListeningUnavailable(preferences = ui, locale = selected.locale.toShellLocale())
        } else {
            val state by controller.state.collectAsState()
            PublicCatalogScreen(state, controller::dispatch, preferences = ui, locale = selected.locale.toShellLocale())
        }
    }

    fun close() {
        catalog?.close()
        scope.cancel()
    }
}
