package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performSemanticsAction
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
import com.cantos.ui.UiPreferences
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

/** Actual JVM production rows/dialog. A controlled value stays unchanged until its owner acknowledges it. */
@OptIn(ExperimentalTestApi::class)
class MePreferencesTest {
    @Test
    fun compact_rows_keep_full_targets_names_system_and_autonyms_and_restore_focus_after_escape_or_choice() {
        for (locale in ShellLocale.entries) runDesktopComposeUiTest(width = 320, height = 800) {
            val events = mutableListOf<ShellEvent>()
            val state = ShellState(locale = locale, selectedEpisode = ShellEpisodeId("none", ShellWorkId("none")))
            setContent {
                CantosTheme(UiPreferences(textScale = 2f)) {
                    Box(Modifier.requiredSize(320.dp, 800.dp)) {
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
                assertTrue(box.left >= 0f && box.right <= 320f)
            }
            if (locale == ShellLocale.English) {
                onNodeWithText(ShellCopy(locale).text(CopyKey.Theme), useUnmergedTree = true)
                    .performSemanticsAction(SemanticsActions.GetTextLayoutResult) { read ->
                        val layouts = mutableListOf<TextLayoutResult>()
                        assertTrue(read(layouts))
                        assertEquals(1, layouts.single().lineCount, "Appearance stays a whole word at 320dp / 200%")
                    }
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
        }
    }
}
