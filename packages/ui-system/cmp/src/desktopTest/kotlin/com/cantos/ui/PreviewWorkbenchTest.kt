package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.Column
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithText
import com.cantos.ui.preview.ComponentWorkbench
import com.cantos.ui.preview.RecipeFrame
import com.cantos.ui.preview.workbench.ConstraintFrame
import com.cantos.ui.preview.workbench.PreviewMode
import com.cantos.ui.preview.workbench.PreviewRecipe
import com.cantos.ui.preview.workbench.PreviewRecipeResult
import com.cantos.ui.preview.workbench.PreviewSettings
import com.cantos.ui.preview.workbench.SyntheticPreviewText
import java.io.File
import org.junit.Test

@OptIn(ExperimentalTestApi::class)
class PreviewWorkbenchTest {
    @Test
    fun an_interactive_focus_recipe_uses_real_focus_and_the_matrix_cannot_trap_a_modal() {
        runDesktopComposeUiTest(width = 840, height = 1000) {
            setContent { RecipeFrame(recipe("text-field", "focus"), interactive = true) }
            onNodeWithText("Tên tác phẩm").assertIsFocused()
        }
        runDesktopComposeUiTest(width = 840, height = 1000) {
            setContent { RecipeFrame(recipe("dialog", "open", PreviewMode.StateMatrix)) }
            onNodeWithText("Open this modal in Isolated mode; an immutable modal would block the matrix controls.").assertExists()
            onAllNodesWithText("Đóng").assertCountEquals(0)
        }
    }
    @Test
    fun gallery_field_values_survive_availability_state_changes() {
        runDesktopComposeUiTest(width = 840, height = 1000) {
            setContent {
                var state by remember { mutableStateOf("editable") }
                BundledPreviewCaptureRoot { Column {
                    Button("Change availability", { state = "readonly" })
                    com.cantos.ui.preview.ComponentExample(PreviewExample.TextField, state, {}, Modifier)
                } }
            }
            onNodeWithText("Tên tác phẩm").performTextReplacement(SyntheticPreviewText.alternateTitleVi)
            onNodeWithText("Change availability").performClick()
            onAllNodesWithText(SyntheticPreviewText.alternateTitleVi).assertCountEquals(2)
        }
    }
    @Test
    fun actual_button_intents_survive_replay_and_recreate_then_reset_explicitly() {
        val recipe = recipe("button", "enabled")
        runDesktopComposeUiTest(width = 1140, height = 2200) {
            setContent { BundledPreviewCaptureRoot { ComponentWorkbench(recipe) } }
            onNodeWithText("Lưu · Primary").performScrollTo().performClick()
            onNodeWithText("Callback count: 1").assertExists()
            onNodeWithText("Replay intents").performScrollTo().performClick()
            onNodeWithText("Callback count: 1").assertExists()
            onNodeWithText("Recreate").performScrollTo().performClick()
            onNodeWithText("Callback count: 1").assertExists()
            onNodeWithText("Reset to selected").performScrollTo().performClick()
            onNodeWithText("Callback count: 0").assertExists()
        }
    }

    @Test
    fun filtering_does_not_unmount_the_edited_field_and_snapshot_restores_it() {
        val recipe = recipe("text-field", "selection")
        runDesktopComposeUiTest(width = 1140, height = 2200) {
            setContent { BundledPreviewCaptureRoot { ComponentWorkbench(recipe) } }
            onNodeWithText("Tên tác phẩm").performScrollTo().performTextReplacement(SyntheticPreviewText.alternateTitleVi)
            onNodeWithText("Filter recipes").performScrollTo().performTextReplacement("checkbox")
            onNodeWithText(SyntheticPreviewText.alternateTitleVi).assertExists()
            onNodeWithText("Filter recipes").performScrollTo().performTextReplacement("no-matching-component")
            onNodeWithText(SyntheticPreviewText.alternateTitleVi).assertExists()
            onNodeWithText("Export snapshot").performScrollTo().performClick()
            onNodeWithText("Synthetic snapshot exported").assertExists()
            onNodeWithText("Reset to selected").performScrollTo().performClick()
            onNodeWithText("Restore snapshot").performScrollTo().performClick()
            onNodeWithText(SyntheticPreviewText.alternateTitleVi).assertExists()
            onNodeWithText("Restored validated synthetic snapshot").assertExists()
        }
    }

