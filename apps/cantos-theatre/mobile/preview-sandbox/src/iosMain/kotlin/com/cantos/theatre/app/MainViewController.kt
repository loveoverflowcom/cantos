package com.cantos.theatre.app

import androidx.compose.ui.window.ComposeUIViewController
import platform.UIKit.UIViewController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import com.cantos.theatre.application.preferences.PreferenceTheme
import platform.UIKit.UIScreen
import platform.UIKit.UIUserInterfaceStyle
import com.cantos.theatre.features.launch.presentation.CantosLocalLaunch
import com.cantos.ui.UiPreferences

/** SwiftUI's application owner shares one mock controller across mounted view controllers. */
class IosDemoOwner {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val container = mockAppContainer(scope).also {
        val dark = UIScreen.mainScreen.traitCollection.userInterfaceStyle == UIUserInterfaceStyle.UIUserInterfaceStyleDark
        it.preferences.setTheme(if (dark) PreferenceTheme.Dark else PreferenceTheme.Light)
    }

    fun viewController(): UIViewController = ComposeUIViewController {
        // The retained local container already exists; skip the optional branded frame.
        CantosLocalLaunch(localReady = true, preferences = UiPreferences()) {
            CantosDemoApp(appContainer = container)
        }
    }

    fun close() {
        container.close()
        scope.cancel()
    }
}

// Navigation has visible shared Back/Close actions. No audio session or network adapter exists.
fun MainViewController(): UIViewController = ComposeUIViewController {
    CantosDemoApp()
}
