package com.cantos.theatre.app

import android.Manifest
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import com.cantos.theatre.R
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.CatalogOriginSelection
import com.cantos.theatre.adapters.http.JvmCatalogTransport
import com.cantos.theatre.adapters.http.PublicCatalogHttpAdapter
import com.cantos.theatre.adapters.http.selectCatalogOrigin
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.features.catalog.presentation.PublicCatalogScreen
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.UiPreferences
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel

/*
 * Live public catalog composition for the production host.
 *
 * Origin policy: a non-debuggable (release) app admits only `https://`; a debuggable build whose
 * debug source set sets `cantos_loopback_origin_allowed` also admits explicit loopback `http://`
 * (with the debug-only network security config). Main declares INTERNET and the explicit
 * `CantosApiOrigin` for both variants; absent/rejected origins render the unavailable state.
 * No default origin or demo fallback exists.
 */

/** The admitted origin, or `null` (not configured, rejected, or no network permission). */
internal fun ComponentActivity.liveCatalogOrigin(): ApiOrigin? {
    val raw = try {
        packageManager.getApplicationInfo(packageName, PackageManager.GET_META_DATA).metaData?.getString("CantosApiOrigin")
    } catch (_: PackageManager.NameNotFoundException) {
        null
    }
    val debuggable = applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
    val allowLoopback = debuggable && resources.getBoolean(R.bool.cantos_loopback_origin_allowed)
    val networkDeclared = checkSelfPermission(Manifest.permission.INTERNET) == PackageManager.PERMISSION_GRANTED
    return when (val selection = selectCatalogOrigin(raw, allowLoopback)) {
        is CatalogOriginSelection.Configured -> selection.origin.takeIf { networkDeclared }
        // A rejected value fails closed to the explicit state; it never falls back elsewhere.
        CatalogOriginSelection.NotConfigured, CatalogOriginSelection.Rejected -> null
    }
}

internal fun ComponentActivity.setLiveCatalogContent(origin: ApiOrigin, preferences: UiPreferences, locale: ShellLocale) {
    // The owner survives configuration changes and is closed with this Activity's store.
    val catalog = ViewModelProvider(this, LiveCatalogOwner.Factory(origin))[LiveCatalogOwner::class.java].controller
    setContent {
        val state by catalog.state.collectAsState()
        PublicCatalogScreen(
            state = state,
            onIntent = catalog::dispatch,
            preferences = preferences,
            locale = locale,
            backHandler = { enabled, onBack -> BackHandler(enabled = enabled, onBack = onBack) },
        )
    }
}

/** Owner of one live catalog controller and its sequential scope, across recreation. */
internal class LiveCatalogOwner(origin: ApiOrigin) : ViewModel() {
    @OptIn(ExperimentalCoroutinesApi::class)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default.limitedParallelism(1))
    val controller = PublicCatalogController(scope, PublicCatalogHttpAdapter(origin, JvmCatalogTransport()))
        .also { it.dispatch(PublicCatalogIntent.Start) }

    override fun onCleared() {
        controller.close()
        scope.cancel()
    }

    class Factory(private val origin: ApiOrigin) : ViewModelProvider.Factory {
        @Suppress("UNCHECKED_CAST")
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            check(modelClass == LiveCatalogOwner::class.java) { "Unknown owner ${modelClass.name}" }
            return LiveCatalogOwner(origin) as T
        }
    }
}
