package com.cantos.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.assertWidthIsEqualTo
import androidx.compose.ui.test.assertHeightIsEqualTo
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.PreviewCatalog
import com.cantos.ui.preview.ComponentExample
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Captures every registered Theatre-expression specimen from the production preview dispatch, in both
 * themes at phone width and at 2x text, so each can be opened and reviewed. JVM test root only: not
 * Android/iOS, an OS window or a screen reader, and a capture never replaces an interaction check.
 */
@OptIn(ExperimentalTestApi::class)
class TheatreCaptureTest {
    private val specimens: List<Pair<PreviewExample, String>> = PreviewCatalog.components.flatMap { component ->
        component.states.filter { it.startsWith("theatre-") }.map { state ->
            requireNotNull(PreviewExample.fromId(component.id)) to state
        }
    }

    @Test
    fun the_catalog_registers_every_theatre_specimen_the_gallery_dispatches() {
        assertEquals(11, specimens.size, specimens.toString())
        assertEquals(specimens.size, specimens.map { it.second }.distinct().size, "state ids are unique across components")
    }

    @Test
    fun every_registered_theatre_specimen_renders_light_and_dark_at_phone_width_and_at_double_text() {
        listOf(
            UiPreferences(theme = Theme.Light) to 390,
            UiPreferences(theme = Theme.Dark) to 390,
            UiPreferences(theme = Theme.Light, textScale = 2f) to 390,
            UiPreferences(theme = Theme.Dark, textScale = 2f) to 320,
            UiPreferences(theme = Theme.Light) to 1100,
        ).forEach { (preferences, width) ->
            specimens.forEach { (example, state) -> capture(example, state, preferences, width) }
        }
    }

    private fun capture(example: PreviewExample, state: String, preferences: UiPreferences, width: Int) {
        val name = "cmp-theatre-${example.id}-$state-${preferences.theme.tokenName}-text${preferences.textScale}-$width"
        runDesktopComposeUiTest(width = width, height = 1800) {
            setContent {
                UiRoot(preferences = preferences) {
                    Column(Modifier.testTag("scene").padding(tokenDp("space.md"))) {
                        ComponentExample(example, state, {}, Modifier)
                    }
                }
            }
            waitForIdle()
            if (example == PreviewExample.Logo) {
                onNodeWithContentDescription("Tài khoản của bạn")
                    .assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp)
                val scene = onNodeWithTag("scene").fetchSemanticsNode().boundsInRoot
                val action = onNodeWithContentDescription("Chuyển giao diện").fetchSemanticsNode().boundsInRoot
                assertTrue(action.left >= scene.left && action.right <= scene.right, "header actions stay inside the frame")
            }
            val image = onNodeWithTag("scene").captureToImage()
            assertTrue(image.width > 0 && image.height > 0, name)
            writeCapture(
                CaptureScenario(
                    name = name, section = "theatre-foundation", component = example.id, state = state,
                    themeMode = if (preferences.theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light, systemIsDark = false,
                    preferences = preferences, viewportWidthDp = width, viewportHeightDp = 1800,
                    hostDensity = 1f, hostFontScale = 1f,
                ),
                image,
            )
        }
    }
}
