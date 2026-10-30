package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.adapters.preview.PreviewEpisodeIds
import com.cantos.theatre.app.PreviewSeed
import com.cantos.theatre.app.PreviewTestRoot
import com.cantos.theatre.app.previewState
import com.cantos.theatre.presentation.model.ShellRoute
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

@OptIn(ExperimentalTestApi::class)
class PreviewLayoutTest {
    @Test
    fun account_space_stacks_profile_membership_and_settings_above_the_shell() {
        // Tall enough for every card at once: boundsInRoot is clipped to the scroll viewport.
        runDesktopComposeUiTest(width = 390, height = 1500) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 1500.dp)) {
                    PreviewTestRoot(accountSpace())
                }
            }
            val title = onNodeWithTag("header-title").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
            val profile = onNodeWithTag("account-profile").fetchSemanticsNode().boundsInRoot
            val avatar = onNodeWithTag("account-avatar").fetchSemanticsNode().boundsInRoot
            val signIn = onNodeWithTag("sign-in").fetchSemanticsNode().boundsInRoot
            val membership = onNodeWithTag("account-membership").fetchSemanticsNode().boundsInRoot
            val group = onNodeWithTag("account-settings-group").fetchSemanticsNode().boundsInRoot
            assertEquals(40f * density.density, avatar.width)
            assertEquals(avatar.width, avatar.height)
            assertTrue(title.bottom <= profile.top)
            // Both guest auth actions remain inside the compact profile card.
            assertTrue(profile.top <= avatar.top && avatar.bottom <= signIn.top)
            assertTrue(signIn.bottom <= profile.bottom)
            assertTrue(profile.bottom <= membership.top)
            assertTrue(membership.bottom <= group.top)
            onNodeWithTag("account").assertDoesNotExist()
            // The shell stays below the scrolling account content.
            val settings = onNodeWithTag("settings").performScrollTo().assertIsDisplayed()
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
                .fetchSemanticsNode().boundsInRoot
            val mini = onNodeWithTag("mini-player").fetchSemanticsNode().boundsInRoot
            val tabs = onNodeWithTag("nav-library").fetchSemanticsNode().boundsInRoot
            assertTrue(settings.bottom <= mini.top)
            assertTrue(mini.bottom <= tabs.top)
            onNodeWithTag("settings").performClick()
            onNodeWithTag("setting-Theme").assertIsDisplayed()
        }
    }

    @Test
    fun account_space_scrolls_above_the_shell_on_a_short_large_text_screen() {
        runDesktopComposeUiTest(width = 320, height = 568) {
            setContent {
                Box(Modifier.requiredSize(320.dp, 568.dp)) {
                    PreviewTestRoot(accountSpace(), textScale = 2f)
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

    /** A guest on the Account space with a paused selection, so the mini-player is part of the layout. */
    private fun accountSpace() = PreviewSeed(previewState(
        stack = listOf(ShellRoute.Library, ShellRoute.Account),
        selectedEpisode = PreviewEpisodeIds.SonHa03, positionMs = 768_000L,
    ))
}
