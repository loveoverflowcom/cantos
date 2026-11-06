package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.adapters.preview.PreviewEpisodeIds
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import java.io.File
import java.security.MessageDigest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Production route → shell → single controller → decoder stand-in; never native audio evidence. */
@OptIn(ExperimentalTestApi::class)
class PlayerSeekFlowTest {
    @Test
    fun a_drag_dispatches_one_seek_and_the_engine_ack_preserves_paused_or_playing_intent() {
        for (playing in listOf(false, true)) {
            val width = if (playing) 320 else 390
            val height = if (playing) 568 else 844
            val scale = if (playing) 2f else 1f
            val theme = if (playing) ShellTheme.Dark else ShellTheme.Light
            lateinit var owners: PreviewOwners
            runDesktopComposeUiTest(width = width, height = height) {
                setContent {
                    Box(Modifier.requiredSize(width.dp, height.dp).testTag("seek-flow-root")) {
                        PreviewTestRoot(PreviewSeed(previewState(stack = listOf(ShellRoute.Player),
                            selectedEpisode = PreviewEpisodeIds.SonHa03, positionMs = 768_000,
                            theme = theme, transportRequested = playing)), textScale = scale, onOwners = { owners = it })
                    }
                }
                waitUntil(timeoutMillis = 5_000) {
                    owners.runtime.listening.playback.state.value.let {
                        it.session?.positionMs == 768_000L && it.status == if (playing) PlaybackStatus.Playing else PlaybackStatus.Paused
                    }
                }
                val slider = onNodeWithTag("seek").performScrollTo()
                slider.performSemanticsAction(SemanticsActions.RequestFocus) { it() }
                val before = owners.runtime.listening.playback.state.value
                val commands = owners.engine.commands
                capture("flow-$width-$theme-text$scale-initial")
                slider.performTouchInput {
                    down(center)
                    for (step in 1..25) moveTo(Offset(width * (.5f + step * .01f), centerY))
                }
                val draft = slider.fetchSemanticsNode().config[SemanticsProperties.ProgressBarRangeInfo].current
                assertEquals(commands, owners.engine.commands, "25 drag updates must issue no decoder commands")
                assertEquals(before, owners.runtime.listening.playback.state.value)
                runOnIdle { assertTrue(owners.engine.emit(PlaybackEvent.EnginePosition(requireNotNull(before.identity), 770_000))) }
                waitUntil(timeoutMillis = 5_000) { owners.runtime.listening.playback.state.value.session?.positionMs == 770_000L }
                assertEquals(draft, slider.fetchSemanticsNode().config[SemanticsProperties.ProgressBarRangeInfo].current)
                capture("flow-$width-$theme-text$scale-draft")
                slider.performTouchInput { up() }
                val target = draft.toInt().toLong() * 1000
                waitUntil(timeoutMillis = 5_000) {
                    owners.runtime.listening.playback.state.value.session?.let { it.positionMs == target && it.pendingPositionMs == null } == true
                }
                // The existing access adapter reloads an authorized playing source; a paused seek stays paused.
                val expected = if (playing) listOf("pause@$target", "load@$target", "start@$target") else listOf("seek@$target")
                assertEquals(expected, owners.engine.commands.drop(commands.size))
                val acknowledged = owners.runtime.listening.playback.state.value
                assertEquals(before.session?.intent, acknowledged.session?.intent)
                assertEquals(if (playing) PlaybackStatus.Playing else PlaybackStatus.Paused, acknowledged.status)
                assertEquals(playing, acknowledged.session?.intent == PlaybackIntent.Play)
                assertEquals(before.generation + 1, acknowledged.generation)
                slider.assertIsFocused()
                capture("flow-$width-$theme-text$scale-ack")
                val after = owners.engine.commands
                onNodeWithTag("back").performClick()
                onNodeWithTag("mini-open").performClick()
                assertEquals(after, owners.engine.commands, "Back/mini-open must not restart the session")
                assertEquals(acknowledged, owners.runtime.listening.playback.state.value)
                // A stale visible frame can enqueue after a newer control. Check the real shell forwards its fence.
                var captured: PlaybackState? = null
                runOnIdle {
                    owners.shell.dispatch(ShellEvent.SeekTo(0, before.generation))
                    assertTrue(owners.runtime.listening.playback.captureState { value, _ -> captured = value })
                }
                waitUntil(timeoutMillis = 5_000) { captured != null }
                assertEquals(acknowledged, captured)
                assertEquals(after, owners.engine.commands)
            }
        }
    }

    private fun ComposeUiTest.capture(name: String) {
        val root = onNodeWithTag("seek-flow-root")
        val bitmap = root.captureToImage()
        val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
        val output = File(System.getenv("CANTOS_PLAYER_CAPTURE_DIR") ?: "build/reports/player-seek").apply { mkdirs() }
        File(output, "$name.png").writeBytes(bytes)
        File(output, "$name.semantics.txt").writeText(root.printToString(maxDepth = 100))
        val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
        File(output, "$name.json").writeText("""{"renderer":"JVM Desktop Compose","entrypoint":"PreviewTestRoot → CantosTheatreApp → TheatreShell → PlaybackController","fixture":"PlayerSeekFlowTest","variant":"$name","pngSha256":"$hash","nativeAcceptance":false}""")
    }
}
