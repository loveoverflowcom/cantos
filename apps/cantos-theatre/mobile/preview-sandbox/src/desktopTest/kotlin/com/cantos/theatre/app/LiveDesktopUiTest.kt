package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * The opt-in live Desktop Preview window body, [LiveEnrollmentPreview], driven through its own product controls
 * by semantics only (no pixels). The server is the loopback protocol fixture, so this proves the live window
 * wiring: the same Register/Login/Library screens, a real v1 login over HTTP, and the Library read from the
 * server with the session stamp. Real Server acceptance is the Rust harness lane; audio stays unavailable.
 */
@OptIn(ExperimentalTestApi::class)
class LiveDesktopUiTest {
    private val actor = "listener_a"
    private val family = "11111111-2222-4333-8444-555555555555"
    private val work = "10000000-0000-4000-8000-000000000001"

    @Test fun the_live_window_logs_in_through_its_screens_and_shows_the_server_read_library() {
        val server = LoopbackServer(actor, family, work, "ci1_n_" + "a".repeat(64), "ci1_r_" + "b".repeat(64))
        try {
            runDesktopComposeUiTest(width = 390, height = 844) {
                setContent { Box(Modifier.requiredSize(390.dp, 844.dp)) { LiveEnrollmentPreview(server.origin) } }
                waitUntil(timeoutMillis = 5_000) { onAllNodesWithTag("register-email").fetchSemanticsNodes().isNotEmpty() }
                // The live window opens on Register; header Back returns to Login without submitting.
                onNodeWithTag("back").performClick()
                waitUntil(timeoutMillis = 5_000) { onAllNodesWithTag("login-email").fetchSemanticsNodes().isNotEmpty() }
                onNodeWithTag("login-email").performTextInput("listener@example.test")
                onNodeWithTag("login-password").performTextInput("synthetic-test-password")
                onNodeWithTag("login-primary").performScrollTo().performClick()
                waitUntil(timeoutMillis = 10_000) { server.seen.any { it.route.startsWith("/library/saved?") } }
                waitUntil(timeoutMillis = 5_000) { onAllNodesWithTag("nav-library").fetchSemanticsNodes().isNotEmpty() }
                onNodeWithTag("nav-library").performClick()
                onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Của tôi"))
                val reads = server.seen.filter { it.route.startsWith("/library/") }
                assertEquals(setOf("continue", "saved", "history", "bookmarks"), reads.map { it.route.removePrefix("/library/").substringBefore('?') }.toSet())
                assertTrue(reads.all { it.authorization?.startsWith("Bearer ci1_n_") == true && it.session == family })
                assertTrue(server.seen.none { !it.cookie.isNullOrEmpty() })
            }
        } finally { server.stop() }
    }
}
