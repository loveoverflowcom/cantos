package com.cantos.theatre.app

import android.Manifest
import android.content.Context
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import androidx.activity.ComponentActivity
import com.cantos.theatre.R
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.JvmCatalogTransport
import com.cantos.theatre.adapters.http.NativeAuthorizedHttpPort
import com.cantos.theatre.adapters.http.PlaybackAccessHttp
import com.cantos.theatre.adapters.http.PublicCatalogHttpAdapter
import com.cantos.theatre.adapters.http.selectPackagedApiOrigin
import com.cantos.theatre.adapters.identity.AndroidNativeHttp
import com.cantos.theatre.adapters.identity.createAndroidNativeAccount
import com.cantos.theatre.adapters.identity.createAndroidRegistration
import com.cantos.theatre.adapters.library.AccountCatalogCache
import com.cantos.theatre.adapters.library.NativeLibraryClient
import com.cantos.theatre.adapters.library.androidLibraryStore
import com.cantos.theatre.adapters.playback.AndroidMediaEngine
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.adapters.library.nativeLibraryController
import com.cantos.theatre.adapters.preferences.AndroidLocalePreferenceStore
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.application.preferences.PreferencesState
import java.util.UUID
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

/**
 * The host's origin: the packaged explicit override, else the production default (#298). An
 * invalid override or a missing network declaration is a typed refusal shown before any request;
 * it never selects another environment and never composes a mock.
 */
internal fun ComponentActivity.liveCatalogOrigin(): HostOrigin {
    val raw = nativeMetadata("CantosApiOrigin")
    val debuggable = applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
    val allowLoopback = debuggable && resources.getBoolean(R.bool.cantos_loopback_origin_allowed)
    val networkDeclared = checkSelfPermission(Manifest.permission.INTERNET) == PackageManager.PERMISSION_GRANTED
    return hostOrigin(selectPackagedApiOrigin(raw, allowLoopback), networkDeclared)
}

private fun Context.nativeMetadata(key: String): String? = try {
    packageManager.getApplicationInfo(packageName, PackageManager.GET_META_DATA).metaData?.getString(key)
} catch (_: PackageManager.NameNotFoundException) { null }

/** Application-context singleton: Activity/navigation recreation never constructs another player. */
internal class AndroidProductionOwner(context: Context, val origin: ApiOrigin, systemDark: Boolean) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val app = context.applicationContext
    private val http = AndroidNativeHttp(scope, origin)
    private val store = try { androidLibraryStore(app) }
        catch (_: android.database.sqlite.SQLiteException) { null }
        catch (_: SecurityException) { null }
    private var accountScope: String? = null
    val cache = store?.let { AccountCatalogCache(it) { accountScope } }
    val catalog = PublicCatalogController(scope, PublicCatalogHttpAdapter(origin, JvmCatalogTransport(), cache), { cache?.invalidate() })
        .also { it.dispatch(PublicCatalogIntent.Start) }
    private val account = createAndroidNativeAccount(scope, app, origin)
    private val authorizedHttp = NativeAuthorizedHttpPort(scope, origin, account, http)
    private val library = store?.let { nativeLibraryController(scope, NativeLibraryClient(origin, authorizedHttp), it,
        { UUID.randomUUID().toString() }, onSessionInvalidated = { expected -> account.revalidate(expected) },
        ownsAuthorization = { owner -> account.belongsToCurrent(owner.bearer, owner.session) &&
            account.verifiedSession.value?.session?.actor == owner.account },
        nowMs = { System.currentTimeMillis() }) }
    /** Language is stored; the theme starts from the system and is then the listener's in-app choice. */
    val preferences = PreferencesController(
        PreferencesState(theme = if (systemDark) PreferenceTheme.Dark else PreferenceTheme.Light),
        AndroidLocalePreferenceStore(app),
    )
    var systemDark: Boolean = systemDark
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
    // One registration for this process, independent of Activity/navigation recreation.
    private val networkSignals = AndroidLibraryNetworkSignals(app, scope, runtime::networkRestored)
    /** Navigation and typed route text outlive Activity recreation, like every other owner here. */
    private val registration = createAndroidRegistration(scope, origin)
    val shell = TheatreShell(scope, catalog, runtime, preferences, registration = registration)
}

internal object ProductionProcess {
    private var owner: AndroidProductionOwner? = null
    fun obtain(context: Context, origin: ApiOrigin, systemDark: Boolean): AndroidProductionOwner {
        val existing = owner
        if (existing != null) {
            check(existing.origin == origin) { "Process API environment cannot change" }
            // A system theme change recreates the Activity; the user's in-app choice is kept otherwise.
            if (existing.systemDark != systemDark) {
                existing.systemDark = systemDark
                existing.preferences.setTheme(if (systemDark) PreferenceTheme.Dark else PreferenceTheme.Light)
            }
            return existing
        }
        return AndroidProductionOwner(context, origin, systemDark).also { owner = it }
    }
    fun current(): AndroidProductionOwner? = owner
}

/** The process owner for [origin]; the Activity composes [CantosTheatreApp] over its shell. */
internal fun ComponentActivity.theatreOwner(origin: ApiOrigin, systemDark: Boolean): AndroidProductionOwner =
    ProductionProcess.obtain(applicationContext, origin, systemDark)
