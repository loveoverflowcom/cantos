package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

/**
 * The Shell, Auth and Membership headers present one theme control from one owner (the
 * `components` layer). JVM Desktop Compose only: this is not Android/iOS, TalkBack or VoiceOver
 * evidence. Each header keeps its own tag, localized name and intent.
 */
@OptIn(ExperimentalTestApi::class)
class TopBarParityTest {
    private class Header(val label: String, val stack: List<ShellRoute>, val tag: String)

    private val headers = listOf(
        Header("shell", listOf(ShellRoute.Discovery), "theme-toggle"),
        Header("auth", listOf(ShellRoute.Discovery, ShellRoute.Login), "auth-theme"),
        Header("membership", listOf(ShellRoute.Discovery, ShellRoute.Account, ShellRoute.MembershipPreview), "membership-theme"),
    )

    private fun host(
        header: Header,
        theme: ShellTheme,
        locale: ShellLocale = ShellLocale.Vietnamese,
        width: Int = 390,
        height: Int = 844,
        textScale: Float = 1f,
        body: ComposeUiTest.() -> Unit,
    ) = runDesktopComposeUiTest(width = width, height = height) {
        setContent {
            Box(Modifier.requiredSize(width.dp, height.dp)) {
                PreviewTestRoot(PreviewSeed(previewState(stack = header.stack, theme = theme, locale = locale)), textScale = textScale)
            }
        }
        body()
    }

    private fun ComposeUiTest.pixel(x: Float, y: Float): Int {
        val bitmap = onRoot().captureToImage().asSkiaBitmap()
        return bitmap.getColor(x.toInt(), y.toInt())
    }

    @Test
    fun every_header_shows_the_same_48_dp_tonal_theme_square_in_both_themes_and_languages() {
        listOf(ShellTheme.Dark, ShellTheme.Light).forEach { theme ->
            listOf(ShellLocale.Vietnamese, ShellLocale.English).forEach { locale ->
                val containers = headers.map { header ->
                    var sample = 0
                    var background = 0
                    host(header, theme, locale) {
                        val box = onNodeWithTag(header.tag).assertIsDisplayed().fetchSemanticsNode().boundsInRoot
                        assertEquals(48f, box.width, 0.5f, "${header.label} width")
                        assertEquals(48f, box.height, 0.5f, "${header.label} height")
                        // Inside the square, clear of its rounded corner and the centered 24 dp glyph.
                        sample = pixel(box.left + 5f, box.center.y)
                        background = pixel(2f, 2f)
                    }
                    assertNotEquals(background, sample, "${header.label} $theme container reads as a tonal square")
                    sample
                }
                assertEquals(1, containers.toSet().size, "$theme/$locale containers $containers")
            }
        }
    }

    @Test
    fun auth_and_membership_keep_back_brand_and_theme_on_one_48_dp_centerline() {
        listOf(headers[1], headers[2]).forEach { header ->
            listOf(ShellTheme.Dark, ShellTheme.Light).forEach { theme ->
                host(header, theme) {
                    val back = onNodeWithTag("back").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
                    val logo = onNodeWithTag("header-title", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
                    val themeBox = onNodeWithTag(header.tag).fetchSemanticsNode().boundsInRoot
                    assertEquals(back.center.y, themeBox.center.y, 1f, "${header.label} back/theme")
                    assertEquals(logo.center.y, themeBox.center.y, 1f, "${header.label} logo/theme")
                    assertTrue(back.right <= logo.left && logo.right <= themeBox.left, "${header.label} order")
                    assertTrue(back.width >= 48f - 0.5f && back.height >= 48f - 0.5f)
                }
            }
        }
    }

    @Test
    fun the_localized_name_and_the_theme_intent_are_unchanged() {
        listOf(headers[1], headers[2]).forEach { header ->
            host(header, ShellTheme.Light) {
                val node = onNodeWithTag(header.tag).assertHasClickAction()
                    .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
                node.assertContentDescriptionEquals("Chuyển giao diện sáng tối")
                val before = pixel(2f, 2f)
                node.performClick()
                waitForIdle()
                assertNotEquals(before, pixel(2f, 2f), "${header.label}: one press switches the theme")
                onNodeWithTag(header.tag).assertContentDescriptionEquals("Chuyển giao diện sáng tối")
            }
            host(header, ShellTheme.Light, ShellLocale.English) {
                onNodeWithTag(header.tag).assertContentDescriptionEquals("Switch light and dark theme")
            }
        }
    }

    @Test
    fun large_text_on_a_narrow_screen_keeps_the_theme_square_whole_and_inside_the_screen() {
        listOf(headers[1], headers[2]).forEach { header ->
            host(header, ShellTheme.Dark, width = 320, height = 568, textScale = 2f) {
                val logo = onNodeWithTag("header-title", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
                val themeBox = onNodeWithTag(header.tag).assertIsDisplayed().fetchSemanticsNode().boundsInRoot
                assertEquals(48f, themeBox.width, 0.5f)
                assertEquals(48f, themeBox.height, 0.5f)
                assertTrue(themeBox.right <= 320f && logo.right <= themeBox.left, "${header.label} $logo $themeBox")
            }
        }
    }
}
