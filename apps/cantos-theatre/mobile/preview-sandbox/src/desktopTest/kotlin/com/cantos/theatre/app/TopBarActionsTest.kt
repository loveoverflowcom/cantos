package com.cantos.theatre.app

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertHeightIsEqualTo
import androidx.compose.ui.test.assertWidthIsEqualTo
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.assertIsSelected
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
import com.cantos.theatre.presentation.model.ShellThemeMode
import com.cantos.theatre.presentation.model.ShellWorkId
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** The Theatre top bar's three controls share one 48 dp centerline and one accessible-name owner. */
@OptIn(ExperimentalTestApi::class)
class TopBarActionsTest {
    private val noSelection = ShellEpisodeId("none", ShellWorkId("none"))

    private fun state(theme: ShellTheme, vararg stack: ShellRoute) =
        ShellState(stack = stack.toList(), theme = theme, selectedEpisode = noSelection)

    @Test
    fun brand_theme_and_account_share_one_centerline_in_both_themes_and_the_controls_are_48_dp() {
        listOf(ShellTheme.Dark, ShellTheme.Light).forEach { theme ->
            runDesktopComposeUiTest(width = 390, height = 844) {
                setContent { CantosPresentation(state(theme, ShellRoute.Discovery), ShellCatalogView(), {}) }
                val logo = onNodeWithTag("header-logo", useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
                val themeBox = onNodeWithTag("theme-toggle").fetchSemanticsNode().boundsInRoot
                val account = onNodeWithTag("account").fetchSemanticsNode().boundsInRoot
                assertEquals(logo.center.y, themeBox.center.y, 1f)
                assertEquals(themeBox.center.y, account.center.y, 1f)
                assertTrue(themeBox.right <= account.left, "theme $themeBox must not overlap account $account")
                onNodeWithTag("theme-toggle").assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp)
                onNodeWithTag("account").assertWidthIsEqualTo(48.dp).assertHeightIsEqualTo(48.dp)
            }
        }
    }

    @Test
    fun account_is_one_named_action_that_keeps_its_navigation_and_theme_keeps_its_intent() =
        runDesktopComposeUiTest(width = 390, height = 844) {
            val events = mutableListOf<ShellEvent>()
            setContent { CantosPresentation(state(ShellTheme.Dark, ShellRoute.Discovery), ShellCatalogView(), events::add) }
            onNodeWithTag("account").assertHasClickAction().assertContentDescriptionEquals("Tài khoản").performClick()
            onNodeWithTag("theme-toggle").assertHasClickAction().performClick()
            assertEquals(
                listOf<ShellEvent>(ShellEvent.Navigate(ShellRoute.Account), ShellEvent.SetThemeMode(ShellThemeMode.Light)),
                events.toList(),
            )
        }

    @Test
    fun drill_down_routes_lead_with_back_and_keep_the_same_trailing_actions_without_a_second_brand_mark() =
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                CantosPresentation(state(ShellTheme.Light, ShellRoute.Discovery, ShellRoute.Settings), ShellCatalogView(), {})
            }
            onNodeWithTag("back").assertIsDisplayed()
            onNodeWithTag("header-logo", useUnmergedTree = true).assertDoesNotExist()
            val themeBox = onNodeWithTag("theme-toggle").fetchSemanticsNode().boundsInRoot
            val account = onNodeWithTag("account").fetchSemanticsNode().boundsInRoot
            assertEquals(themeBox.center.y, account.center.y, 1f)
        }

    @Test
    fun large_text_on_a_narrow_screen_keeps_both_trailing_actions_reachable_and_apart() =
        runDesktopComposeUiTest(width = 320, height = 568) {
            setContent {
                CantosPresentation(state(ShellTheme.Dark, ShellRoute.Discovery), ShellCatalogView(), {}, textScale = 2f)
            }
            onNodeWithTag("theme-toggle").assertIsDisplayed()
            onNodeWithTag("account").assertIsDisplayed()
            val themeBox = onNodeWithTag("theme-toggle").fetchSemanticsNode().boundsInRoot
            val account = onNodeWithTag("account").fetchSemanticsNode().boundsInRoot
            assertTrue(themeBox.right <= account.left || themeBox.bottom <= account.top)
        }

    @Test
    fun the_header_cycles_the_selected_mode_and_settings_retains_all_registry_locales() =
        runDesktopComposeUiTest(width = 320, height = 568) {
            val events = mutableListOf<ShellEvent>()
            setContent {
                CantosPresentation(
                    state(ShellTheme.Dark, ShellRoute.Discovery, ShellRoute.Settings),
                    ShellCatalogView(),
                    events::add,
                    themeMode = ShellThemeMode.System,
                )
            }
            onNodeWithTag("theme-toggle").assertContentDescriptionEquals("Hệ thống · Chuyển sang giao diện tối").performClick()
            assertEquals(listOf<ShellEvent>(ShellEvent.SetThemeMode(ShellThemeMode.Dark)), events)
            onNodeWithTag("setting-Theme").performClick()
            onNodeWithTag("theme-System").assertIsSelected()
            onNodeWithTag("preference-close").performClick()
            onNodeWithTag("setting-Language").performClick()
            onNodeWithTag("locale-Vietnamese").assertIsSelected()
            onNodeWithTag("locale-English").assertIsDisplayed()
            onNodeWithText("C", substring = false).assertDoesNotExist()
        }
}
