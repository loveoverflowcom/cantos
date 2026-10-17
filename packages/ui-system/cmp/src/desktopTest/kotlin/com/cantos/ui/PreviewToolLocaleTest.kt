package com.cantos.ui

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.ui.preview.PreviewApp
import org.junit.Test

@OptIn(ExperimentalTestApi::class)
class PreviewToolLocaleTest {
    @Test
    fun tool_controls_are_english_while_the_vietnamese_button_specimen_remains_unchanged() {
        runDesktopComposeUiTest(width = 390, height = 1000) {
            setContent { PreviewApp() }
            onNodeWithText("Find component").assertExists()
            onNodeWithText("Lưu · Primary").assertExists()
            onNodeWithText("Find component").performTextReplacement("no-matching-component")
            onNodeWithText("No results").assertExists()
            onNodeWithText("Search by component name or family.").assertExists()
        }
    }
}
