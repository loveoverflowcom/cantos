package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import kotlin.math.abs
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Issue #421 on the production components, JVM Desktop Compose only: the pending ring turns under
 * full motion over a manual clock and holds a visible stationary arc under reduced motion, and an
 * auth field paints one inset perimeter (invalid 1 dp, focus 2 dp, focused-invalid 2 dp in the error
 * colour) instead of stacking a ring on the error outline. Pixels here
 * are not Android/iOS runtime, TalkBack or a recorded video.
 */
@OptIn(ExperimentalTestApi::class)
class PendingRingAndFieldIndicatorTest {
    private val reason = DisabledReason.of("Đang gửi yêu cầu, không gửi lần hai.")

    private fun ComposeUiTest.advance(millis: Long) {
        val until = mainClock.currentTime + millis
        while (mainClock.currentTime < until) mainClock.advanceTimeByFrame()
    }

    /** Frames of the pending Auth action at the given virtual times, or null when the ring never differs. */
    private fun pendingFrames(reducedMotion: Boolean, times: List<Long>, scale: Float = 1f): List<ImageBitmap> {
        val frames = mutableListOf<ImageBitmap>()
        runDesktopComposeUiTest(width = 390, height = 200) {
            // The test policy cancels infinite animations while the clock auto-advances, so pause it first.
            mainClock.autoAdvance = false
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences(reducedMotion = reducedMotion, textScale = scale)) {
                    Box(Modifier.padding(tokenDp("space.lg"))) {
                        Button("Gửi liên kết", {}, state = ButtonState.Loading(reason), variant = ButtonVariant.AuthPrimary,
                            icon = "mail", controlModifier = Modifier.requiredSize(300.dp, 60.dp).testTag("pending"))
                    }
                }
            }
            mainClock.advanceTimeByFrame()
            var now = 0L
            times.forEach { at ->
                advance(at - now)
                now = at
                frames += onNodeWithTag("pending").captureToImage()
            }
        }
        return frames
    }

    @Test
    fun pending_ring_turn_is_one_second_under_full_motion_and_absent_under_reduced_motion() {
        assertEquals(PendingRingTurnMillis, pendingRingTurnMillis(MotionPreference.Full))
        assertEquals(null, pendingRingTurnMillis(MotionPreference.Reduced))
    }

    @Test
    fun the_pending_ring_moves_over_time_under_full_motion() {
        // 1008 ms is the first 16 ms frame at or after one whole turn.
        val (start, quarter, half, full) = pendingFrames(false, listOf(0L, 250L, 500L, 1_008L))
        assertTrue(imageDistance(start, quarter) > 0.0, "a quarter turn must repaint the ring")
        assertTrue(imageDistance(start, half) > imageDistance(start, quarter), "a half turn is further from rest than a quarter turn")
        assertTrue(imageDistance(start, full) < imageDistance(start, quarter) / 4, "one whole turn is back near the rest frame")
    }

    @Test
    fun the_pending_ring_stays_a_visible_stationary_arc_under_reduced_motion() {
        val (start, later, muchLater) = pendingFrames(true, listOf(0L, 400L, 1_700L))
        assertEquals(0.0, imageDistance(start, later), 1e-6)
        assertEquals(0.0, imageDistance(start, muchLater), 1e-6)
        val pixels = start.toPixelMap()
        val foreground = tokenColorInTheme("color.onSurfaceVariant")
        val painted = (0 until start.height).sumOf { y -> (0 until start.width).count { x -> pixels[x, y].isNear(foreground) } }
        assertTrue(painted > 0, "reduced motion still paints readable pending feedback")
    }

    private fun tokenColorInTheme(name: String): Color = parseTokenColor(CantosTokens.color(name, Theme.Light.tokenName))

    private fun Color.isNear(other: Color) = abs(red - other.red) + abs(green - other.green) + abs(blue - other.blue) < 0.06f

    private data class Field(val unfocused: ImageBitmap, val focused: ImageBitmap, val topPx: Int, val bottomPx: Int, val density: Float)

    private fun invalidField(error: String?): Field {
        val focus = FocusRequester()
        lateinit var unfocused: ImageBitmap
        lateinit var focused: ImageBitmap
        var top = 0
        var bottom = 0
        var density = 0f
        runDesktopComposeUiTest(width = 390, height = 300) {
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences()) {
                    density = androidx.compose.ui.platform.LocalDensity.current.density
                    Column(Modifier.testTag("scene").padding(tokenDp("space.lg"))) {
                        TextField("Email", "linh@example", {}, variant = TextFieldVariant.AuthTonal, error = error,
                            controlModifier = Modifier.focusRequester(focus).testTag("field"))
                    }
                }
            }
            waitForIdle()
            val node = onNodeWithTag("field").fetchSemanticsNode().boundsInRoot
            top = node.top.toInt()
            bottom = node.bottom.toInt()
            unfocused = onNodeWithTag("scene").captureToImage()
            runOnIdle { focus.requestFocus() }
            onNodeWithTag("field").assertIsFocused()
            waitForIdle()
            focused = onNodeWithTag("scene").captureToImage()
        }
        return Field(unfocused, focused, top, bottom, density)
    }

    /**
     * Pixels along a vertical line through the field's horizontal middle, from [outside] rows above the
     * field's top edge to [depth] rows into it. Message text below the field is never sampled.
     */
    private fun column(image: ImageBitmap, field: Field, outside: Int, depth: Int): List<Color> {
        val x = (390 * field.density / 2f).toInt().coerceAtMost(image.width - 1)
        val pixels = image.toPixelMap()
        return ((field.topPx - outside) until (field.topPx + depth)).map { pixels[x, it.coerceIn(0, image.height - 1)] }
    }

    /** Consecutive rows, from the field's top edge inward, painted in [color]: the stroke thickness. */
    private fun strokeRows(image: ImageBitmap, field: Field, color: Color): Int =
        column(image, field, 0, 12).takeWhile { it.isNear(color) }.size

    private fun paintedAbove(image: ImageBitmap, field: Field): List<Color> = column(image, field, 12, 0)

    @Test
    fun an_invalid_field_keeps_one_inside_error_stroke_and_focus_swaps_it_for_one_thicker_error_stroke() {
        val field = invalidField("Email cần được kiểm tra.")
        val error = tokenColorInTheme("color.error")
        val focusColor = tokenColorInTheme("color.focus")
        assertEquals(1, strokeRows(field.unfocused, field, error), "an unfocused invalid field has one 1 dp error stroke")
        assertEquals(2, strokeRows(field.focused, field, error), "focus is one 2 dp stroke in the error colour, never stacked")
        assertTrue(column(field.focused, field, 12, 12).none { it.isNear(focusColor) }, "no second, focus-coloured perimeter")
        assertEquals(paintedAbove(field.unfocused, field), paintedAbove(field.focused, field), "nothing is painted outside the bounds")
    }

    @Test
    fun a_focused_valid_field_paints_one_inside_focus_stroke_and_no_error_colour() {
        val field = invalidField(null)
        val error = tokenColorInTheme("color.error")
        val focusColor = tokenColorInTheme("color.focus")
        assertEquals(0, strokeRows(field.unfocused, field, focusColor), "idle paints no perimeter")
        assertEquals(2, strokeRows(field.focused, field, focusColor), "focus is one 2 dp stroke")
        assertTrue(column(field.focused, field, 12, 12).none { it.isNear(error) })
        assertEquals(paintedAbove(field.unfocused, field), paintedAbove(field.focused, field), "nothing is painted outside the bounds")
    }
}
