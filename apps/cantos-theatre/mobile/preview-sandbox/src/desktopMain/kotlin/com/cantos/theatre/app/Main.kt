package com.cantos.theatre.app

import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import com.cantos.theatre.app.workbench.TheatreWorkbench
import com.cantos.theatre.domain.membership.MembershipState
import com.cantos.theatre.app.PreviewOwners
import com.cantos.theatre.app.PreviewSession
import com.cantos.theatre.adapters.preview.previewLibraryStore
import com.cantos.theatre.app.seed

// Explicit live mode uses the same production app/controllers over real HTTP. The default
// diagnostic recipes remain offline; neither host supplies native/device acceptance.
fun main() = application {
    val workbench = System.getenv("CANTOS_THEATRE_WORKBENCH") == "1"
    val apiOrigin = System.getenv("CANTOS_THEATRE_PREVIEW_API_ORIGIN")
    val requestedLive = apiOrigin != null
    val liveOrigin = remember(apiOrigin) {
        // A fixture may never reach production because its origin env was omitted.
        if (!requestedLive) null else {
            check(!workbench) { "Live integration and mock workbench must be selected separately" }
            selectLivePreviewOrigin(apiOrigin ?: "")
        }
    }
    // Diagnostic desktop opt-in only; Android/iOS hosts never receive a Membership fixture.
    val membership = remember {
        if (System.getenv("CANTOS_MEMBERSHIP_PREVIEW_FIXTURE") == "1") MembershipState.fixture()
        else MembershipState.unavailable()
    }
    val keys = remember { PreviewBackKeys() }
    Window(
        onCloseRequest = ::exitApplication,
        onKeyEvent = keys::onKeyEvent,
        title = "Cantos · xem thử trên máy tính",
        state = rememberWindowState(width = if (workbench) 1200.dp else 420.dp, height = if (workbench) 1000.dp else 900.dp),
    ) {
        if (requestedLive) {
            when (val selected = liveOrigin) {
                is HostOrigin.Ready -> LiveEnrollmentPreview(selected.origin)
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
        PreviewDesktopHost(owners, keys, Modifier.fillMaxSize())
    }
}
