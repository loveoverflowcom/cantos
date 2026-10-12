package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState

// A diagnostic desktop host for the exact shared mock app; it implements no native media.
fun main() = application {
    Window(
        onCloseRequest = ::exitApplication,
        title = "Cantos · bản mô phỏng",
        state = rememberWindowState(width = 420.dp, height = 900.dp),
    ) {
        var backEnabled by remember { mutableStateOf(false) }
        var backAction by remember { mutableStateOf<() -> Unit>({}) }
        Box(
            modifier = Modifier
                .fillMaxSize()
                .onPreviewKeyEvent { event ->
                    if (event.key == Key.Escape && event.type == KeyEventType.KeyUp && backEnabled) {
                        backAction()
                        true
                    } else {
                        false
                    }
                },
        ) {
            CantosDemoApp(
                backHandler = { enabled, onBack ->
                    SideEffect {
                        backEnabled = enabled
                        backAction = onBack
                    }
                },
            )
        }
    }
}
