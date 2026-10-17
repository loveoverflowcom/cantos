package com.cantos.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertHasNoClickAction
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import androidx.compose.ui.text.TextLayoutResult
import com.cantos.ui.preview.AuthAccountSpecimen
import com.cantos.ui.preview.AuthActionSample
import com.cantos.ui.preview.ComponentExample
import java.io.File
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** These actual JVM captures become visual evidence only after their pixels are opened. */
class AuthAccountCaptureTest {
    @Test
    fun auth_account_light_narrow_vietnamese_anatomy_is_captured() {
        capture("account-light-390-text1", UiPreferences(density = Density.Comfortable), height = 1600, wholeScene = true) {
            AuthAccountSpecimen()
        }
    }

    @Test
    fun auth_account_dark_narrow_vietnamese_anatomy_is_captured() {
        capture("account-dark-390-text1", UiPreferences(theme = Theme.Dark, density = Density.Comfortable), height = 1600, wholeScene = true) {
            AuthAccountSpecimen()
        }
    }

    @Test
    fun auth_account_dark_narrow_large_text_error_keeps_intrinsic_groups_visible() {
        capture("account-dark-390-text2-error", UiPreferences(theme = Theme.Dark, density = Density.Comfortable, textScale = 2f),
            height = 4800, wholeScene = true) { AuthAccountSpecimen("auth-error") }
    }

    @Test
    fun auth_primary_real_focus_is_captured_without_replacing_the_native_focus_node() {
        val focus = FocusRequester()
        capture("primary-light-390-focus", UiPreferences(), focus = focus) {
            AuthActionSample("auth-primary", {}, Modifier.focusRequester(focus))
        }
    }

    @Test
    fun auth_primary_real_pressed_interaction_is_captured_in_dark_theme() {
        capture("primary-dark-390-pressed", UiPreferences(theme = Theme.Dark)) {
            ComponentExample(PreviewExample.Button, "pressed", {}, Modifier)
        }
    }

    @Test
    fun auth_loading_disabled_and_error_reasons_are_captured_without_opaque_controls() {
        capture("states-light-390", UiPreferences(), height = 1000) {
            listOf("auth-loading", "auth-disabled", "auth-error").forEach { AuthActionSample(it, {}) }
            BackButton(AccessibleName.of("Quay lại Tài khoản"), {}, state = ButtonState.Disabled(DisabledReason.of("Nội dung đang được giữ lại.")))
        }
    }

    @Test
    fun auth_tonal_field_focus_and_error_have_separate_readable_supporting_text() {
        val focus = FocusRequester()
        capture("field-light-390-focus-error", UiPreferences(textScale = 2f), height = 1000, fieldFocus = focus) {
            TextField("Tên hiển thị · Ỗ, Ữ", "Nguyễn Thị Minh Phương", {},
                variant = TextFieldVariant.AuthTonal,
                help = "Giữ nguyên dấu tiếng Việt và nội dung đã nhập.",
                error = "Tên hiển thị cần được kiểm tra.", controlModifier = Modifier.focusRequester(focus).testTag("focused-field"))
        }
    }

    @Test
    fun every_badge_tone_retains_readable_vietnamese_status_at_large_text_in_both_themes() {
        val labels = listOf("Bản nháp chưa xuất bản · Ỗ, Ữ", "Thông tin cần xem lại", "Đã duyệt bản ghi",
            "Cần kiểm tra quyền sử dụng", "Chưa thể xuất bản nội dung")
        listOf(Theme.Light, Theme.Dark).forEach { theme ->
            capture("badges-${theme.tokenName}-390-text2", UiPreferences(theme = theme, textScale = 2f),
                height = 1000, badgeLabels = labels) {
                StatusTone.entries.zip(labels).forEach { (tone, label) -> Badge(label, tone = tone) }
            }
        }
    }

    @OptIn(ExperimentalTestApi::class)
    private fun capture(
        name: String,
        preferences: UiPreferences,
        height: Int = 600,
        wholeScene: Boolean = false,
        focus: FocusRequester? = null,
        fieldFocus: FocusRequester? = null,
        badgeLabels: List<String> = emptyList(),
        content: @Composable () -> Unit,
    ) {
        runDesktopComposeUiTest(width = 390, height = height) {
            setContent {
                Box(Modifier.requiredSize(390.dp, height.dp).testTag("auth-capture-root")) {
                    UiRoot(preferences = preferences) {
                        // Unbounded vertical measurement prevents the last card from consuming
                        // only a bounded Column's remaining height and clipping its own text.
                        val scene = if (wholeScene) Modifier.verticalScroll(rememberScrollState()) else Modifier
                        Column(scene.padding(tokenDp("space.lg")),
                            verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) { content() }
                    }
                }
            }
            waitForIdle()
            if (focus != null) {
                runOnIdle { focus.requestFocus() }
                onNodeWithContentDescription("Đăng nhập với Cantos ID").assertIsFocused()
            }
            if (fieldFocus != null) {
                runOnIdle { fieldFocus.requestFocus() }
                onNodeWithTag("focused-field").assertIsFocused()
            }
            // The retained PressInteraction is emitted after a frame; allow the token tween to settle.
            mainClock.advanceTimeBy(400)
            waitForIdle()
            if (wholeScene) {
                val footerNode = onNodeWithText("Appearance and listening settings form a quiet group without a decorative border.")
                    .assertIsDisplayed()
                val footer = footerNode.fetchSemanticsNode().boundsInRoot
                assertTrue(footer.width > 0f && footer.height > 0f, "Offscreen zero bounds cannot establish complete visibility")
                assertTrue(footer.top >= 0f && footer.bottom < height.toFloat(), "The whole intrinsic specimen must fit the capture")
                footerNode.performSemanticsAction(SemanticsActions.GetTextLayoutResult) { getLayout ->
                    val layouts = mutableListOf<TextLayoutResult>()
                    assertTrue(getLayout(layouts))
                    val layout = layouts.single()
                    assertFalse(layout.hasVisualOverflow, "The footer must retain all its wrapped Vietnamese text")
                    assertTrue(footer.height + 0.5f >= layout.size.height.toFloat(), "The capture must include the unclipped text layout height")
                }
            }
            val bitmap = onNodeWithTag("auth-capture-root").captureToImage()
            assertEquals(390, bitmap.width)
            assertEquals(height, bitmap.height)
            val output = File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview")
            check(output.isDirectory || output.mkdirs()) { "Could not create auth screenshot output directory" }
            val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
            File(output, "cmp-auth-$name.png").writeBytes(bytes)
            badgeLabels.forEach { label ->
                onNodeWithText(label).assertIsDisplayed().assertHasNoClickAction()
                    .performSemanticsAction(SemanticsActions.GetTextLayoutResult) { getLayout ->
                        val layouts = mutableListOf<TextLayoutResult>()
                        assertTrue(getLayout(layouts))
                        val layout = layouts.single()
                        // Intrinsic Text can retain a wider cached paragraph constraint than
                        // its actual width; measure each drawn line instead of that cache width.
                        assertFalse(layout.didOverflowHeight, "Status '$label' must retain every line")
                        assertEquals(label.length, layout.getLineEnd(layout.lineCount - 1))
                        (0 until layout.lineCount).forEach { line ->
                            assertFalse(layout.isLineEllipsized(line))
                            assertTrue(layout.getLineRight(line) <= layout.size.width + 1f,
                                "Status '$label' line $line exceeds its rendered width")
                        }
                        if (label == badgeLabels.first()) assertTrue(layout.lineCount > 1,
                            "The long Vietnamese status must exercise actual wrapping")
                    }
            }
        }
    }
}
