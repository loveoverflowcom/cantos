package com.cantos.theatre.app

import androidx.compose.ui.window.ComposeUIViewController
import com.cantos.theatre.features.launch.presentation.CantosLocalLaunch
import com.cantos.ui.UiPreferences
import platform.UIKit.UIViewController

/** Explicit demo composition over the process-owned native identity boundary. */
class IosNativeOwner(apiOrigin: String, issuer: String, platform: IosIdentityPlatform) {
    private val identity = IosNativeIdentityOwner(apiOrigin, issuer, platform)
    fun viewController(): UIViewController = ComposeUIViewController {
        CantosLocalLaunch(localReady = true, preferences = UiPreferences()) {
            CantosDemoApp(account = identity.controller)
        }
    }
    fun callback(state: String, code: String, hasError: Boolean) = identity.callback(state, code, hasError)
    fun browserCancelled() = identity.browserCancelled()
    fun close() = identity.close()
}
