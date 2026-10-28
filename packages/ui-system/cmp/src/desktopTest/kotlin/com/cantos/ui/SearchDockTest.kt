package com.cantos.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import org.junit.Test
import kotlin.test.assertEquals

/** Real shared component on the JVM renderer; no native keyboard, OS or device claim. */
@OptIn(ExperimentalTestApi::class)
class SearchDockTest {
    @Test
    fun collapse_retains_raw_query_clear_restores_focus_and_header_does_not_move() {
        for (theme in listOf(Theme.Light, Theme.Dark)) {
            runDesktopComposeUiTest(width = 390, height = 844) {
                setContent {
                    BundledPreviewCaptureRoot(preferences = UiPreferences(theme = theme, reducedMotion = true, expression = CantosExpression.Theatre)) {
                        var query by remember { mutableStateOf("") }
                        var expanded by remember { mutableStateOf(false) }
                        var restoring by remember { mutableStateOf(false) }
                        val focus = remember { FocusRequester() }
                        TextField("Tìm truyện", query, { query = it; expanded = true },
                            modifier = Modifier.testTag("shell"),
                            controlModifier = Modifier.testTag("query").focusRequester(focus)
                                .onFocusChanged { if (it.isFocused && !restoring) expanded = true },
                            variant = TextFieldVariant.FloatingSearch,
                            dockExpanded = expanded,
                            dockContent = {
                                Column(Modifier.heightIn(max = 300.dp).testTag("body")) {
                                    Button("Thu gọn", {
                                        restoring = true; focus.requestFocus(); expanded = false; restoring = false
                                    }, controlModifier = Modifier.testTag("collapse"))
                                }
                            },
                            trailing = {
                                IconButton(AccessibleName.of("Xóa nội dung"), "close", {
                                    query = ""; focus.requestFocus(); expanded = true
                                }, controlModifier = Modifier.size(48.dp).testTag("clear"))
                            },
                        )
                    }
                }
                val resting = onNodeWithTag("query").fetchSemanticsNode().boundsInRoot
                onNodeWithTag("query").performClick().performTextReplacement("  Quán Trà  ")
                waitForIdle()
                val open = onNodeWithTag("query").fetchSemanticsNode().boundsInRoot
                assertEquals(resting, open, "the same input bounds anchor the body")
                writeCapture(CaptureScenario(name = "search-dock-${theme.tokenName}-390", section = "theatre-foundation",
                    component = "text-field", state = "theatre-docked-search", themeMode = if (theme == Theme.Dark) ThemeMode.Dark else ThemeMode.Light,
                    systemIsDark = false, preferences = UiPreferences(theme = theme, reducedMotion = true, expression = CantosExpression.Theatre),
                    viewportWidthDp = 390, viewportHeightDp = 844, hostDensity = 1f, hostFontScale = 1f, fontSource = BundledPreviewFontSource), onNodeWithTag("shell").captureToImage())
                onNodeWithTag("collapse").performClick()
                onNodeWithTag("body").assertDoesNotExist()
                onNodeWithTag("query").assertIsFocused().assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("  Quán Trà  ")))
                onNodeWithTag("clear").performClick()
                onNodeWithTag("query").assertIsFocused().assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("")))
                onNodeWithTag("body").assertExists()
            }
        }
    }
}
