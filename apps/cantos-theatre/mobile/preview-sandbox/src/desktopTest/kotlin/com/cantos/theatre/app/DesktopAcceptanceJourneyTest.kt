package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.adapters.preview.PreviewEpisodeIds
import com.cantos.theatre.adapters.preview.PreviewWorkIds
import com.cantos.theatre.adapters.preview.previewLibraryStore
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.domain.playback.PlaybackStatus
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Desktop Preview acceptance through semantics only: no window, pixel or screenshot is read. Each journey
 * drives the real product controls on the shared application, mounted by the same [PreviewDesktopHost] the
 * window uses, and asserts the owners' state. The decoder stand-in acknowledges commands and produces no audio;
 * this is mocked-port evidence, never native audio, OS input, IME or screen-reader acceptance.
 */
@OptIn(ExperimentalTestApi::class)
class DesktopAcceptanceJourneyTest {
    private val sonHa = PreviewWorkIds.SonHa.value
    private val episode = PreviewEpisodeIds.SonHa03.value

    private lateinit var keys: PreviewBackKeys

    private fun run(
        seed: PreviewSeed = PreviewSession.Guest.seed(),
        width: Int = 390,
        height: Int = 844,
        body: ComposeUiTest.(PreviewOwners) -> Unit,
    ) = runDesktopComposeUiTest(width = width, height = height) {
        lateinit var owners: PreviewOwners
        keys = PreviewBackKeys()
        setContent {
            val scope = rememberCoroutineScope()
            val mounted = remember(seed) { PreviewOwners(scope, seed, previewLibraryStore()).also { owners = it } }
            DisposableEffect(mounted) { onDispose { mounted.close() } }
            Box(Modifier.requiredSize(width.dp, height.dp)) { PreviewDesktopHost(mounted, keys) }
        }
        body(owners)
    }

    private fun ComposeUiTest.untilTag(tag: String) = waitUntil(timeoutMillis = 5_000) {
        onAllNodesWithTag(tag).fetchSemanticsNodes().isNotEmpty()
    }

    private fun ComposeUiTest.title(expected: String) =
        onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, expected))

    private fun ComposeUiTest.openPlayer() {
        untilTag("work-$sonHa")
        onNodeWithTag("work-$sonHa").performScrollTo().performClick()
        untilTag("episode-$episode")
        onNodeWithTag("episode-$episode").performScrollTo().performClick()
        title("Phòng nghe")
    }

    @Test fun player_controls_keep_one_owner_across_queue_library_and_the_mini_player() = run { owners ->
        openPlayer()
        waitUntil(timeoutMillis = 5_000) { owners.runtime.listening.playback.state.value.status == PlaybackStatus.Playing }
        val session = owners.runtime.listening.playback.state.value.session?.source
        onNodeWithTag("transport").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { !owners.projection().transportRequested }
        onNodeWithTag("forward").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.projection().positionMs == 15_000L }
        onNodeWithTag("elapsed").assertTextEquals("0:15")
        onNodeWithTag("rewind").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.projection().positionMs == 0L }
        onNodeWithTag("forward").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.projection().positionMs == 15_000L }
        onNodeWithTag("queue").performScrollTo().performClick()
        assertEquals(TheatreRoute.Queue, owners.shell.navigation.value.route)
        onNodeWithTag("back").performClick()
        title("Phòng nghe")
        onNodeWithTag("back").performClick()
        onNodeWithTag("mini-player").assertExists()
        onNodeWithTag("nav-library").performClick()
        title("Của tôi")
        onNodeWithTag("mini-player").assertExists()
        assertEquals(session, owners.runtime.listening.playback.state.value.session?.source, "navigation never replaced the player")
        assertEquals(15_000L, owners.projection().positionMs, "the paused position survived every route change")
        onNodeWithTag("mini-transport").performClick()
        waitUntil(timeoutMillis = 5_000) { owners.projection().transportRequested }
    }

    @Test fun the_timer_dialog_opens_and_closing_it_restores_focus_to_its_opener() = run { owners ->
        openPlayer()
        onNodeWithTag("player-timer").performScrollTo().performClick()
        untilTag("preference-dialog")
        onNodeWithTag("preference-close").performClick()
        waitUntil(timeoutMillis = 5_000) { onAllNodesWithTag("preference-dialog").fetchSemanticsNodes().isEmpty() }
        onNodeWithTag("player-timer").assertIsFocused()
        assertEquals(TheatreRoute.Player, owners.shell.navigation.value.route)
    }

    @OptIn(androidx.compose.ui.InternalComposeUiApi::class)
    private fun key(key: androidx.compose.ui.input.key.Key = androidx.compose.ui.input.key.Key.Escape,
                    type: androidx.compose.ui.input.key.KeyEventType = androidx.compose.ui.input.key.KeyEventType.KeyUp) =
        androidx.compose.ui.input.key.KeyEvent(key, type)

    @Test fun the_back_key_is_the_shell_back_only_below_the_root_and_only_on_escape_release() = run { owners ->
        untilTag("work-$sonHa")
        // The key predicate and its hookup to the shell's Back. Delivery of the key to it, whatever holds focus,
        // is DesktopWindowEscapeTest (real window, explicit).
        assertTrue(!keys.onKeyEvent(key()), "at the root there is nothing to go back to")
        onNodeWithTag("work-$sonHa").performScrollTo().performClick()
        untilTag("episode-$episode")
        title("Tác phẩm")
        assertTrue(!keys.onKeyEvent(key(type = androidx.compose.ui.input.key.KeyEventType.KeyDown)), "only the release acts")
        assertTrue(!keys.onKeyEvent(key(androidx.compose.ui.input.key.Key.Enter)), "only Escape acts")
        assertTrue(keys.onKeyEvent(key()))
        waitForIdle()
        title("Khám phá")
        assertEquals(TheatreRoute.Discovery, owners.shell.navigation.value.route)
        assertTrue(!keys.onKeyEvent(key()))
    }

    @Test fun a_signed_in_logout_clears_the_library_and_the_next_listener_starts_without_it_on_screen() =
        run(PreviewSession.Restored.seed()) { owners ->
            waitUntil(timeoutMillis = 5_000) { owners.account.state.value is NativeAccountState.SignedIn }
            onNodeWithTag("nav-library").performClick()
            title("Của tôi")
            assertTrue(owners.shell.library.value.saved.isNotEmpty())
            onNodeWithTag("account").performClick()
            onNodeWithTag("logout").performScrollTo().performClick()
            waitUntil(timeoutMillis = 5_000) { owners.account.state.value == NativeAccountState.SignedOut }
            assertEquals(null, owners.shell.library.value.account)
            assertTrue(owners.shell.library.value.saved.isEmpty(), "no previous listener row remains in the Library owner")
            onNodeWithTag("back").performClick()
            title("Của tôi")
            onNodeWithTag("library-saved").assertExists()
            assertTrue(owners.projection().savedWorks.isEmpty())
        }

}