    @Test
    fun immutable_recipe_frames_are_captured_with_semantics_and_actual_constraints() {
        capture("cmp-recipe-light-390-field", recipe("text-field", "selection"))
        capture("cmp-recipe-dark-320-text2-button", recipe("button", "auth-primary",
            settings = PreviewSettings(themeMode = ThemeMode.Dark, fontScale = 2f, reducedMotion = true,
                constraints = ConstraintFrame(320, 844))))
        capture("cmp-recipe-system-dark-840-text1-3-matrix", recipe("checkbox", "selected", PreviewMode.StateMatrix,
            PreviewSettings(themeMode = ThemeMode.System, fontScale = 1.3f, constraints = ConstraintFrame(840, 1000))), systemIsDark = true)
    }

    @Test
    fun workbench_controls_are_captured_in_the_real_gallery_root() {
        val recipe = recipe("button", "enabled")
        runDesktopComposeUiTest(width = 1140, height = 2200) {
            var density = 0f
            var fontScale = 0f
            setContent {
                density = LocalDensity.current.density
                fontScale = LocalDensity.current.fontScale
                Box(Modifier.requiredSize(1140.dp, 2200.dp).testTag("workbench-root")) {
                    com.cantos.ui.preview.PreviewApp(initialSection = 3, initialRecipe = recipe,
                        uiFontFamily = com.cantos.ui.preview.PreviewFonts.ui,
                        editorialFontFamily = com.cantos.ui.preview.PreviewFonts.editorial)
                }
            }
            val name = "cmp-workbench-light-1140-controls"
            writeCapture(CaptureScenario(name, "workbench", "button", "enabled", themeMode = ThemeMode.Light,
                systemIsDark = false, preferences = UiPreferences(), viewportWidthDp = 1140, viewportHeightDp = 2200,
                hostDensity = density, hostFontScale = fontScale, fontSource = BundledPreviewFontSource), onNodeWithTag("workbench-root").captureToImage())
            File(captureDirectory(), "$name.semantics.txt").writeText(onNodeWithTag("workbench-root").printToString())
        }
    }

    private fun capture(name: String, recipe: PreviewRecipe, systemIsDark: Boolean = false) {
        val frame = recipe.settings.constraints
        runDesktopComposeUiTest(width = frame.widthDp, height = frame.heightDp) {
            var density = 0f
            var fontScale = 0f
            setContent {
                density = LocalDensity.current.density
                fontScale = LocalDensity.current.fontScale
                CantosTheme(UiPreferences(), com.cantos.ui.preview.PreviewFonts.ui,
                    com.cantos.ui.preview.PreviewFonts.editorial) {
                    RecipeFrame(recipe, systemIsDark = systemIsDark)
                }
            }
            val node = onNodeWithTag("recipe-frame")
            writeCapture(CaptureScenario(name, recipe.mode.name, recipe.component.id, recipe.stateId,
                themeMode = recipe.settings.themeMode, systemIsDark = systemIsDark,
                preferences = recipe.settings.preferences(systemIsDark), viewportWidthDp = frame.widthDp,
                viewportHeightDp = frame.heightDp, hostDensity = density, hostFontScale = fontScale, fontSource = BundledPreviewFontSource), node.captureToImage())
            File(captureDirectory(), "$name.semantics.txt").writeText(node.printToString())
        }
    }

    private fun recipe(id: String, state: String, mode: PreviewMode = PreviewMode.Isolated,
        settings: PreviewSettings = PreviewSettings()): PreviewRecipe =
        (PreviewRecipe.of(id, state, mode, settings = settings) as PreviewRecipeResult.Accepted).recipe
}
