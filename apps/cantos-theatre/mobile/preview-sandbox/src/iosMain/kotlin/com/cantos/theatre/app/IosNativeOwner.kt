package com.cantos.theatre.app

import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import com.cantos.theatre.adapters.preferences.IosLocalePreferenceStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import androidx.compose.ui.window.ComposeUIViewController
import com.cantos.theatre.features.launch.presentation.CantosLocalLaunch
import com.cantos.ui.UiPreferences
import platform.UIKit.UIViewController

/** Explicit demo composition over the process-owned native identity boundary. */
class IosNativeOwner(apiOrigin: String, issuer: String, platform: IosIdentityPlatform) {
    private val identity = IosNativeIdentityOwner(apiOrigin, issuer, platform)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val container = mockAppContainer(scope, account = identity.controller, localePreferenceStore = IosLocalePreferenceStore())
    fun viewController(): UIViewController = ComposeUIViewController {
        val preferences by container.preferences.state.collectAsState()
        CantosLocalLaunch(localReady = true, preferences = UiPreferences(), locale = preferences.locale.toShellLocale()) {
            CantosDemoApp(appContainer = container)
        }
    }
    fun callback(state: String, code: String, hasError: Boolean) = identity.callback(state, code, hasError)
    fun browserCancelled() = identity.browserCancelled()
    fun close() {
        container.close()
        scope.cancel()
        identity.close()
    }
}
