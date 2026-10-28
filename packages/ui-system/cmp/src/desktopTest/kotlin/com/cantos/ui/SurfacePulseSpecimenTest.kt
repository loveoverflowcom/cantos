package com.cantos.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.ui.preview.ComponentExample
import com.cantos.ui.preview.PulseRegionLoadingName
import com.cantos.ui.preview.PulseRegionReadyName
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Issue #585 SurfacePulse specimens, JVM Desktop Compose over a manual clock only: the region is
 * named once while loading and its placeholders are never read; the reveal keeps the specimen's
 * geometry at large text; reduced motion reveals at once. Not Android/iOS runtime or TalkBack.
 */
@OptIn(ExperimentalTestApi::class)
class SurfacePulseSpecimenTest {
    private val firstTitle = "Ánh đèn cuối sân khấu"

    private fun ComposeUiTest.advance(millis: Long) {
        val until = mainClock.currentTime + millis
        while (mainClock.currentTime < until) mainClock.advanceTimeByFrame()
    }

    private fun ComposeUiTest.specimen(preferences: UiPreferences, state: () -> String) = setContent {
        BundledPreviewCaptureRoot(preferences = preferences) {
            ComponentExample(PreviewExample.SurfacePulse, state(), {}, Modifier.testTag("region"))
        }
    }

    @Test
    fun the_region_is_named_once_while_loading_and_no_placeholder_or_hidden_content_is_read() {
        runDesktopComposeUiTest(width = 360, height = 640) {
            mainClock.autoAdvance = false
            specimen(UiPreferences()) { "running" }
            val config = onNodeWithTag("region").fetchSemanticsNode().config
            assertEquals(PulseRegionLoadingName, config[SemanticsProperties.ContentDescription].single())
            assertEquals(PulseRegionLoadingName, config[SemanticsProperties.StateDescription])
            assertTrue(SemanticsProperties.LiveRegion !in config, "waiting is not announced on every beat")
            onNodeWithText(firstTitle).assertDoesNotExist() // cleared in the merged tree a reader sees
        }
    }

    @Test
    fun the_revealed_content_reads_normally_without_the_loading_name() {
        runDesktopComposeUiTest(width = 360, height = 640) {
            mainClock.autoAdvance = false
            specimen(UiPreferences()) { "revealed" }
            onNodeWithText(firstTitle).assertExists()
            onNodeWithText("Midnight storytellers of the old quarter, a long unbroken title").assertExists()
            val config = onNodeWithTag("region").fetchSemanticsNode().config
            assertTrue(SemanticsProperties.StateDescription !in config)
            assertTrue(SemanticsProperties.ContentDescription !in config || PulseRegionLoadingName !in config[SemanticsProperties.ContentDescription])
            assertTrue(PulseRegionReadyName.isNotBlank())
        }
    }

    @Test
    fun the_reveal_keeps_the_specimen_geometry_at_double_text_scale() {
        fun size(state: String, textScale: Float): Pair<Int, Int> {
            var measured = 0 to 0
            runDesktopComposeUiTest(width = 360, height = 1600) {
                mainClock.autoAdvance = false
                specimen(UiPreferences(textScale = textScale)) { state }
                val node = onNodeWithTag("region").fetchSemanticsNode()
                measured = node.size.width to node.size.height
            }
            return measured
        }
        listOf(1f, 2f).forEach { scale ->
            val loading = size("running", scale)
            assertEquals(loading, size("idle", scale), "idle keeps the running geometry at ×$scale")
            assertEquals(loading, size("revealed", scale), "the reveal moves no layout at ×$scale")
        }
    }

    private fun revealFrames(reducedMotion: Boolean, times: List<Long>): List<ImageBitmap> {
        val captured = mutableListOf<ImageBitmap>()
        runDesktopComposeUiTest(width = 360, height = 640) {
            var state by mutableStateOf("idle")
            mainClock.autoAdvance = false
            specimen(UiPreferences(reducedMotion = reducedMotion)) { state }
            mainClock.advanceTimeByFrame()
            state = "revealed"
            var now = 0L
            times.forEach { at ->
                advance(at - now)
                now = at
                captured += onNodeWithTag("region").captureToImage()
            }
        }
        return captured
    }

    @Test
    fun the_reveal_crossfades_over_150_ms_under_full_motion() {
        val (first, middle, end, later) = revealFrames(false, listOf(16L, 90L, 200L, 600L))
        assertTrue(imageDistance(first, middle) > 0.0, "the crossfade is in progress at 90 ms")
        assertTrue(imageDistance(middle, end) > 0.0, "the crossfade is still moving after the midpoint")
        assertEquals(0.0, imageDistance(end, later), 1e-6, "settled by 200 ms: nothing keeps ticking")
    }

    @Test
    fun the_reveal_is_instantaneous_under_reduced_motion() {
        // A snap needs the frames that apply the state change, never a duration: settled within 3 frames.
        val (first, later) = revealFrames(true, listOf(48L, 600L))
        assertEquals(0.0, imageDistance(first, later), 1e-6)
    }
}
