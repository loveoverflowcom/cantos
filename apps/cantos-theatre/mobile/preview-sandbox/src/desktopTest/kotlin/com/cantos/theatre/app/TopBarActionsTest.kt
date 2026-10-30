package com.cantos.theatre.app

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertHeightIsEqualTo
import androidx.compose.ui.test.assertWidthIsEqualTo
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellWorkId
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Me owns appearance; the retained Account opener is still needed until native three-root navigation. */
@OptIn(ExperimentalTestApi::class)
class TopBarActionsTest {
    private val noSelection = ShellEpisodeId("none", ShellWorkId("none"))
    private fun state(theme: ShellTheme, vararg stack: ShellRoute) =
        ShellState(stack = stack.toList(), theme = theme, selectedEpisode = noSelection)

    @Test
    fun brand_and_account_keep_one_centerline_and_a_full_target_without_a_duplicate_theme_control() {
        for (theme in ShellTheme.entries) runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { CantosPresentation(state(theme, ShellRoute.Discovery), ShellCatalogView(), {}) }
            val logo = onNodeWithTag("header-logo", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
            val account = onNodeWithTag("account").fetchSemanticsNode().boundsInRoot
            assertEquals(logo.center.y, account.center.y, 1f)
            assertTrue(logo.right <= account.left)
            onNodeWithTag("account").assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp)
            onNodeWithTag("theme-toggle").assertDoesNotExist()
        }
    }

    @Test
    fun large_text_keeps_the_only_header_account_target_full_size_and_reachable() =
        runDesktopComposeUiTest(width = 320, height = 568) {
            setContent { CantosPresentation(state(ShellTheme.Dark, ShellRoute.Discovery), ShellCatalogView(), {}, textScale = 2f) }
            val box = onNodeWithTag("account").assertHasClickAction().fetchSemanticsNode().boundsInRoot
            assertTrue(box.left >= 0f && box.right <= 320f)
            onNodeWithTag("account").assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp)
            onNodeWithTag("theme-toggle").assertDoesNotExist()
        }

    @Test
    fun account_retains_its_one_named_navigation_intent() = runDesktopComposeUiTest(width = 390, height = 844) {
        val events = mutableListOf<ShellEvent>()
        setContent { CantosPresentation(state(ShellTheme.Dark, ShellRoute.Discovery), ShellCatalogView(), events::add) }
        onNodeWithTag("account").assertHasClickAction().assertContentDescriptionEquals("Tài khoản").performClick()
        assertEquals(listOf<ShellEvent>(ShellEvent.Navigate(ShellRoute.Account)), events)
        onNodeWithTag("theme-toggle").assertDoesNotExist()
    }

    @Test
    fun a_drill_down_keeps_back_and_account_targets_and_me_adds_no_empty_action_group() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { CantosPresentation(state(ShellTheme.Light, ShellRoute.Discovery, ShellRoute.Settings), ShellCatalogView(), {}) }
            onNodeWithTag("back").assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp)
            onNodeWithTag("account").assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp)
            onNodeWithTag("theme-toggle").assertDoesNotExist()
        }
        runDesktopComposeUiTest(width = 320, height = 568) {
            setContent { CantosPresentation(state(ShellTheme.Light, ShellRoute.Discovery, ShellRoute.Account), ShellCatalogView(), {}, textScale = 2f) }
            onNodeWithTag("account").assertDoesNotExist()
            onNodeWithTag("theme-toggle").assertDoesNotExist()
            onNodeWithTag("back").assertHasClickAction()
        }
    }
}
