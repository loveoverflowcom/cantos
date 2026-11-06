package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.presentation.model.PlaybackUiStatus
import com.cantos.theatre.presentation.model.SampleEpisodeIds
import com.cantos.theatre.presentation.model.SampleWorkIds
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.generated.ShellMessage
import com.cantos.theatre.presentation.model.sampleState
import com.cantos.theatre.presentation.model.sampleWorks
import java.io.File
import java.security.MessageDigest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** JVM production composables with explicit presentation fixtures; no device/audio claim. */
@OptIn(ExperimentalTestApi::class)
class PlayerResponsiveTest {
    @Test
    fun mini_player_long_metadata_separate_actions_and_last_content_fit_mobile_matrix() {
        for (width in listOf(320, 390)) for (scale in listOf(1f, 2f)) {
            for (theme in ShellTheme.entries) for (locale in ShellLocale.entries) for (signedIn in listOf(false, true)) {
                val height = if (width == 320) 568 else 844
                val title = if (locale == ShellLocale.Vietnamese)
                    "Ánh đèn cuối sân khấu — Người dẫn chuyện và lời hẹn chưa bao giờ được gửi"
                    else "The light at the end of the stage — A storyteller and the promise that was never sent"
                val works = sampleWorks.map { work -> work.copy(episodes = work.episodes.map {
                    if (it.id == SampleEpisodeIds.SonHa01) it.copy(title = title) else it
                }) }
                val events = mutableListOf<ShellEvent>()
                runDesktopComposeUiTest(width = width, height = height) {
                    setContent {
                        Box(Modifier.requiredSize(width.dp, height.dp).testTag("player-matrix")) {
                            CantosPresentation(sampleState(stack = listOf(ShellRoute.Work(SampleWorkIds.SonHa)),
                                selectedEpisode = SampleEpisodeIds.SonHa01, theme = theme, locale = locale),
                                ShellCatalogView(works), { events += it }, textScale = scale, accountSignedIn = signedIn)
                        }
                    }
                    val open = onNodeWithTag("mini-open").assertIsDisplayed().fetchSemanticsNode()
                    val transport = onNodeWithTag("mini-transport").assertIsDisplayed().fetchSemanticsNode()
                    assertTrue(open.boundsInRoot.width >= 48f && open.boundsInRoot.height >= 48f)
                    assertTrue(transport.boundsInRoot.width >= 48f && transport.boundsInRoot.height >= 48f)
                    assertTrue(open.boundsInRoot.right <= transport.boundsInRoot.left)
                    assertEquals(listOf(ShellCopy(locale).text(ShellMessage.OpenPlayer(title))),
                        open.config[SemanticsProperties.ContentDescription])
                    val time = onNodeWithTag("mini-time", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
                    val name = onNodeWithTag("mini-title", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
                    assertTrue(name.left >= open.boundsInRoot.left && name.right <= open.boundsInRoot.right)
                    assertTrue(time.left >= open.boundsInRoot.left && time.right <= open.boundsInRoot.right)
                    assertTrue(name.bottom <= time.top)
                    // The actual shell must reserve the bar/nav height, not add per-route magic padding.
                    val tail = onNodeWithTag("episode-SonHa03").performScrollTo().assertIsDisplayed().fetchSemanticsNode().boundsInRoot
                    val mini = onNodeWithTag("mini-player").fetchSemanticsNode().boundsInRoot
                    assertTrue(tail.bottom <= mini.top, "Tail covered: $width/$scale/$theme/$locale/$signedIn")
                    if (signedIn) {
                        val nav = onNodeWithTag("nav-library").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
                        assertTrue(mini.bottom <= nav.top && nav.bottom <= height)
                    } else onNodeWithTag("nav-library").assertDoesNotExist()
                    if ((signedIn && locale == ShellLocale.Vietnamese) || (!signedIn && locale == ShellLocale.English)) {
                        val root = onNodeWithTag("player-matrix")
                        val bitmap = root.captureToImage()
                        val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
                        val name = "mini-$width-$theme-text$scale-$locale-${if (signedIn) "signed-in" else "guest"}"
                        val output = File(System.getenv("CANTOS_PLAYER_CAPTURE_DIR") ?: "build/reports/player-matrix").apply { mkdirs() }
                        File(output, "$name.png").writeBytes(bytes)
                        File(output, "$name.semantics.txt").writeText(root.printToString(maxDepth = 100))
                        val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
                        File(output, "$name.json").writeText("""{"renderer":"JVM Desktop Compose","fixture":"PlayerResponsiveTest/long-title-tail","widthDp":$width,"heightDp":$height,"textScale":$scale,"theme":"$theme","locale":"$locale","signedIn":$signedIn,"pngSha256":"$hash","nativeAcceptance":false}""")
                    }
                    assertEquals(emptyList(), events)
                    onNodeWithTag("mini-transport").performClick()
                    assertEquals(listOf<ShellEvent>(ShellEvent.ToggleSimulation), events)
                    onNodeWithTag("mini-open").performClick()
                    assertEquals(listOf<ShellEvent>(ShellEvent.ToggleSimulation, ShellEvent.Navigate(ShellRoute.Player)), events)
                }
            }
        }
    }

    @Test
    fun mini_player_keeps_actions_for_every_playback_state_and_disappears_without_a_source() {
        for (status in PlaybackUiStatus.entries) runDesktopComposeUiTest(width = 320, height = 568) {
            val events = mutableListOf<ShellEvent>()
            setContent { CantosPresentation(sampleState(playbackStatus = status), ShellCatalogView(sampleWorks), { events += it }) }
            onNodeWithTag("mini-open").assertIsDisplayed()
            onNodeWithTag("mini-transport").assertIsDisplayed().performClick()
            assertEquals(listOf<ShellEvent>(ShellEvent.ToggleSimulation), events)
        }
        runDesktopComposeUiTest(width = 320, height = 568) {
            setContent { CantosPresentation(sampleState(), ShellCatalogView(emptyList()), {}) }
            onNodeWithTag("mini-player").assertDoesNotExist()
        }
    }
}
