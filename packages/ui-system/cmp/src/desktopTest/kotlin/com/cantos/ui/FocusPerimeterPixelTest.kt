package com.cantos.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import java.io.File
import kotlin.math.abs
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/**
 * Pixels of the production components on the JVM Compose test root. They prove where the focus ring
 * falls relative to the native node's bounds: nothing is painted outside them, the ring is on the
 * node's own edge, and focus moves neither the bounds nor the size. They are not Android/iOS, a
 * screen reader, an IME or a design match, and an opened capture is a separate review step.
 */
@OptIn(ExperimentalTestApi::class)
class FocusPerimeterPixelTest {
    private class Scenario(
        val name: String,
        val expression: CantosExpression,
        /** The ring token role, drawn on the node's edge while focused. */
        val ringRole: String,
        val content: @Composable (Modifier) -> Unit,
    ) {
        /** A ring in the content colour of a primary fill sits one stroke (2 dp) inside the edge. */
        val ringDepth: Int get() = if (ringRole == "color.onPrimary" || ringRole == "color.onPrimaryContainer") 2 else 0
    }

    private class Frames(val idle: ImageBitmap, val focused: ImageBitmap, val idleNode: Rect, val focusedNode: Rect, val scene: Rect, val density: Float)

    private val name = AccessibleName.of("Mở")

    private val scenarios: List<Scenario> = listOf(
        Scenario("button-primary-theatre", CantosExpression.Theatre, "color.onPrimary") { m ->
            Button("Đăng nhập", {}, controlModifier = m)
        },
        Scenario("button-tonal-theatre", CantosExpression.Theatre, "color.focus") { m ->
            Button("Đăng nhập", {}, variant = ButtonVariant.Tonal, controlModifier = m)
        },
        Scenario("button-secondary-theatre", CantosExpression.Theatre, "color.focus") { m ->
            Button("Đăng nhập", {}, variant = ButtonVariant.Secondary, controlModifier = m)
        },
        Scenario("button-text-theatre", CantosExpression.Theatre, "color.focus") { m ->
            Button("Đăng nhập", {}, variant = ButtonVariant.Text, controlModifier = m)
        },
        Scenario("auth-primary-standard", CantosExpression.Standard, "color.onPrimary") { m ->
            Button("Đăng nhập", {}, variant = ButtonVariant.AuthPrimary, controlModifier = m.fillMaxWidth())
        },
        Scenario("auth-secondary-standard", CantosExpression.Standard, "color.focus") { m ->
            Button("Đăng ký", {}, variant = ButtonVariant.AuthSecondary, controlModifier = m.fillMaxWidth())
        },
        Scenario("auth-outlined-standard", CantosExpression.Standard, "color.focus") { m ->
            Button("Đăng ký", {}, variant = ButtonVariant.AuthOutlined, controlModifier = m.fillMaxWidth())
        },
        Scenario("back-standard", CantosExpression.Standard, "color.focus") { m ->
            BackButton(name, {}, controlModifier = m)
        },
        Scenario("icon-tonal-circular-theatre", CantosExpression.Theatre, "color.focus") { m ->
            IconButton(name, "search", {}, controlModifier = m, variant = ButtonVariant.Tonal, circular = true)
        },
        Scenario("icon-primary-theatre", CantosExpression.Theatre, "color.onPrimary") { m ->
            IconButton(name, "search", {}, controlModifier = m, variant = ButtonVariant.Primary)
        },
        Scenario("icon-text-theatre", CantosExpression.Theatre, "color.focus") { m ->
            IconButton(name, "search", {}, controlModifier = m)
        },
        Scenario("chip-unselected-theatre", CantosExpression.Theatre, "color.focus") { m ->
            FilterChip("Truyện đêm khuya", false, {}, controlModifier = m)
        },
        Scenario("chip-selected-theatre", CantosExpression.Theatre, "color.focus") { m ->
            FilterChip("Truyện đêm khuya", true, {}, controlModifier = m)
        },
        Scenario("auth-field-standard", CantosExpression.Standard, "color.focus") { m ->
            TextField("Email", "linh@example.com", {}, variant = TextFieldVariant.AuthTonal, controlModifier = m)
        },
        Scenario("auth-field-invalid-standard", CantosExpression.Standard, "color.error") { m ->
            TextField("Email", "linh@example", {}, variant = TextFieldVariant.AuthTonal, error = "Email cần được kiểm tra.", controlModifier = m)
        },
    )

