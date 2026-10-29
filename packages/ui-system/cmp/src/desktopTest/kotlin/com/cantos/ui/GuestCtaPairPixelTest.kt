package com.cantos.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.text.TextLayoutResult
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
 * Pixels and text colours of the production Button inside the profile card on the JVM Compose test
 * root, density 1. Not Android/iOS, a screen reader or a design match; a reviewer opens the capture.
 */
@OptIn(ExperimentalTestApi::class)
class GuestCtaPairPixelTest {
    private val signIn = "Đăng nhập"
    private val create = "Đăng ký"

    private fun role(name: String, theme: Theme): Color =
        parseTokenColor(CantosTokens.color(CantosTokens.role(name, CantosExpression.Theatre.tokenName), theme.tokenName))

    private fun Color.isNear(other: Color) = abs(red - other.red) + abs(green - other.green) + abs(blue - other.blue) < 0.08f

    private fun androidx.compose.ui.test.ComposeUiTest.labelColor(text: String): Color {
        var color = Color.Unspecified
        onNodeWithText(text, useUnmergedTree = true).performSemanticsAction(SemanticsActions.GetTextLayoutResult) { read ->
            val layouts = mutableListOf<TextLayoutResult>()
            assertTrue(read(layouts))
            color = layouts.single().layoutInput.style.color
        }
        return color
    }

    @Test
    fun the_guest_pair_is_a_deep_filled_and_a_transparent_outlined_pill_of_one_size_in_both_themes() {
        Theme.entries.forEach { theme ->
            runDesktopComposeUiTest(width = 390, height = 360) {
                setContent {
                    BundledPreviewCaptureRoot(preferences = UiPreferences(theme = theme, expression = CantosExpression.Theatre)) {
                        Column(Modifier.testTag("scene").requiredWidth(390.dp)) {
                            Surface(Modifier.fillMaxWidth(), tone = SurfaceTone.PrimaryContainer,
                                variant = SurfaceVariant.Profile, inset = Space.Lg) {
                                Column(verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(tokenDp("space.md"))) {
                                    Button(signIn, {}, variant = ButtonVariant.AuthPrimary, icon = "arrow_forward",
                                        controlModifier = Modifier.fillMaxWidth().testTag("sign-in"))
                                    Button(create, {}, variant = ButtonVariant.AuthOutlined, icon = "person_add",
                                        controlModifier = Modifier.fillMaxWidth().testTag("create"))
                                }
                            }
                        }
                    }
                }
                waitForIdle()
                val filled = onNodeWithTag("sign-in").fetchSemanticsNode().boundsInRoot
                val outlined = onNodeWithTag("create").fetchSemanticsNode().boundsInRoot
                assertEquals(filled.width, outlined.width, "$theme width")
                assertEquals(filled.height, outlined.height, "$theme height")
                assertTrue(filled.height >= 60f, "$theme height")

                val ink = role("color.onPrimaryContainer", theme)
                val card = role("color.primaryContainer", theme)
                assertTrue(labelColor(signIn).isNear(card), "$theme sign-in text reads the card colour")
                assertTrue(labelColor(create).isNear(ink), "$theme create text reads the card foreground")

                val image = onNodeWithTag("scene").captureToImage()
                // Opened by a reviewer, never asserted: the production pair at 1 px per dp.
                File(System.getenv("CANTOS_UI_PREVIEW_CAPTURE_DIR") ?: "build/reports/cantos-preview").let { output ->
                    check(output.isDirectory || output.mkdirs())
                    File(output, "cmp-guest-cta-pair-v42b-${theme.tokenName}.png").writeBytes(
                        requireNotNull(Image.makeFromBitmap(image.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes)
                }
                val pixels = image.toPixelMap()
                val scene = onNodeWithTag("scene").fetchSemanticsNode().boundsInRoot
                fun at(rect: androidx.compose.ui.geometry.Rect, dx: Int) =
                    pixels[(rect.left - scene.left).toInt() + dx, (rect.center.y - scene.top).toInt()]
                // Past the 1.5 dp stroke, well left of the glyph: fill versus the card showing through.
                assertTrue(at(filled, 12).isNear(ink), "$theme filled pill is the card foreground: ${at(filled, 12)}")
                assertTrue(at(outlined, 12).isNear(card), "$theme outlined pill is transparent over the card: ${at(outlined, 12)}")
                assertTrue(at(outlined, 0).isNear(ink) || at(outlined, 1).isNear(ink), "$theme outline is solid foreground")
            }
        }
    }

    @Test
    fun the_remap_belongs_to_the_profile_card_only() {
        runDesktopComposeUiTest(width = 390, height = 320) {
            setContent {
                BundledPreviewCaptureRoot(preferences = UiPreferences(expression = CantosExpression.Theatre)) {
                    Column(Modifier.requiredWidth(390.dp)) {
                        Surface(tone = SurfaceTone.Container, variant = SurfaceVariant.Regular) {
                            Button("Ngoài thẻ hồ sơ", {}, variant = ButtonVariant.AuthPrimary)
                        }
                        Surface(tone = SurfaceTone.PrimaryContainer, variant = SurfaceVariant.Profile) {
                            Surface(tone = SurfaceTone.Container) {
                                Button("Thẻ lồng nhau", {}, variant = ButtonVariant.AuthPrimary)
                            }
                        }
                        Surface(tone = SurfaceTone.PrimaryContainer, variant = SurfaceVariant.Membership) {
                            Button("Hội viên", {}, variant = ButtonVariant.AuthPrimary)
                        }
                    }
                }
            }
            waitForIdle()
            val onPrimary = role("color.onPrimary", Theme.Light)
            listOf("Ngoài thẻ hồ sơ", "Thẻ lồng nhau", "Hội viên").forEach {
                assertTrue(labelColor(it).isNear(onPrimary), "$it keeps the global AuthPrimary text")
            }
        }
    }
}
