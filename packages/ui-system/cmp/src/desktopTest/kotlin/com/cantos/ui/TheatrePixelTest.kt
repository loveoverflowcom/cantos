package com.cantos.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.isPopup
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import kotlin.math.abs
import kotlin.math.roundToInt
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Pixels of the production Theatre components on the JVM Compose test root, light Vietnamese content
 * at 1x. They prove where strokes, fills and ink fall; they are not Android/iOS, an OS window, a
 * screen reader or a design match, and an opened capture is a separate review step.
 */
@OptIn(ExperimentalTestApi::class)
class TheatrePixelTest {
    private val theatre = UiPreferences(expression = CantosExpression.Theatre)

    private fun role(name: String, theme: Theme = Theme.Light): Color =
        parseTokenColor(CantosTokens.color(CantosTokens.role(name, "theatre"), theme.tokenName))

    private fun Color.isNear(other: Color, tolerance: Float = 0.06f) =
        abs(red - other.red) + abs(green - other.green) + abs(blue - other.blue) < tolerance

    private data class Frame(val image: ImageBitmap, val top: Int, val left: Int, val right: Int, val bottom: Int, val density: Float)

    private fun ComposeUiTest.frameOf(scene: String, field: String, density: Float): Frame {
        val bounds = onNodeWithTag(field).fetchSemanticsNode().boundsInRoot
        return Frame(onNodeWithTag(scene).captureToImage(), bounds.top.toInt(), bounds.left.toInt(), bounds.right.toInt(), bounds.bottom.toInt(), density)
    }

    /** Pixels down the field's vertical middle, from [margin] rows above its top edge to [depth] rows into it. */
    private fun Frame.column(margin: Int, depth: Int): List<Color> {
        val x = (left + right) / 2
        val pixels = image.toPixelMap()
        return ((top - margin) until (top + depth)).map { pixels[x, it.coerceIn(0, image.height - 1)] }
    }

    /** Consecutive rows from the field's top edge that are [color]; the stroke thickness inside the bounds. */
    private fun Frame.strokeRows(color: Color): Int {
        val rows = column(0, 12)
        return rows.takeWhile { it.isNear(color) }.size
    }

    private fun Frame.outside(colors: List<Color>) = column(4, 0).none { pixel -> colors.any { pixel.isNear(it) } }

    private fun floatingField(error: String?, focused: Boolean): Frame {
        val focus = FocusRequester()
        lateinit var result: Frame
        runDesktopComposeUiTest(width = 390, height = 300) {
            var density = 1f
            setContent {
                UiRoot(preferences = theatre) {
                    density = LocalDensity.current.density
                    Column(Modifier.testTag("scene").padding(tokenDp("space.lg"))) {
                        TextField("Email", "ban@example.com", {}, variant = TextFieldVariant.Floating, error = error,
                            controlModifier = Modifier.focusRequester(focus).testTag("field"))
                    }
                }
            }
            waitForIdle()
            if (focused) {
                runOnIdle { focus.requestFocus() }
                waitForIdle()
            }
            result = frameOf("scene", "field", density)
        }
        return result
    }

    private val focusColor get() = role("color.focus")
    private val errorColor get() = parseTokenColor(CantosTokens.color("color.error", "light"))

    @Test
    fun an_idle_valid_floating_field_paints_no_stroke_at_all() {
        val frame = floatingField(error = null, focused = false)
        assertEquals(0, frame.strokeRows(focusColor))
        assertEquals(0, frame.strokeRows(errorColor))
        assertTrue(frame.column(0, 12).none { it.isNear(focusColor) || it.isNear(errorColor) })
    }

    @Test
    fun focus_is_two_inset_units_in_the_focus_colour_and_never_leaves_the_bounds() {
        val frame = floatingField(error = null, focused = true)
        assertEquals((2 * frame.density).roundToInt(), frame.strokeRows(focusColor))
        assertTrue(frame.outside(listOf(focusColor, errorColor)), "no halo or outset ring above the shell")
    }

