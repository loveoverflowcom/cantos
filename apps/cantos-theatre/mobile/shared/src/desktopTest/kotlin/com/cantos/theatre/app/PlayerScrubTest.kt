package com.cantos.theatre.app

import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.LayoutDirection
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.features.shell.presentation.PlayerTimeline
import com.cantos.theatre.presentation.model.SampleEpisodeIds
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.sampleState
import com.cantos.theatre.presentation.model.sampleWorks
import com.cantos.ui.CantosTheme
import com.cantos.ui.UiPreferences
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

@OptIn(ExperimentalTestApi::class)
class PlayerScrubTest {
    @Test
    fun drag_previews_without_commands_or_tick_override_and_release_seeks_once() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            var state by mutableStateOf(sampleState(stack = listOf(ShellRoute.Player)))
            val events = mutableListOf<ShellEvent>()
            setContent { CantosPresentation(state, ShellCatalogView(sampleWorks), { events += it }) }
            val slider = onNodeWithTag("seek").performScrollTo()
            slider.performTouchInput {
                down(center)
                moveTo(Offset(width * .7f, centerY))
                moveTo(Offset(width * .8f, centerY))
            }
            val draft = slider.fetchSemanticsNode().config[SemanticsProperties.ProgressBarRangeInfo].current
            assertTrue(events.isEmpty(), "Dragging must preview without dispatch: $events")
            runOnIdle { state = state.copy(positionSeconds = 770) }
            assertEquals(draft, slider.fetchSemanticsNode().config[SemanticsProperties.ProgressBarRangeInfo].current)
            slider.performTouchInput { up() }
            assertEquals(listOf<ShellEvent>(ShellEvent.SeekTo(draft.toInt(), 0)), events)
        }
    }

    @Test
    fun component_drag_cancel_reverts_without_seeking() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            val events = mutableListOf<ShellEvent>()
            val interactions = MutableInteractionSource()
            var started: DragInteraction.Start? = null
            setContent {
                LaunchedEffect(interactions) {
                    interactions.interactions.collect { if (it is DragInteraction.Start) started = it }
                }
                CantosTheme(UiPreferences()) {
                    Column { PlayerTimeline(sampleState(), sampleWorks.first().episodes.last(), ShellCopy(ShellLocale.Vietnamese),
                        { events += it }, interactions) }
                }
            }
            val slider = onNodeWithTag("seek")
            slider.performTouchInput { down(center); moveTo(Offset(width * .8f, centerY)) }
            waitUntil(timeoutMillis = 2_000) { started != null }
            // Skiko 1.9.0 enqueueCancel is a no-op. Inject the exact Material cancellation contract instead.
            runOnIdle { assertTrue(interactions.tryEmit(DragInteraction.Cancel(requireNotNull(started)))) }
            waitUntil(timeoutMillis = 2_000) {
                slider.fetchSemanticsNode().config[SemanticsProperties.ProgressBarRangeInfo].current == 768f
            }
            onNodeWithTag("elapsed").assertTextEquals("12:48")
            slider.performTouchInput { up() }
            assertEquals(emptyList(), events)
        }
    }

    @Test
    fun accessibility_set_progress_commits_once_with_bounded_targets() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            val events = mutableListOf<ShellEvent>()
            setContent { CantosPresentation(sampleState(stack = listOf(ShellRoute.Player)), ShellCatalogView(sampleWorks), { events += it }) }
            val slider = onNodeWithTag("seek").performScrollTo()
            slider.performSemanticsAction(SemanticsActions.SetProgress) { it(0f) }
            slider.performSemanticsAction(SemanticsActions.SetProgress) { it(99999f) }
            assertEquals(listOf<ShellEvent>(ShellEvent.SeekTo(0, 0), ShellEvent.SeekTo(1722, 0)), events)
        }
    }

    @Test
    fun keyboard_previews_on_key_down_and_commits_once_on_release() {
        for (rtl in listOf(false, true)) runDesktopComposeUiTest(width = 390, height = 844) {
            val events = mutableListOf<ShellEvent>()
            setContent {
                CompositionLocalProvider(LocalLayoutDirection provides if (rtl) LayoutDirection.Rtl else LayoutDirection.Ltr) {
                    CantosPresentation(sampleState(stack = listOf(ShellRoute.Player)), ShellCatalogView(sampleWorks), { events += it })
                }
            }
            val slider = onNodeWithTag("seek").performScrollTo()
            slider.performSemanticsAction(SemanticsActions.RequestFocus) { it() }
            slider.performKeyInput { keyDown(Key.DirectionRight) }
            assertTrue(events.isEmpty())
            val draft = slider.fetchSemanticsNode().config[SemanticsProperties.ProgressBarRangeInfo].current
            assertTrue(if (rtl) draft < 768f else draft > 768f)
            slider.performKeyInput { keyUp(Key.DirectionRight) }
            assertEquals(listOf<ShellEvent>(ShellEvent.SeekTo(draft.toInt(), 0)), events)
            slider.performKeyInput { pressKey(Key.MoveHome); pressKey(Key.MoveEnd) }
            assertEquals(listOf<ShellEvent>(ShellEvent.SeekTo(draft.toInt(), 0), ShellEvent.SeekTo(0, 0), ShellEvent.SeekTo(1722, 0)), events)
        }
    }

    @Test
    fun a_controller_generation_change_discards_the_drag_without_losing_keyboard_focus() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            val events = mutableListOf<ShellEvent>()
            var state by mutableStateOf(sampleState(stack = listOf(ShellRoute.Player)))
            setContent { CantosPresentation(state, ShellCatalogView(sampleWorks), { events += it }) }
            val slider = onNodeWithTag("seek").performScrollTo()
            slider.performSemanticsAction(SemanticsActions.RequestFocus) { it() }
            slider.performTouchInput { down(center); moveTo(Offset(width * .8f, centerY)) }
            runOnIdle { state = state.copy(playbackGeneration = 1, positionSeconds = 50) }
            assertEquals(50f, slider.fetchSemanticsNode().config[SemanticsProperties.ProgressBarRangeInfo].current)
            slider.performTouchInput { moveTo(Offset(width * .9f, centerY)); up() }
            assertEquals(emptyList(), events)
            slider.assertIsFocused()
        }
    }

    @Test
    fun source_switch_and_route_exit_cannot_commit_an_old_drag() {
        for (leaveRoute in listOf(false, true)) runDesktopComposeUiTest(width = 390, height = 844) {
            val events = mutableListOf<ShellEvent>()
            var state by mutableStateOf(sampleState(stack = listOf(ShellRoute.Player)))
            setContent { CantosPresentation(state, ShellCatalogView(sampleWorks), { events += it }) }
            onNodeWithTag("seek").performScrollTo().performTouchInput { down(center); moveTo(Offset(width * .8f, centerY)) }
            runOnIdle {
                state = if (leaveRoute) state.copy(stack = listOf(ShellRoute.Discovery))
                    else state.copy(selectedEpisode = SampleEpisodeIds.DenKhua01, positionSeconds = 91)
            }
            onNodeWithTag("screen").performTouchInput { up() }
            runOnIdle { state = state.copy(stack = listOf(ShellRoute.Player), selectedEpisode = SampleEpisodeIds.SonHa03, positionSeconds = 768) }
            assertEquals(emptyList(), events)
            assertEquals(768f, onNodeWithTag("seek").performScrollTo().fetchSemanticsNode().config[SemanticsProperties.ProgressBarRangeInfo].current)
        }
    }

    @Test
    fun focus_loss_discards_uncommitted_keyboard_input() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            val events = mutableListOf<ShellEvent>()
            setContent { CantosPresentation(sampleState(stack = listOf(ShellRoute.Player)), ShellCatalogView(sampleWorks), { events += it }) }
            val slider = onNodeWithTag("seek").performScrollTo()
            slider.performSemanticsAction(SemanticsActions.RequestFocus) { it() }
            slider.performKeyInput { keyDown(Key.DirectionRight) }
            onNodeWithTag("transport").performScrollTo().performSemanticsAction(SemanticsActions.RequestFocus) { it() }
            onNodeWithTag("transport").performKeyInput { keyUp(Key.DirectionRight) }
            onNodeWithTag("elapsed").assertTextEquals("12:48")
            assertEquals(emptyList(), events)
        }
    }

}