    private fun role(name: String, expression: CantosExpression, theme: Theme): Color =
        parseTokenColor(CantosTokens.color(CantosTokens.role(name, expression.tokenName), theme.tokenName))

    private fun Color.isNear(other: Color, tolerance: Float = 0.08f) =
        abs(red - other.red) + abs(green - other.green) + abs(blue - other.blue) < tolerance

    private fun capture(scenario: Scenario, theme: Theme, textScale: Float = 1f): Frames {
        val focus = FocusRequester()
        lateinit var frames: Frames
        runDesktopComposeUiTest(width = 390, height = 300) {
            var density = 1f
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences(theme = theme, expression = scenario.expression, textScale = textScale)) {
                    density = LocalDensity.current.density
                    // The margin is wider than the outset ring (4 + 3 dp) so any leak is inside the capture.
                    Column(Modifier.testTag("scene").padding(tokenDp("space.xl"))) {
                        scenario.content(Modifier.focusRequester(focus).testTag("node"))
                    }
                }
            }
            waitForIdle()
            val idleNode = onNodeWithTag("node").fetchSemanticsNode().boundsInRoot
            val idle = onNodeWithTag("scene").captureToImage()
            runOnIdle { focus.requestFocus() }
            onNodeWithTag("node").assertIsFocused()
            waitForIdle()
            frames = Frames(
                idle, onNodeWithTag("scene").captureToImage(), idleNode,
                onNodeWithTag("node").fetchSemanticsNode().boundsInRoot,
                onNodeWithTag("scene").fetchSemanticsNode().boundsInRoot, density,
            )
        }
        return frames
    }

    /** Opened by a reviewer, never asserted: the same scene idle and focused, 1 px per dp. */
    private fun Frames.save(name: String) {
        val output = File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview")
        check(output.isDirectory || output.mkdirs())
        listOf("idle" to idle, "focused" to focused).forEach { (state, bitmap) ->
            File(output, "cmp-focus-478-$name-$state.png").writeBytes(
                requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes,
            )
        }
    }

    /** Pixels that changed with focus and lie outside the node's own bounds: a halo, gutter or second ring. */
    private fun Frames.paintedOutside(): Int {
        val before = idle.toPixelMap()
        val after = focused.toPixelMap()
        var changed = 0
        for (y in 0 until idle.height) for (x in 0 until idle.width) {
            val inside = x + scene.left >= focusedNode.left && x + scene.left < focusedNode.right &&
                y + scene.top >= focusedNode.top && y + scene.top < focusedNode.bottom
            if (!inside && before[x, y] != after[x, y]) changed += 1
        }
        return changed
    }

    /** The pixel one row into the stroke, [depth] dp below the top edge: wholly inside a 2 dp stroke at density 1. */
    private fun Frames.ringAtTopEdge(depth: Int): Color {
        val x = (focusedNode.center.x - scene.left).toInt()
        val top = (focusedNode.top - scene.top).toInt()
        return focused.toPixelMap()[x, top + depth + 1]
    }

    @Test
    fun painted_theatre_controls_paint_no_focus_pixel_outside_their_bounds_and_a_ring_on_their_edge() {
        scenarios.forEach { scenario ->
            Theme.entries.forEach { theme ->
                val frames = capture(scenario, theme)
                val label = "${scenario.name} / ${theme.tokenName}"
                frames.save("${scenario.name}-${theme.tokenName}")
                assertEquals(frames.idleNode, frames.focusedNode, "$label: focus must not move or resize the node")
                assertEquals(0, frames.paintedOutside(), "$label: the ring must not leave the node's bounds")
                val ring = role(scenario.ringRole, scenario.expression, theme)
                assertTrue(frames.ringAtTopEdge(scenario.ringDepth).isNear(ring), "$label: expected the ring $ring on the top edge, saw ${frames.ringAtTopEdge(scenario.ringDepth)}")
            }
        }
    }

    @Test
    fun the_ring_is_absent_without_focus_and_changes_no_pixel_of_the_idle_control() {
        scenarios.forEach { scenario ->
            val frames = capture(scenario, Theme.Light)
            val x = (frames.idleNode.center.x - frames.scene.left).toInt()
            val top = (frames.idleNode.top - frames.scene.top).toInt()
            val idleEdge = frames.idle.toPixelMap()[x, top + scenario.ringDepth + 1]
            val ring = role(scenario.ringRole, scenario.expression, Theme.Light)
            // The invalid auth field's idle 1 dp error stroke is the one deliberate idle perimeter.
            if (scenario.name != "auth-field-invalid-standard") {
                assertTrue(!idleEdge.isNear(ring), "${scenario.name}: no ring colour on the idle edge")
            }
        }
    }

    @Test
    fun the_ring_holds_at_large_text_and_narrow_width_with_realistic_vietnamese_labels() {
        listOf("button-primary-theatre", "auth-primary-standard", "chip-selected-theatre", "auth-field-invalid-standard").forEach { name ->
            val scenario = scenarios.single { it.name == name }
            val frames = capture(scenario, Theme.Dark, textScale = 2f)
            assertEquals(frames.idleNode, frames.focusedNode, "$name: no layout shift at 2x text")
            assertEquals(0, frames.paintedOutside(), "$name: no leak at 2x text")
            assertTrue(frames.ringAtTopEdge(scenario.ringDepth).isNear(role(scenario.ringRole, scenario.expression, Theme.Dark)), "$name: ring at 2x text")
        }
    }

    @Test
    fun the_standard_expression_and_unpainted_controls_keep_their_outset_ring_by_design() {
        // Characterisation of the intentional exceptions, so a future change to them is a decision.
        val standard = Scenario("button-primary-standard", CantosExpression.Standard, "color.focus") { m -> Button("Đăng nhập", {}, controlModifier = m) }
        val standardFrames = capture(standard, Theme.Light).also { it.save("button-primary-standard-light") }
        assertTrue(standardFrames.paintedOutside() > 0, "Standard Button keeps the outset ring")
        val link = Scenario("link-standard", CantosExpression.Standard, "color.focus") { m -> Link("Điều khoản", "/terms", {}, controlModifier = m) }
        val linkFrames = capture(link, Theme.Light).also { it.save("link-standard-light") }
        assertTrue(linkFrames.paintedOutside() > 0, "an unpainted Link keeps the outset ring")
    }

    @Test
    fun the_ring_follows_the_pressed_auth_capsule_corner_instead_of_staying_a_capsule() {
        val focus = FocusRequester()
        runDesktopComposeUiTest(width = 340, height = 140) {
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences(reducedMotion = true)) {
                    // Red marks everything outside the control, so the corner cannot be mistaken for it.
                    Column(Modifier.testTag("scene").background(Color.Red).padding(20.dp)) {
                        Button("Đăng nhập với Cantos ID", {}, variant = ButtonVariant.AuthPrimary,
                            controlModifier = Modifier.requiredSize(300.dp, 60.dp).focusRequester(focus).testTag("node"))
                    }
                }
            }
            waitForIdle()
            mainClock.autoAdvance = false
            runOnIdle { focus.requestFocus() }
            onNodeWithTag("node").performTouchInput { down(center) }
            repeat(2) { mainClock.advanceTimeByFrame() }
            mainClock.advanceTimeBy(400)
            val scene = onNodeWithTag("scene").fetchSemanticsNode().boundsInRoot
            val node = onNodeWithTag("node").fetchSemanticsNode().boundsInRoot
            val pixels = onNodeWithTag("scene").captureToImage().toPixelMap()
            val at = { dx: Int, dy: Int -> pixels[(node.left - scene.left).toInt() + dx, (node.top - scene.top).toInt() + dy] }
            val ring = role("color.onPrimary", CantosExpression.Standard, Theme.Light)
            val fill = role("color.primary", CantosExpression.Standard, Theme.Light)
            // The pressed radius is shape.authPressed (16 dp) and the ring sits 2 dp inside it, so its band
            // is 12..14 dp from the corner's centre: the centre of pixel (6,6) is 13.4 away, on the stroke.
            assertTrue(at(6, 6).isNear(ring), "the ring hugs the pressed corner, saw ${at(6, 6)}")
            assertTrue(!at(6, 6).isNear(fill), "a capsule-radius ring would leave the primary fill at the pressed corner")
            // (1,1) is outside the 16 dp arc: the backdrop, not the control.
            assertTrue(at(1, 1).isNear(Color.Red), "outside the pressed corner stays backdrop, saw ${at(1, 1)}")
        }
    }
}
