package com.cantos.theatre.app

import androidx.compose.ui.window.ComposeUIViewController
import platform.UIKit.UIViewController

// Navigation has visible shared Back/Close actions. No audio session or network adapter exists.
fun MainViewController(): UIViewController = ComposeUIViewController {
    CantosDemoApp()
}