    @Test
    fun invalid_is_one_inset_unit_in_the_error_colour_with_no_focus_colour() {
        val frame = floatingField(error = "Kiểm tra địa chỉ email.", focused = false)
        assertEquals((1 * frame.density).roundToInt(), frame.strokeRows(errorColor))
        assertTrue(frame.column(0, 12).none { it.isNear(focusColor) })
        assertTrue(frame.outside(listOf(focusColor, errorColor)))
    }

    @Test
    fun focused_invalid_is_one_two_unit_error_perimeter_not_two_stacked_rings() {
        val frame = floatingField(error = "Kiểm tra địa chỉ email.", focused = true)
        assertEquals((2 * frame.density).roundToInt(), frame.strokeRows(errorColor))
        assertTrue(frame.column(0, 12).none { it.isNear(focusColor) }, "the focus colour never stacks on the error stroke")
        assertTrue(frame.outside(listOf(focusColor, errorColor)))
    }

    @Test
    fun long_text_never_runs_under_the_trailing_action() {
        lateinit var frame: Frame
        var eyeLeft = 0
        var eyeRight = 0
        runDesktopComposeUiTest(width = 390, height = 300) {
            setContent {
                UiRoot(preferences = theatre) {
                    Column(Modifier.testTag("scene").padding(tokenDp("space.lg"))) {
                        TextField("Mật khẩu", "W".repeat(80), {}, variant = TextFieldVariant.Floating,
                            controlModifier = Modifier.testTag("field"),
                            trailing = { IconButton(AccessibleName.of("Hiện mật khẩu"), "visibility", {}, controlModifier = Modifier.testTag("eye")) })
                    }
                }
            }
            waitForIdle()
            val eye = onNodeWithTag("eye").fetchSemanticsNode().boundsInRoot
            eyeLeft = eye.left.toInt()
            eyeRight = eye.right.toInt()
            frame = frameOf("scene", "field", 1f)
            writeFoundationCapture("cmp-theatre-floating-trailing-long-text-light", frame.image, theatre, 390, 300)
        }
        val pixels = frame.image.toPixelMap()
        val ink = role("color.onSurface")
        fun inkIn(from: Int, to: Int) = (from until to).sumOf { x ->
            ((frame.top + 8) until (frame.bottom - 8)).count { y -> pixels[x, y].isNear(ink) }
        }
        assertTrue(inkIn(frame.left + 8, eyeLeft - 4) > 100, "the long value is visibly painted left of the action")
        assertEquals(0, inkIn(eyeLeft, eyeRight), "no value ink inside the action's 48-unit hit area")
    }

    @Test
    fun the_open_theatre_select_shows_one_two_unit_inset_cue_and_idle_shows_none() {
        lateinit var idle: Frame
        lateinit var open: Frame
        runDesktopComposeUiTest(width = 390, height = 400) {
            var density = 1f
            setContent {
                UiRoot(preferences = theatre) {
                    density = LocalDensity.current.density
                    Column(Modifier.testTag("scene").padding(tokenDp("space.lg"))) {
                        Select("Ngôn ngữ", listOf("Tiếng Việt", "English"), 0, {}, expandedDescription = "Đang mở",
                            collapsedDescription = "Đang đóng", controlModifier = Modifier.testTag("select"))
                    }
                }
            }
            waitForIdle()
            idle = frameOf("scene", "select", density)
            onNodeWithTag("select").performClick()
            waitForIdle()
            open = frameOf("scene", "select", density)
        }
        assertEquals(0, idle.strokeRows(focusColor), "idle select is tonal with no stroke")
        assertTrue(idle.column(3, 4).last().isNear(role("color.surfaceContainer")), "board 5 utility select uses the field tonal surface")
        assertEquals((2 * open.density).roundToInt(), open.strokeRows(focusColor), "open select has one 2-unit inset cue")
        assertTrue(open.outside(listOf(focusColor)), "the cue stays inside the control")
    }

    private data class SelectFrames(val idle: Frame, val focused: Frame, val open: Frame)

