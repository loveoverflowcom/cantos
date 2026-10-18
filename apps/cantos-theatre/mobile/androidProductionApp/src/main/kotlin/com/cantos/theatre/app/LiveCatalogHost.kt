package com.cantos.theatre.app

import android.Manifest
import android.content.Context
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import com.cantos.theatre.R
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.CatalogOriginSelection
import com.cantos.theatre.adapters.http.JvmCatalogTransport
import com.cantos.theatre.adapters.http.PlaybackAccessHttp
import com.cantos.theatre.adapters.http.PublicCatalogHttpAdapter
import com.cantos.theatre.adapters.http.selectCatalogOrigin
import com.cantos.theatre.adapters.identity.AndroidNativeHttp
import com.cantos.theatre.adapters.identity.createAndroidNativeAccount
import com.cantos.theatre.adapters.library.AccountCatalogCache
import com.cantos.theatre.adapters.library.NativeLibraryClient
import com.cantos.theatre.adapters.library.androidLibraryStore
import com.cantos.theatre.adapters.playback.AndroidMediaEngine
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.adapters.library.nativeLibraryController
import com.cantos.theatre.app.NativeCatalogScreen
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.UiPreferences
import java.util.UUID
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

internal fun ComponentActivity.liveCatalogOrigin(): ApiOrigin? {
    val raw = nativeMetadata("CantosApiOrigin")
    val debuggable = applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
    val allowLoopback = debuggable && resources.getBoolean(R.bool.cantos_loopback_origin_allowed)
    val networkDeclared = checkSelfPermission(Manifest.permission.INTERNET) == PackageManager.PERMISSION_GRANTED
    return when (val selection = selectCatalogOrigin(raw, allowLoopback)) {
        is CatalogOriginSelection.Configured -> selection.origin.takeIf { networkDeclared }
        CatalogOriginSelection.NotConfigured, CatalogOriginSelection.Rejected -> null
    }
}

private fun Context.nativeMetadata(key: String): String? = try {
    packageManager.getApplicationInfo(packageName, PackageManager.GET_META_DATA).metaData?.getString(key)
} catch (_: PackageManager.NameNotFoundException) { null }

/** Application-context singleton: Activity/navigation recreation never constructs another player. */
internal class AndroidProductionOwner(context: Context, val origin: ApiOrigin) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val app = context.applicationContext
    private val http = AndroidNativeHttp(scope, origin.toString())
    private val store = try { androidLibraryStore(app) }
        catch (_: android.database.sqlite.SQLiteException) { null }
        catch (_: SecurityException) { null }
    private var accountScope: String? = null
    val cache = store?.let { AccountCatalogCache(it) { accountScope } }
    val catalog = PublicCatalogController(scope, PublicCatalogHttpAdapter(origin, JvmCatalogTransport(), cache), { cache?.invalidate() })
        .also { it.dispatch(PublicCatalogIntent.Start) }
    private val account = createAndroidNativeAccount(scope, app, origin.toString(), app.nativeMetadata("CantosNativeIssuer"))
    private val library = store?.let { nativeLibraryController(scope, NativeLibraryClient(origin, http), it,
        { UUID.randomUUID().toString() }, onSessionInvalidated = { expected -> account?.revalidate(expected) },
        nowMs = { System.currentTimeMillis() }) }
    val runtime = NativeSessionRuntime(scope,
        nativeFactory = { onControl -> AndroidMediaEngine(app, onControl) },
        access = PlaybackAccessHttp(origin, http) { System.currentTimeMillis() },
        account = account,
        library = library, newUuid = { UUID.randomUUID().toString() }, nowMs = { System.currentTimeMillis() },
        onAccountChanged = { actor ->
            if (actor != accountScope) {
                accountScope = actor
                catalog.dispatch(PublicCatalogIntent.AccountChanged)
            }
        })
}

internal object ProductionProcess {
    private var owner: AndroidProductionOwner? = null
    fun obtain(context: Context, origin: ApiOrigin): AndroidProductionOwner {
        val existing = owner
        if (existing != null) {
            check(existing.origin == origin) { "Process API environment cannot change" }
            return existing
        }
        return AndroidProductionOwner(context, origin).also { owner = it }
    }
    fun current(): AndroidProductionOwner? = owner
}

internal fun ComponentActivity.setLiveCatalogContent(origin: ApiOrigin, preferences: UiPreferences, locale: ShellLocale) {
    val owner = ProductionProcess.obtain(applicationContext, origin)
    setContent {
        NativeCatalogScreen(owner.catalog, owner.runtime, preferences, locale,
            backHandler = { enabled, onBack -> BackHandler(enabled = enabled, onBack = onBack) }, cache = owner.cache)
    }
}
