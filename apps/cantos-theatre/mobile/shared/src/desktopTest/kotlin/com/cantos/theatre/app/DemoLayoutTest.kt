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
import com.cantos.theatre.adapters.mock.DemoFixtures
import com.cantos.theatre.demo.DemoCatalogView
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoEvent
import com.cantos.theatre.demo.DemoProgress
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.demo.DemoWorkId
import com.cantos.theatre.features.shell.presentation.CantosDemoPresentation
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

@OptIn(ExperimentalTestApi::class)
class DemoLayoutTest {
    @Test
    fun settings_options_are_scoped_to_a_dismissible_dialog_and_restore_opener_focus() {
        val events = mutableListOf<DemoEvent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosDemoPresentation(DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Settings)),
                        DemoCatalogView(DemoFixtures.works), { events += it })
                }
            }
            onNodeWithTag("theme-Dark").assertDoesNotExist()
            onNodeWithTag("setting-Theme").performScrollTo()
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button)).performClick()
            onNodeWithTag("preference-dialog").assertIsDisplayed()
            onNodeWithTag("theme-Light").assertIsFocused()
            onNodeWithTag("theme-Dark").performClick()
            assertEquals<List<DemoEvent>>(listOf(DemoEvent.SetTheme(DemoTheme.Dark)), events)
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
                    CantosDemoPresentation(DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Player)),
                        DemoCatalogView(DemoFixtures.works), {}, textScale = 2f)
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
        var state by mutableStateOf(DemoState())
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosDemoPresentation(state, DemoCatalogView(DemoFixtures.works), {})
                }
            }
            onNodeWithText("Sơn Hà Ký", useUnmergedTree = true).assertIsDisplayed()
            runOnIdle {
                state = state.copy(stack = listOf(DemoRoute.Library),
                    savedWorks = setOf(DemoWorkId.SonHa),
                    progress = listOf(
                        DemoProgress(DemoEpisodeId.SonHa03, 768),
                        DemoProgress(DemoEpisodeId.DenKhua01, 91),
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
                    CantosDemoPresentation(DemoState(stack = listOf(DemoRoute.Discovery,
                        DemoRoute.Work(DemoWorkId.SonHa))),
                        DemoCatalogView(DemoFixtures.works), {}, textScale = 2f)
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

    @Test
    fun account_space_stacks_profile_status_sign_in_and_settings_above_the_shell() {
        val events = mutableListOf<DemoEvent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosDemoPresentation(DemoState(stack = listOf(DemoRoute.Library, DemoRoute.Account)),
                        DemoCatalogView(DemoFixtures.works), { events += it })
                }
            }
            val title = onNodeWithTag("header-title").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
            val avatar = onNodeWithTag("account-avatar").fetchSemanticsNode().boundsInRoot
            val preview = onNodeWithTag("account-preview").fetchSemanticsNode().boundsInRoot
            val signIn = onNodeWithTag("sign-in").fetchSemanticsNode().boundsInRoot
            val settings = onNodeWithTag("settings").assertIsDisplayed()
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
                .fetchSemanticsNode().boundsInRoot
            val mini = onNodeWithTag("mini-player").fetchSemanticsNode().boundsInRoot
            val tabs = onNodeWithTag("nav-library").fetchSemanticsNode().boundsInRoot
            assertEquals(64f * density.density, avatar.width)
            assertEquals(avatar.width, avatar.height)
            assertTrue(title.bottom <= avatar.top)
            assertTrue(avatar.bottom <= preview.top)
            assertTrue(preview.bottom <= signIn.top)
            assertTrue(signIn.bottom <= settings.top)
            assertTrue(settings.bottom <= mini.top)
            assertTrue(mini.bottom <= tabs.top)
            onNodeWithTag("account").assertDoesNotExist()
            onNodeWithTag("settings").performClick()
            assertEquals<List<DemoEvent>>(listOf(DemoEvent.Navigate(DemoRoute.Settings)), events)
        }
    }

    @Test
    fun account_space_scrolls_above_the_shell_on_a_short_large_text_screen() {
        runDesktopComposeUiTest(width = 320, height = 568) {
            setContent {
                Box(Modifier.requiredSize(320.dp, 568.dp)) {
                    CantosDemoPresentation(DemoState(stack = listOf(DemoRoute.Library, DemoRoute.Account)),
                        DemoCatalogView(DemoFixtures.works), {}, textScale = 2f)
                }
            }
            onNodeWithTag("settings").performScrollTo().assertIsDisplayed()
            val settings = onNodeWithTag("settings").fetchSemanticsNode().boundsInRoot
            val mini = onNodeWithTag("mini-open").fetchSemanticsNode().boundsInRoot
            assertTrue(settings.bottom <= mini.top)
            onNodeWithTag("back").assertIsDisplayed()
            onNodeWithTag("nav-discovery").assertIsDisplayed()
            onNodeWithTag("nav-library").assertIsDisplayed()
        }
    }
}
