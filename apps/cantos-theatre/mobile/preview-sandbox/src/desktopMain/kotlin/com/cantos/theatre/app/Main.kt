package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
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
import com.cantos.theatre.domain.membership.MembershipState
import com.cantos.theatre.app.workbench.TheatreWorkbench

// A diagnostic desktop host for the exact shared mock app; it implements no native media.
fun main() = application {
    val workbench = System.getenv("CANTOS_THEATRE_WORKBENCH") == "1"
    // Diagnostic desktop opt-in only; Android/iOS hosts always use unavailable defaults.
    val membership = remember {
        if (System.getenv("CANTOS_MEMBERSHIP_PREVIEW_FIXTURE") == "1") MembershipState.fixture()
        else MembershipState.unavailable()
    }
    Window(
        onCloseRequest = ::exitApplication,
        title = "Cantos · bản mô phỏng",
        state = rememberWindowState(width = if (workbench) 1200.dp else 420.dp, height = if (workbench) 1000.dp else 900.dp),
    ) {
        if (workbench) {
            TheatreWorkbench()
            return@Window
        }
        val scope = rememberCoroutineScope()
        val container = remember { mockAppContainer(scope) }
        DisposableEffect(container) { onDispose { container.close() } }
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
                appContainer = container,
                membershipPreviewState = membership,
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
