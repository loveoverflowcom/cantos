package com.cantos.theatre.app

import androidx.compose.ui.window.ComposeUIViewController
import platform.UIKit.UIViewController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel

/** SwiftUI's application owner shares one mock controller across mounted view controllers. */
class IosDemoOwner {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val container = AppContainer.mock(scope)

    fun viewController(): UIViewController = ComposeUIViewController {
        CantosDemoApp(appContainer = container)
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
