package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.features.shell.presentation.LocalShellCapabilities
import com.cantos.theatre.presentation.model.ShellCapabilities
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryPhase
import com.cantos.theatre.presentation.model.ShellLibrarySection
import com.cantos.theatre.presentation.model.ShellLibraryView
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.sampleState
import org.junit.Test
import kotlin.test.assertEquals

/** JVM production-presentation semantics; supplied facts do not establish native or controller acceptance. */
@OptIn(ExperimentalTestApi::class)
class LibraryEmptyUiTest {
    @Test
    fun one_discover_action_has_a_full_hit_target_retains_focus_on_theme_change_and_dispatches_once() {
        val events = mutableListOf<ShellEvent>()
        var state by mutableStateOf(sampleState(stack = listOf(ShellRoute.Library)))
        runDesktopComposeUiTest(width = 320, height = 800) {
            setContent { Box(Modifier.requiredSize(320.dp, 800.dp)) { CantosPresentation(state, ShellCatalogView(), { events += it }, textScale = 2f) } }
            onAllNodesWithTag("library-empty-discover").assertCountEquals(1)
            val discover = onNodeWithTag("library-empty-discover").performScrollTo()
            discover.assertHeightIsAtLeast(48.dp).assertWidthIsAtLeast(48.dp)
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
            discover.performSemanticsAction(SemanticsActions.RequestFocus) { it() }
            discover.assertIsFocused()
            runOnIdle { state = state.copy(theme = ShellTheme.Dark, locale = ShellLocale.English) }
            discover.assertIsFocused().performKeyInput { pressKey(Key.Enter) }
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.Navigate(ShellRoute.Discovery)), events)
            onNodeWithTag("history-all").assertDoesNotExist()
            onNodeWithTag("library-continue-retry").assertDoesNotExist()
        }
    }

    @Test
    fun every_phase_keeps_the_full_history_empty_retry_and_more_actions_distinct() {
        var phase by mutableStateOf(ShellLibraryPhase.Complete)
        val events = mutableListOf<ShellEvent>()
        val state = sampleState(stack = listOf(ShellRoute.Library, ShellRoute.History))
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { CantosPresentation(state, ShellCatalogView(library = ShellLibraryView(history = phase)), { events += it }) }
            ShellLibraryPhase.entries.forEach { next ->
                runOnIdle { phase = next; events.clear() }
                onAllNodesWithTag("history-empty").assertCountEquals(if (next == ShellLibraryPhase.Complete) 1 else 0)
                onAllNodesWithTag("library-empty-discover").assertCountEquals(if (next == ShellLibraryPhase.Complete) 1 else 0)
                onNodeWithTag("history-clear").assertDoesNotExist()
                when (next) {
                    ShellLibraryPhase.Complete -> {
                        onNodeWithTag("history-retry").assertDoesNotExist()
                        onNodeWithTag("history-more").assertDoesNotExist()
                    }
                    ShellLibraryPhase.Loading, ShellLibraryPhase.LoadingMore -> onNodeWithTag("history-loading").assertExists()
                    ShellLibraryPhase.More -> {
                        onNodeWithTag("history-more").performScrollTo().performClick()
                        assertEquals<List<ShellEvent>>(listOf(ShellEvent.LoadMoreLibrary(ShellLibrarySection.History)), events)
                    }
                    ShellLibraryPhase.Failed, ShellLibraryPhase.MoreFailed, ShellLibraryPhase.Stale -> {
                        onNodeWithTag("history-retry").performScrollTo().performClick()
                        assertEquals<List<ShellEvent>>(listOf(ShellEvent.RetryLibrary(ShellLibrarySection.History)), events)
                    }
                }
            }
        }
    }

    @Test
    fun disabled_bookmarks_explain_unavailability_without_claiming_empty_or_offering_actions() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                CompositionLocalProvider(LocalShellCapabilities provides ShellCapabilities(bookmarks = false)) {
                    CantosPresentation(sampleState(stack = listOf(ShellRoute.Library)), ShellCatalogView(), {})
                }
            }
            onNodeWithTag("library-bookmarks-unavailable").assertExists()
            onNodeWithTag("library-bookmarks-empty").assertDoesNotExist()
            onNodeWithTag("library-bookmarks-retry").assertDoesNotExist()
            onAllNodesWithTag("library-empty-discover").assertCountEquals(1)
        }
    }
}
