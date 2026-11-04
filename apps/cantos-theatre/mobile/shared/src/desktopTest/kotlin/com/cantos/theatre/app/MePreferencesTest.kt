package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.shell.presentation.MePreferences
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellThemeMode
import com.cantos.ui.CantosTheme
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import java.io.File
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/** Actual JVM production rows/dialog. A controlled value stays unchanged until its owner acknowledges it. */
@OptIn(ExperimentalTestApi::class)
class MePreferencesTest {
    @Test
    fun compact_controls_keep_targets_names_values_and_restore_focus_after_escape_or_choice() {
        for (theme in Theme.entries) for (locale in ShellLocale.entries) for ((width, scale) in listOf(390 to 1f, 320 to 2f)) runDesktopComposeUiTest(width = width, height = 800) {
            val events = mutableListOf<ShellEvent>()
            val state = ShellState(locale = locale, selectedEpisode = ShellEpisodeId("none", ShellWorkId("none")))
            setContent {
                CantosTheme(UiPreferences(theme = theme, textScale = scale)) {
                    Box(Modifier.requiredSize(width.dp, 800.dp).testTag("preference-capture")) {
                        Column(Modifier.padding(horizontal = 16.dp).verticalScroll(rememberScrollState())) {
                            MePreferences(state, ShellCopy(locale), events::add, ShellThemeMode.System)
                        }
                    }
                }
            }
            for (tag in listOf("setting-Theme", "setting-Language")) {
                val box = onNodeWithTag(tag).performScrollTo()
                    .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
                    .fetchSemanticsNode().boundsInRoot
                assertTrue(box.width >= 48f && box.height >= 48f)
                assertTrue(box.left >= 0f && box.right <= width.toFloat())
            }
            val copy = ShellCopy(locale)
            onNodeWithTag("setting-Theme")
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.ContentDescription, listOf(copy.text(CopyKey.Theme))))
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, copy.text(CopyKey.System)))
            onNodeWithTag("setting-Language")
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.ContentDescription, listOf(copy.text(CopyKey.PreferenceLanguage))))
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, locale.autonym))
            if (scale == 1f) {
                val theme = onNodeWithTag("setting-Theme").fetchSemanticsNode().boundsInRoot
                val language = onNodeWithTag("setting-Language").fetchSemanticsNode().boundsInRoot
                assertEquals(theme.top, language.top, "Both choices share a row at normal text size")
                assertTrue(theme.right <= language.left)
            }
            System.getenv("CANTOS_ME_PREF_CAPTURE_DIR")?.let { path ->
                val output = File(path)
                check(output.isDirectory || output.mkdirs())
                val bitmap = onNodeWithTag("preference-capture").captureToImage().asSkiaBitmap()
                val bytes = requireNotNull(Image.makeFromBitmap(bitmap).encodeToData(EncodedImageFormat.PNG)).bytes
                File(output, "${theme.name}-${locale.name}-$width-$scale.png").writeBytes(bytes)
            }
            onNodeWithTag("setting-Theme").performScrollTo().performClick()
            onNodeWithTag("theme-System").assertIsSelected().assertIsFocused()
            onNodeWithTag("theme-System").performKeyInput { pressKey(Key.Escape) }
            onNodeWithTag("preference-dialog").assertDoesNotExist()
            onNodeWithTag("setting-Theme").assertIsFocused().performClick()
            onNodeWithTag("theme-Dark").performClick()
            onNodeWithTag("setting-Theme").assertIsFocused()
            assertEquals(listOf<ShellEvent>(ShellEvent.SetThemeMode(ShellThemeMode.Dark)), events)
            // The callback alone cannot claim that the controlled preference changed.
            onNodeWithTag("setting-Theme").performClick()
            onNodeWithTag("theme-System").assertIsSelected()
            onNodeWithTag("preference-close").performClick()
            onNodeWithTag("setting-Language").performScrollTo().performClick()
            onNodeWithTag("locale-Vietnamese").assert(hasText("Tiếng Việt"))
            onNodeWithTag("locale-English").assert(hasText("English"))
            onNodeWithTag("locale-$locale").assertIsSelected().assertIsFocused()
            onNodeWithTag("preference-close").performClick()
            onNodeWithTag("setting-Language").assertIsFocused()
            assertEquals(1, events.size, "Dismissal emits no preference mutation")
            val other = ShellLocale.entries.first { it != locale }
            onNodeWithTag("setting-Language").performClick()
            onNodeWithTag("locale-$other").performClick()
            assertEquals(listOf(ShellEvent.SetThemeMode(ShellThemeMode.Dark), ShellEvent.SetLocale(other)), events)
            onNodeWithTag("setting-Language").assertIsFocused().performClick()
            onNodeWithTag("locale-$locale").assertIsSelected()
        }
    }

    @Test
    fun acknowledged_theme_modes_are_reflected_in_the_control_and_dialog() {
        for (mode in ShellThemeMode.entries) runDesktopComposeUiTest(width = 390, height = 800) {
            val events = mutableListOf<ShellEvent>()
            val state = ShellState(locale = ShellLocale.Vietnamese, selectedEpisode = ShellEpisodeId("none", ShellWorkId("none")))
            val copy = ShellCopy(state.locale)
            val value = copy.text(when (mode) {
                ShellThemeMode.Light -> CopyKey.PreferenceLight
                ShellThemeMode.Dark -> CopyKey.PreferenceDark
                ShellThemeMode.System -> CopyKey.System
            })
            setContent { CantosTheme(UiPreferences()) { MePreferences(state, copy, events::add, mode) } }
            onNodeWithTag("setting-Theme")
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, value))
                .performClick()
            onNodeWithTag("theme-$mode").assertIsSelected()
            assertTrue(events.isEmpty(), "Opening a controlled chooser sends no preference mutation")
        }
    }
}
