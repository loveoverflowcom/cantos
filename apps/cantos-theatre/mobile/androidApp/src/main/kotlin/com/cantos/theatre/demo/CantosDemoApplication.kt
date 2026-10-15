package com.cantos.theatre.demo

import android.app.Application
import android.content.pm.PackageManager
import android.content.res.Configuration
import com.cantos.theatre.adapters.http.NativeIdentityClient
import com.cantos.theatre.adapters.identity.AndroidCredentialStore
import com.cantos.theatre.adapters.identity.AndroidIdentityCrypto
import com.cantos.theatre.adapters.identity.AndroidLogoutBarrier
import com.cantos.theatre.adapters.identity.AndroidNativeHttp
import com.cantos.theatre.adapters.identity.AndroidSystemBrowser
import com.cantos.theatre.app.AppContainer
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel

/** Process-lifetime account/mock owners, retained when an Activity or route is recreated. */
class CantosDemoApplication : Application() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    val container: AppContainer by lazy {
        val dark = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
        AppContainer.mock(scope).also {
            it.preferences.setTheme(if (dark) PreferenceTheme.Dark else PreferenceTheme.Light)
        }
    }
    val nativeIdentity: NativeAccountController? by lazy {
        val metadata = packageManager.getApplicationInfo(packageName, PackageManager.GET_META_DATA).metaData
        val api = metadata?.getString("CantosApiOrigin")
        val issuer = metadata?.getString("CantosNativeIssuer")
        val configuration = if (api != null && issuer != null) NativeClientConfiguration.create(api, issuer) else null
        configuration?.let {
            NativeAccountController(
                scope = scope,
                configuration = it,
                api = NativeIdentityClient(it, AndroidNativeHttp(scope, it.apiOrigin)),
                crypto = AndroidIdentityCrypto(),
                browser = AndroidSystemBrowser(this),
                store = AndroidCredentialStore(this),
                barrier = AndroidLogoutBarrier(this),
                now = { System.currentTimeMillis() / 1000 },
            )
        }
    }

    override fun onTerminate() {
        // Emulator shutdown only; ordinary process death loses all mock memory.
        container.close()
        nativeIdentity?.close()
        scope.cancel()
        super.onTerminate()
    }
}
