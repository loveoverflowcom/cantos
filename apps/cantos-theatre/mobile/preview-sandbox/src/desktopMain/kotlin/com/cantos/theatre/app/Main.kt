package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.SideEffect
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
import com.cantos.theatre.app.workbench.TheatreWorkbench
import com.cantos.theatre.domain.membership.MembershipState
import com.cantos.theatre.app.PreviewOwners
import com.cantos.theatre.app.PreviewSession
import com.cantos.theatre.app.PreviewTheatreApp
import com.cantos.theatre.adapters.preview.previewLibraryStore
import com.cantos.theatre.app.seed
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeIdentityAvailability

// Explicit live mode uses the same production app/controllers over real HTTP. The default
// diagnostic recipes remain offline; neither host supplies native/device acceptance.
fun main() = application {
    val workbench = System.getenv("CANTOS_THEATRE_WORKBENCH") == "1"
    val apiOrigin = System.getenv("CANTOS_THEATRE_PREVIEW_API_ORIGIN")
    val issuer = System.getenv("CANTOS_THEATRE_PREVIEW_NATIVE_ISSUER")
    val requestedLive = apiOrigin != null || issuer != null
    val liveOrigin = remember(apiOrigin, issuer) {
        // A fixture may never reach production because its origin env was omitted.
        if (!requestedLive) null else {
            check(!workbench) { "Live integration and mock workbench must be selected separately" }
            hostOrigin(com.cantos.theatre.adapters.http.selectCatalogOrigin(apiOrigin ?: "", allowLoopbackHttp = true))
        }
    }
    val identity = remember(liveOrigin, issuer) {
        (liveOrigin as? HostOrigin.Ready)?.let { NativeClientConfiguration.resolve(it.origin.toString(), issuer) }
    }
    // Diagnostic desktop opt-in only; Android/iOS hosts never receive a Membership fixture.
    val membership = remember {
        if (System.getenv("CANTOS_MEMBERSHIP_PREVIEW_FIXTURE") == "1") MembershipState.fixture()
        else MembershipState.unavailable()
    }
    Window(
        onCloseRequest = ::exitApplication,
        title = "Cantos · xem thử trên máy tính",
        state = rememberWindowState(width = if (workbench) 1200.dp else 420.dp, height = if (workbench) 1000.dp else 900.dp),
    ) {
        if (requestedLive) {
            when (val selected = liveOrigin) {
                is HostOrigin.Ready -> {
                    val available = identity as? NativeIdentityAvailability.Available
                    if (available != null) LiveEnrollmentPreview(available.configuration)
                    else LivePublicCatalogPreview(selected.origin, identity)
                }
                is HostOrigin.Refused -> com.cantos.theatre.features.launch.presentation.CantosListeningUnavailable(com.cantos.ui.UiPreferences(), fault = selected.fault)
                null -> Unit
            }
            return@Window
        }
        if (workbench) {
            TheatreWorkbench()
            return@Window
        }
        val scope = rememberCoroutineScope()
        val owners = remember { PreviewOwners(scope, PreviewSession.Guest.seed().copy(membership = membership), previewLibraryStore()) }
        DisposableEffect(owners) { onDispose { owners.close() } }
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
            PreviewTheatreApp(
                owners,
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
