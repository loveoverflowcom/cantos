package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.printToString
import com.cantos.ui.preview.ListenHeroSample
import org.junit.Test
import java.io.File
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Actual shared Button on the JVM Compose renderer, not Android/iOS or AT. */
@OptIn(ExperimentalTestApi::class)
class ListenHeroTest {
    @Test
    fun one_action_can_pause_buffering_while_resolving_blocks_duplicate_activation() {
        androidx.compose.ui.test.runDesktopComposeUiTest(width = 320, height = 420) {
            var state by mutableStateOf("listen-buffering")
            var presses = 0
            val focus = FocusRequester()
            setContent {
                BundledPreviewCaptureRoot(UiPreferences(expression = CantosExpression.Theatre)) {
                    ListenHeroSample(state, { presses++; state = "listen-resolving" }, Modifier.testTag("hero").focusRequester(focus))
                }
            }
            runOnIdle { focus.requestFocus() }
            onNodeWithTag("hero").assertIsEnabled().assertIsFocused().performClick()
            assertEquals(1, presses)
            onNodeWithTag("hero").assertIsNotEnabled().assertIsFocused()
            onNodeWithText("Đang chuẩn bị bản nghe…").assertExists()
            onNodeWithTag("hero").performClick()
            assertEquals(1, presses)
        }
    }

    @Test
    fun scoped_geometry_has_a_208_by_56_minimum_and_leaves_the_regular_button_at_48() {
        androidx.compose.ui.test.runDesktopComposeUiTest(width = 320, height = 420) {
            setContent {
                BundledPreviewCaptureRoot(UiPreferences(expression = CantosExpression.Theatre)) {
                    Stack {
                        Button("Nghe", {}, variant = ButtonVariant.ListenHero, icon = "play_arrow", controlModifier = Modifier.testTag("hero"))
                        Button("Nghe", {}, icon = "play_arrow", controlModifier = Modifier.testTag("regular"))
                    }
                }
            }
            val hero = onNodeWithTag("hero").fetchSemanticsNode().size
            assertTrue(hero.width >= 208)
            assertEquals(56, hero.height)
            assertEquals(48, onNodeWithTag("regular").fetchSemanticsNode().size.height)
        }
    }

    @Test
    fun paired_states_at_phone_width_and_double_text_are_captured_with_semantics() {
        for (theme in Theme.entries) for ((width, scale) in listOf(320 to 1f, 320 to 2f, 390 to 2f)) {
            for (phase in listOf("ready", "resolving", "buffering", "playing", "paused", "failed", "unavailable")) {
                val preferences = UiPreferences(theme = theme, textScale = scale, reducedMotion = true, expression = CantosExpression.Theatre)
                val name = "listen-${theme.tokenName}-$width-${scale.toInt()}-$phase"
                var hostDensity = 0f
                var hostFontScale = 0f
                androidx.compose.ui.test.runDesktopComposeUiTest(width = width, height = 420) {
                    setContent {
                        hostDensity = LocalDensity.current.density
                        hostFontScale = LocalDensity.current.fontScale
                        BundledPreviewCaptureRoot(preferences) {
                            Box(Modifier.fillMaxSize().testTag("scene").padding(tokenDp("space.md"))) {
                                ListenHeroSample("listen-$phase", {}, Modifier.testTag("hero"))
                            }
                        }
                    }
                    val control = onNodeWithTag("hero").fetchSemanticsNode()
                    assertTrue(control.boundsInRoot.left >= 0 && control.boundsInRoot.right <= width)
                    if (phase in listOf("resolving", "unavailable")) onNodeWithTag("hero").assertIsNotEnabled()
                    else onNodeWithTag("hero").assertIsEnabled()
                    writeCapture(CaptureScenario(name, "listen-hero", "button", phase,
                        themeMode = if (theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light, systemIsDark = false,
                        preferences = preferences, viewportWidthDp = width, viewportHeightDp = 420,
                        hostDensity = hostDensity, hostFontScale = hostFontScale, fontSource = BundledPreviewFontSource),
                        onNodeWithTag("scene").captureToImage())
                    File(captureDirectory(), "$name.semantics.txt").writeText(onNodeWithTag("scene").printToString())
                }
            }
        }
    }
}
