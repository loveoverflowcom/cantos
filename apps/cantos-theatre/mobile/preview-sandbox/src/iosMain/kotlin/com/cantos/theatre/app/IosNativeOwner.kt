package com.cantos.theatre.app

import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.window.ComposeUIViewController
import com.cantos.theatre.adapters.preferences.IosLocalePreferenceStore
import com.cantos.theatre.features.launch.presentation.CantosLocalLaunch
import com.cantos.ui.UiPreferences
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import platform.UIKit.UIViewController

/** Explicit demo composition over the process-owned native identity boundary. */
class IosNativeOwner(apiOrigin: String?, issuer: String?, platform: IosIdentityPlatform) {
    private val identity = IosNativeIdentityOwner(apiOrigin, issuer, platform)
    val availability get() = identity.availability
    private val guest = if (identity.controller == null) IosDemoOwner() else null
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    // The container borrows the account owner; its locale store and other demo owners are retained.
    private val container = identity.controller?.let { account ->
        mockAppContainer(scope, account = account, localePreferenceStore = IosLocalePreferenceStore())
    }

    fun viewController(): UIViewController {
        guest?.let { return it.viewController() }
        val appContainer = checkNotNull(container)
        return ComposeUIViewController {
            val preferences by appContainer.preferences.state.collectAsState()
            CantosLocalLaunch(localReady = true, preferences = UiPreferences(), locale = preferences.locale.toShellLocale()) {
                CantosDemoApp(appContainer = appContainer)
            }
        }
    }

    fun callback(state: String, code: String, hasError: Boolean) = identity.callback(state, code, hasError)
    fun browserCancelled() = identity.browserCancelled()
    fun close() {
        container?.close()
        guest?.close()
        scope.cancel()
        identity.close()
    }
}