    private fun theatreSelect(theme: Theme, error: String?): SelectFrames {
        val focus = FocusRequester()
        lateinit var result: SelectFrames
        runDesktopComposeUiTest(width = 390, height = 400) {
            var density = 1f
            setContent {
                UiRoot(preferences = theatre.copy(theme = theme)) {
                    density = LocalDensity.current.density
                    Column(Modifier.testTag("scene").padding(tokenDp("space.lg"))) {
                        Select("Ngôn ngữ", listOf("Tiếng Việt", "English"), 0, {},
                            expandedDescription = "Đang mở", collapsedDescription = "Đang đóng", error = error,
                            controlModifier = Modifier.focusRequester(focus).testTag("select"))
                    }
                }
            }
            waitForIdle()
            val trigger = onNodeWithTag("select")
            if (error.isNullOrBlank()) {
                assertEquals(null, trigger.fetchSemanticsNode().config.getOrNull(SemanticsProperties.Error))
            } else {
                assertEquals(error, trigger.fetchSemanticsNode().config.getOrNull(SemanticsProperties.Error))
                onNodeWithText(error).assertIsDisplayed()
            }
            val idle = frameOf("scene", "select", density)
            runOnIdle { focus.requestFocus() }
            waitForIdle()
            trigger.assertIsFocused()
            val focused = frameOf("scene", "select", density)
            trigger.performClick()
            waitForIdle()
            onNode(isPopup()).assertExists()
            result = SelectFrames(idle, focused, frameOf("scene", "select", density))
            val errorState = if (error.isNullOrBlank()) "valid" else "invalid"
            listOf("idle" to result.idle, "focused" to result.focused, "open" to result.open).forEach { (state, frame) ->
                writeCapture(CaptureScenario(
                    name = "cmp-theatre-select-$errorState-$state-${theme.tokenName}",
                    section = "theatre-foundation", component = "select", state = "$errorState-$state",
                    themeMode = if (theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light, systemIsDark = false,
                    preferences = theatre.copy(theme = theme), viewportWidthDp = 390, viewportHeightDp = 400,
                    hostDensity = density, hostFontScale = 1f,
                    fontSource = "platform fallback; intentional perimeter pixel diagnostic",
                ), frame.image)
            }
        }
        return result
    }

    @Test
    fun an_invalid_theatre_select_keeps_one_error_perimeter_when_idle_focused_or_open() {
        Theme.entries.forEach { theme ->
            val frames = theatreSelect(theme, "Kiểm tra ngôn ngữ đã chọn.")
            val error = role("color.error", theme)
            val focus = role("color.focus", theme)
            listOf("idle" to frames.idle, "focused" to frames.focused, "open" to frames.open).forEach { (state, frame) ->
                val thickness = if (state == "idle") 1 else 2
                assertEquals((thickness * frame.density).roundToInt(), frame.strokeRows(error), "$theme/$state error stroke")
                assertTrue(frame.column(0, 12).none { it.isNear(focus) }, "$theme/$state never stacks a focus-colour stroke")
                assertTrue(frame.outside(listOf(error, focus)), "$theme/$state keeps the perimeter inside the trigger")
            }
        }
    }

    @Test
    fun absent_or_blank_theatre_select_errors_keep_the_valid_idle_focus_and_open_cues() {
        Theme.entries.forEach { theme ->
            listOf(null, " \t\n").forEach { message ->
                val frames = theatreSelect(theme, message)
                val error = role("color.error", theme)
                val focus = role("color.focus", theme)
                assertTrue(frames.idle.column(0, 12).none { it.isNear(error) || it.isNear(focus) }, "$theme valid idle has no stroke")
                listOf("focused" to frames.focused, "open" to frames.open).forEach { (state, frame) ->
                    assertEquals((2 * frame.density).roundToInt(), frame.strokeRows(focus), "$theme/$state valid focus stroke")
                    assertTrue(frame.column(0, 12).none { it.isNear(error) }, "$theme/$state blank errors never paint an error stroke")
                    assertTrue(frame.outside(listOf(error, focus)), "$theme/$state keeps the perimeter inside the trigger")
                }
            }
        }
    }

    @Test
    fun theatre_chips_read_by_tone_the_selected_one_an_ink_capsule() {
        val tonal = role("color.surfaceContainer")
        val ink = role("color.primaryContainer")
        lateinit var chips: Pair<ImageBitmap, ImageBitmap>
        runDesktopComposeUiTest(width = 360, height = 120) {
            setContent {
                UiRoot(preferences = theatre) {
                    Row(Modifier.testTag("scene").padding(tokenDp("space.lg"))) {
                        FilterChip("Truyện", false, {}, controlModifier = Modifier.testTag("plain"))
                        FilterChip("Truyện", true, {}, controlModifier = Modifier.testTag("chosen"))
                    }
                }
            }
            waitForIdle()
            chips = onNodeWithTag("plain").captureToImage() to onNodeWithTag("chosen").captureToImage()
            writeFoundationCapture("cmp-theatre-chips-light", onNodeWithTag("scene").captureToImage(), theatre, 360, 120)
        }
        fun ImageBitmap.edgeFill() = toPixelMap()[width - 6, height / 2]
        assertTrue(chips.first.edgeFill().isNear(tonal), "unselected chip is the tonal container")
        assertTrue(chips.second.edgeFill().isNear(ink), "selected chip is the ink container")
        // A resting radius leaves the very corner unpainted; a capsule leaves much more of it.
        fun ImageBitmap.cornerCoverage(): Int {
            val map = toPixelMap()
            return (0 until 12).sumOf { y -> (0 until 12).count { x -> map[x, y].isNear(tonal) || map[x, y].isNear(ink) } }
        }
        assertTrue(chips.second.cornerCoverage() < chips.first.cornerCoverage(), "the selected shape is rounder than the resting shape")
    }

    @Test
    fun a_tonal_icon_button_is_a_rounded_square_and_a_docked_action_a_circle() {
        // Tonal is the pale tonal surface, not the ink that primaryContainer means for selection.
        val container = role("color.surfaceContainerHigh")
        val primary = role("color.primary")
        lateinit var images: Pair<ImageBitmap, ImageBitmap>
        runDesktopComposeUiTest(width = 240, height = 120) {
            setContent {
                UiRoot(preferences = theatre) {
                    Row(Modifier.padding(tokenDp("space.lg"))) {
                        IconButton(AccessibleName.of("Giao diện tối"), "dark_mode", {}, variant = ButtonVariant.Tonal, controlModifier = Modifier.testTag("theme"))
                        IconButton(AccessibleName.of("Tìm trong danh mục"), "search", {}, variant = ButtonVariant.Primary, circular = true,
                            controlModifier = Modifier.size(tokenDp("size.searchAction")).testTag("search"))
                    }
                }
            }
            waitForIdle()
            images = onNodeWithTag("theme").captureToImage() to onNodeWithTag("search").captureToImage()
        }
        val square = images.first.toPixelMap()
        val circle = images.second.toPixelMap()
        assertTrue(square[4, 4].isNear(container), "a 12-unit radius still fills this near-corner pixel")
        // The center contains the moon glyph, whose onSurface ink intentionally equals the
        // selected container. Sample unobstructed fill instead of mistaking that glyph for fill.
        assertTrue(square[4, 24].isNear(container), "the tonal action keeps its tonal fill beside the glyph")
        assertTrue(!circle[4, 4].isNear(primary), "a circle leaves the same near-corner pixel unpainted")
        assertTrue(circle[26, 26].isNear(primary) || circle[8, 26].isNear(primary), "the docked action is the primary tone")
    }

    @Test
    fun the_accent_avatar_is_the_theatre_pink_and_the_identity_default_is_not() {
        lateinit var images: Pair<ImageBitmap, ImageBitmap>
        runDesktopComposeUiTest(width = 200, height = 100) {
            setContent {
                UiRoot(preferences = theatre) {
                    Row(Modifier.padding(tokenDp("space.lg"))) {
                        Avatar("Cantos", size = AvatarSize.Control, accent = true, modifier = Modifier.testTag("accent"))
                        Avatar("Cantos", size = AvatarSize.Control, modifier = Modifier.testTag("identity"))
                    }
                }
            }
            waitForIdle()
            images = onNodeWithTag("accent").captureToImage() to onNodeWithTag("identity").captureToImage()
        }
        val pink = role("color.theatrePink")
        assertTrue(images.first.toPixelMap()[8, 24].isNear(pink), "accent fill is color.theatrePink")
        assertTrue(!images.second.toPixelMap()[8, 24].isNear(pink), "an identity avatar is not pinned to the accent")
    }

    /** Ink columns split into runs; the icon and the label are two runs separated by the 8-unit gap. */
    private fun inkRuns(image: ImageBitmap, container: Color, content: Color): List<IntRange> {
        val pixels = image.toPixelMap()
        val half = abs(container.red - content.red) + abs(container.green - content.green) + abs(container.blue - content.blue)
        fun ink(x: Int, y: Int): Boolean {
            val p = pixels[x, y]
            val distance = abs(p.red - container.red) + abs(p.green - container.green) + abs(p.blue - container.blue)
            return distance > half / 2
        }
        val columns = (8 until image.width - 8).filter { x -> (6 until image.height - 6).any { y -> ink(x, y) } }
        val runs = mutableListOf<IntRange>()
        columns.forEach { x ->
            val last = runs.lastOrNull()
            if (last != null && x - last.last <= 3) runs[runs.lastIndex] = last.first..x else runs += x..x
        }
        return runs
    }

    @Test
    fun an_icon_and_label_are_one_group_centered_on_both_axes_with_equal_insets() {
        val container = role("color.primary")
        val content = role("color.onPrimary")
        lateinit var image: ImageBitmap
        runDesktopComposeUiTest(width = 320, height = 120) {
            setContent {
                UiRoot(preferences = theatre) {
                    Box(Modifier.background(role("color.background")).padding(tokenDp("space.lg"))) {
                        Button("OK", {}, icon = "login", controlModifier = Modifier.requiredSize(200.dp, 48.dp).testTag("button"))
                    }
                }
            }
            waitForIdle()
            image = onNodeWithTag("button").captureToImage()
            writeFoundationCapture("cmp-theatre-button-icon-label-group-light", image, theatre, 320, 120)
        }
        val runs = inkRuns(image, container, content)
        assertEquals(2, runs.size, "the icon and the label are the two ink runs: $runs")
        val left = runs.first().first
        val right = image.width - 1 - runs.last().last
        assertEquals(left.toFloat(), right.toFloat(), 3f, "the group is centered horizontally: left $left, right $right")
        val pixels = image.toPixelMap()
        fun verticalCenter(run: IntRange): Float {
            val rows = (4 until image.height - 4).filter { y -> run.any { x -> abs(pixels[x, y].red - content.red) + abs(pixels[x, y].green - content.green) + abs(pixels[x, y].blue - content.blue) < 0.5f } }
            return (rows.first() + rows.last()) / 2f
        }
        val middle = (image.height - 1) / 2f
        assertEquals(middle, verticalCenter(runs.first()), 1.5f, "the icon is centered vertically")
        assertEquals(middle, verticalCenter(runs.last()), 2f, "the label's glyph box is centered vertically")
    }
}

internal fun writeFoundationCapture(name: String, bitmap: ImageBitmap, preferences: UiPreferences, width: Int, height: Int) {
    writeCapture(
        CaptureScenario(
            name = name, section = "theatre-foundation", component = null, state = null,
            themeMode = if (preferences.theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light, systemIsDark = false,
            preferences = preferences, viewportWidthDp = width, viewportHeightDp = height,
            hostDensity = 1f, hostFontScale = 1f,
        ),
        bitmap,
    )
}
