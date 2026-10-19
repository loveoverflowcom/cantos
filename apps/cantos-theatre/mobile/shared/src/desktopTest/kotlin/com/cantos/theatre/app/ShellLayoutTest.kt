package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.SampleEpisodeIds
import com.cantos.theatre.presentation.model.SampleWorkIds
import com.cantos.theatre.presentation.model.sampleState
import com.cantos.theatre.presentation.model.sampleWorks
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

@OptIn(ExperimentalTestApi::class)
class ShellLayoutTest {
    @Test
    fun settings_options_are_scoped_to_a_dismissible_dialog_and_restore_opener_focus() {
        val events = mutableListOf<ShellEvent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosPresentation(sampleState(stack = listOf(ShellRoute.Discovery, ShellRoute.Settings)),
                        ShellCatalogView(sampleWorks), { events += it })
                }
            }
            onNodeWithTag("theme-Dark").assertDoesNotExist()
            onNodeWithTag("setting-Theme").performScrollTo()
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button)).performClick()
            onNodeWithTag("preference-dialog").assertIsDisplayed()
            onNodeWithTag("theme-Light").assertIsFocused()
            onNodeWithTag("theme-Dark").performClick()
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.SetTheme(ShellTheme.Dark)), events)
            onNodeWithTag("preference-dialog").assertDoesNotExist()
            onNodeWithTag("setting-Theme").assertIsFocused().performClick()
            onNodeWithTag("theme-Light").performKeyInput { pressKey(Key.Escape) }
            onNodeWithTag("preference-dialog").assertDoesNotExist()
            onNodeWithTag("setting-Theme").assertIsFocused()
            onNodeWithTag("screen").assertExists()
            assertEquals(1, events.size)
        }
    }

    @Test
    fun transport_actions_keep_left_center_right_order_and_full_targets_at_double_text() {
        runDesktopComposeUiTest(width = 320, height = 568) {
            setContent {
                Box(Modifier.requiredSize(320.dp, 568.dp)) {
                    CantosPresentation(sampleState(stack = listOf(ShellRoute.Discovery, ShellRoute.Player)),
                        ShellCatalogView(sampleWorks), {}, textScale = 2f)
                }
            }
            onNodeWithTag("transport").performScrollTo().assertIsDisplayed()
            val controls = listOf("rewind", "transport", "forward").map {
                onNodeWithTag(it).assertIsDisplayed().fetchSemanticsNode().boundsInRoot
            }
            controls.forEach { assertTrue(it.width >= 48f && it.height >= 48f) }
            assertTrue(controls[0].right <= controls[1].left)
            assertTrue(controls[1].right <= controls[2].left)
            assertTrue(controls.maxOf { it.top } < controls.minOf { it.bottom })
        }
    }

    @Test
    fun featured_and_first_saved_work_titles_are_visible_before_scrolling() {
        var state by mutableStateOf(sampleState())
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosPresentation(state, ShellCatalogView(sampleWorks), {})
                }
            }
            onNodeWithText("Sơn Hà Ký", useUnmergedTree = true).assertIsDisplayed()
            runOnIdle {
                state = state.copy(stack = listOf(ShellRoute.Library),
                    savedWorks = setOf(SampleWorkIds.SonHa),
                    progress = listOf(
                        ShellProgress(SampleEpisodeIds.SonHa03, 768),
                        ShellProgress(SampleEpisodeIds.DenKhua01, 91),
                    ),
                )
            }
            onNodeWithText("Sơn Hà Ký", useUnmergedTree = true).assertIsDisplayed()
        }
    }

    @Test
    fun large_text_episode_and_shell_controls_remain_reachable_on_a_short_screen() {
        runDesktopComposeUiTest(width = 320, height = 568) {
            setContent {
                Box(Modifier.requiredSize(320.dp, 568.dp)) {
                    CantosPresentation(sampleState(stack = listOf(ShellRoute.Discovery,
                        ShellRoute.Work(SampleWorkIds.SonHa))),
                        ShellCatalogView(sampleWorks), {}, textScale = 2f)
                }
            }
            onNodeWithTag("episode-SonHa03").performScrollTo().assertIsDisplayed()
            val row = onNodeWithTag("episode-SonHa03").fetchSemanticsNode().boundsInRoot
            val mini = onNodeWithTag("mini-open").fetchSemanticsNode().boundsInRoot
            assertTrue(row.bottom <= mini.top)
            onNodeWithTag("back").assertIsDisplayed()
            onNodeWithTag("account").assertIsDisplayed()
            onNodeWithTag("nav-discovery").assertIsDisplayed()
            onNodeWithTag("nav-library").assertIsDisplayed()
        }
    }
}
