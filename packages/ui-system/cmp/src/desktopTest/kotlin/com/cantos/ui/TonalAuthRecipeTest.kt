package com.cantos.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.ui.preview.AuthTonalSpecimen
import java.io.File
import kotlin.math.abs
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/**
 * Pixels of the production Auth v4.3 recipe on the JVM Compose test root: the tonal field and the
 * tonal auth actions paint no stroke at idle, focus or error, yet focus is still visible. The negative
 * controls are the existing stroked Floating field and the stroked AuthOutlined action, so a pass
 * cannot come from a sampling mistake. Not Android/iOS, a screen reader or an IME.
 */
@OptIn(ExperimentalTestApi::class)
class TonalAuthRecipeTest {
    private class Frames(val idle: ImageBitmap, val focused: ImageBitmap, val node: Rect, val scene: Rect)

    private fun Color.isNear(other: Color, tolerance: Float = 0.06f) =
        abs(red - other.red) + abs(green - other.green) + abs(blue - other.blue) < tolerance

    private fun capture(theme: Theme, content: @Composable (Modifier) -> Unit): Frames {
        val focus = FocusRequester()
        lateinit var frames: Frames
        runDesktopComposeUiTest(width = 390, height = 300) {
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences(theme = theme, expression = CantosExpression.Theatre)) {
                    Column(Modifier.testTag("scene").padding(tokenDp("space.xl"))) {
                        content(Modifier.focusRequester(focus).testTag("node"))
                    }
                }
            }
            waitForIdle()
            val idle = onNodeWithTag("scene").captureToImage()
            runOnIdle { focus.requestFocus() }
            onNodeWithTag("node").assertIsFocused()
            waitForIdle()
            frames = Frames(
                idle, onNodeWithTag("scene").captureToImage(),
                onNodeWithTag("node").fetchSemanticsNode().boundsInRoot,
                onNodeWithTag("scene").fetchSemanticsNode().boundsInRoot,
            )
        }
        return frames
    }

    private fun Frames.save(name: String) {
        val output = File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview")
        check(output.isDirectory || output.mkdirs())
        listOf("idle" to idle, "focused" to focused).forEach { (state, bitmap) ->
            File(output, "cmp-tonal-540-$name-$state.png").writeBytes(
                requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes,
            )
        }
    }

    /** The fill one pixel inside the top edge equals the fill deeper inside: nothing is stroked on the edge. */
    private fun ImageBitmap.edgeIsFill(node: Rect, scene: Rect, xFraction: Float = 0.85f): Boolean {
        val pixels = toPixelMap()
        val x = (node.left - scene.left + node.width * xFraction).toInt()
        val top = (node.top - scene.top).toInt()
        val bottom = (node.bottom - scene.top).toInt()
        return pixels[x, top + 1].isNear(pixels[x, top + 8]) && pixels[x, bottom - 2].isNear(pixels[x, bottom - 9])
    }

    @Test
    fun the_tonal_field_has_no_stroke_at_idle_focus_or_error_in_both_themes() {
        Theme.entries.forEach { theme ->
            val scenarios = mapOf<String, @Composable (Modifier) -> Unit>(
                "empty" to { m -> TextField("Địa chỉ email", "", {}, Modifier.fillMaxWidth(), variant = TextFieldVariant.FloatingTonal, controlModifier = m) },
                "filled" to { m -> TextField("Địa chỉ email", "linh@example.com", {}, Modifier.fillMaxWidth(), variant = TextFieldVariant.FloatingTonal, controlModifier = m) },
                "error" to { m ->
                    TextField("Địa chỉ email", "linh@example", {}, Modifier.fillMaxWidth(), variant = TextFieldVariant.FloatingTonal,
                        error = "Nhập địa chỉ email hợp lệ.", controlModifier = m)
                },
            )
            scenarios.forEach { (name, content) ->
                val frames = capture(theme, content)
                frames.save("field-$name-${theme.tokenName}")
                assertTrue(frames.idle.edgeIsFill(frames.node, frames.scene), "$name/${theme.tokenName}: idle edge is the fill")
                assertTrue(frames.focused.edgeIsFill(frames.node, frames.scene), "$name/${theme.tokenName}: focused edge is the fill")
            }
        }
    }

    @Test
    fun negative_control_the_stroked_floating_field_does_draw_a_focus_edge() {
        val frames = capture(Theme.Light) { m ->
            TextField("Địa chỉ email", "linh@example.com", {}, Modifier.fillMaxWidth(), variant = TextFieldVariant.Floating, controlModifier = m)
        }
        assertTrue(!frames.focused.edgeIsFill(frames.node, frames.scene), "the sampler must see the existing inset focus stroke")
    }

    @Test
    fun tonal_field_focus_is_still_visible_through_the_fill() {
        Theme.entries.forEach { theme ->
            val frames = capture(theme) { m ->
                TextField("Địa chỉ email", "", {}, Modifier.fillMaxWidth(), variant = TextFieldVariant.FloatingTonal, controlModifier = m)
            }
            val x = (frames.node.left - frames.scene.left + frames.node.width * 0.85f).toInt()
            val y = (frames.node.center.y - frames.scene.top).toInt()
            assertTrue(!frames.idle.toPixelMap()[x, y].isNear(frames.focused.toPixelMap()[x, y], 0.02f),
                "${theme.tokenName}: focus moves the fill to the higher tonal container")
        }
    }

    @Test
    fun tonal_actions_draw_no_outline_and_show_focus_as_a_fill_inside_their_bounds() {
        val actions = mapOf<String, @Composable (Modifier) -> Unit>(
            "primary" to { m -> CompositionLocalProvider(tonalAuthActions()) { Button("Đăng nhập", {}, variant = ButtonVariant.AuthPrimary, controlModifier = m.fillMaxWidth()) } },
            "outlined" to { m -> CompositionLocalProvider(tonalAuthActions()) { Button("Để sau", {}, variant = ButtonVariant.AuthOutlined, controlModifier = m.fillMaxWidth()) } },
            "text" to { m -> CompositionLocalProvider(tonalAuthActions()) { Button("Tạo tài khoản Cantos", {}, variant = ButtonVariant.Text, controlModifier = m.fillMaxWidth()) } },
        )
        Theme.entries.forEach { theme ->
            actions.forEach { (name, content) ->
                val frames = capture(theme, content)
                frames.save("action-$name-${theme.tokenName}")
                assertTrue(frames.idle.edgeIsFill(frames.node, frames.scene, 0.7f), "$name/${theme.tokenName}: no idle outline")
                assertTrue(frames.focused.edgeIsFill(frames.node, frames.scene, 0.7f), "$name/${theme.tokenName}: no focus ring")
                val x = (frames.node.left - frames.scene.left + 12).toInt()
                val y = (frames.node.center.y - frames.scene.top).toInt()
                assertTrue(!frames.idle.toPixelMap()[x, y].isNear(frames.focused.toPixelMap()[x, y], 0.02f),
                    "$name/${theme.tokenName}: focus is visible without a border")
            }
        }
    }

    @Test
    fun negative_control_the_default_outlined_action_keeps_its_outline() {
        val frames = capture(Theme.Light) { m -> Button("Để sau", {}, variant = ButtonVariant.AuthOutlined, controlModifier = m.fillMaxWidth()) }
        assertTrue(!frames.idle.edgeIsFill(frames.node, frames.scene, 0.7f), "the sampler must see the existing outline")
    }

    @Test
    fun the_specimen_keeps_the_eye_target_error_text_and_disabled_reasons_visible() {
        runDesktopComposeUiTest(width = 390, height = 1500) {
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences(theme = Theme.Light, expression = CantosExpression.Theatre)) {
                    AuthTonalSpecimen(Modifier.testTag("specimen"))
                }
            }
            waitForIdle()
            onNodeWithTag("specimen").assertExists()
            onAllNodesWithContentDescriptionOrNone("Hiện mật khẩu").forEach { eye ->
                eye.assertWidthIsAtLeast(48.dp).assertHeightIsAtLeast(48.dp)
            }
            assertEquals(2, onAllNodesWithContentDescriptionOrNone("Hiện mật khẩu").size, "each eye is its own control")
            onNodeWithText("Nhập địa chỉ email hợp lệ.").assertExists()
            onNodeWithText("Hai mật khẩu chưa giống nhau.").assertExists()
            onNodeWithText("Đang đăng nhập. Không gửi yêu cầu lần hai.").assertExists()
            onNodeWithText("Đăng nhập chưa khả dụng trên bản này.").assertExists()
        }
    }

    private fun androidx.compose.ui.test.ComposeUiTest.onAllNodesWithContentDescriptionOrNone(description: String) =
        onAllNodes(hasContentDescription(description)).fetchSemanticsNodes().indices.map { index ->
            onAllNodes(hasContentDescription(description))[index]
        }
}
