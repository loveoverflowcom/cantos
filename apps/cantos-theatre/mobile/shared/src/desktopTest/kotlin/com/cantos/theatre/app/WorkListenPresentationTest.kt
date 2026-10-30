package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellEpisode
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.presentation.model.WorkListenPhase
import com.cantos.theatre.presentation.model.WorkListenRelease
import com.cantos.theatre.presentation.model.PlaybackUiStatus
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import java.io.File
import kotlin.test.assertEquals

/** The actual Work composition with injected immutable facts; no real decoder/OS or backend. */
@OptIn(ExperimentalTestApi::class)
class WorkListenPresentationTest {
    private val id = ShellWorkId("work")
    private val unit = ShellEpisode(ShellEpisodeId("first", id), id, 4, "Tiếng chuông trong sương", 120, publicationId = "release-a")
    private val work = ShellWork(id, "Người gom tiếng chuông", "Truyện", "Một lời hẹn chưa nói hết.", listOf(unit))
    private val catalog = ShellCatalogView(works = listOf(work))
    private fun state(phase: WorkListenPhase? = null, theme: ShellTheme = ShellTheme.Light, locale: ShellLocale = ShellLocale.Vietnamese) =
        ShellState(stack = listOf(ShellRoute.Discovery, ShellRoute.Work(id)), selectedEpisode = unit.id,
            transportRequested = phase in listOf(WorkListenPhase.PauseLoading, WorkListenPhase.PauseBuffering, WorkListenPhase.PausePlaying),
            playbackStatus = when (phase) {
                WorkListenPhase.PauseLoading -> PlaybackUiStatus.Loading
                WorkListenPhase.PauseBuffering -> PlaybackUiStatus.Buffering
                WorkListenPhase.PausePlaying -> PlaybackUiStatus.Playing
                WorkListenPhase.Resume -> PlaybackUiStatus.Paused
                else -> PlaybackUiStatus.Idle
            },
            positionMs = if (phase == WorkListenPhase.Resume) 1000 else 0,
            positionSeconds = if (phase == WorkListenPhase.Resume) 1 else 0,
            theme = theme, locale = locale, workListenRelease = phase?.let { WorkListenRelease(unit.id, "release-a", it) })

    @Test
    fun work_maps_one_named_action_to_resolve_pause_resume_and_each_failure_owner() = runDesktopComposeUiTest(width = 390, height = 844) {
        var supplied by mutableStateOf(state())
        val events = mutableListOf<ShellEvent>()
        setContent { CantosPresentation(supplied, catalog, events::add) }
        assertEquals(emptyList(), events, "opening Work does not autoplay")
        onNodeWithTag("work-listen").performScrollTo().assert(hasContentDescription("Nghe Tiếng chuông trong sương"))
            .performClick()
        assertEquals(ShellEvent.OpenEpisode(unit.id), events.last())
        runOnIdle { supplied = state(WorkListenPhase.Resolving) }
        onNodeWithTag("work-listen").assertIsNotEnabled().performClick()
        assertEquals(1, events.size)
        for (phase in listOf(WorkListenPhase.PauseLoading, WorkListenPhase.PauseBuffering, WorkListenPhase.PausePlaying,
            WorkListenPhase.Resume, WorkListenPhase.ResolveFailed, WorkListenPhase.TransportFailed)) {
            runOnIdle { supplied = state(phase) }
            onNodeWithTag("work-listen").performScrollTo().assertIsEnabled().performClick()
            assertEquals(if (phase == WorkListenPhase.ResolveFailed) ShellEvent.RetryListening else ShellEvent.ToggleSimulation, events.last())
            if (phase == WorkListenPhase.PauseLoading) onNodeWithTag("work-listen-status").assertTextEquals("Đang mở tập…")
            if (phase == WorkListenPhase.PausePlaying) onNodeWithTag("work-listen-status").assertTextEquals("Đang phát")
        }
        runOnIdle { supplied = state(WorkListenPhase.Resume, locale = ShellLocale.English) }
        onNodeWithTag("work-listen").assert(hasContentDescription("Resume Tiếng chuông trong sương"))
        val beforeUnavailable = events.size
        runOnIdle { supplied = state(WorkListenPhase.Unavailable) }
        onNodeWithTag("work-listen").assertIsNotEnabled().performClick()
        assertEquals(beforeUnavailable, events.size, "an unavailable release never retries or swaps publication")
    }

    @Test
    fun work_composition_is_captured_in_both_themes_at_320_390_and_double_text() {
        val output = File("build/reports/listen-595").apply { mkdirs() }
        for (theme in ShellTheme.entries) for ((width, scale) in listOf(320 to 1f, 320 to 2f, 390 to 1f)) {
            runDesktopComposeUiTest(width = width, height = 844) {
                setContent {
                    Box(Modifier.requiredSize(width.dp, 844.dp).testTag("capture")) {
                        CantosPresentation(state(WorkListenPhase.PauseBuffering, theme), catalog, {}, textScale = scale)
                    }
                }
                onNodeWithTag("work-listen").performScrollTo().assertIsEnabled()
                onNodeWithTag("work-listen-status").performScrollTo()
                val control = onNodeWithTag("work-listen").fetchSemanticsNode()
                kotlin.test.assertTrue(control.boundsInRoot.left >= 0 && control.boundsInRoot.right <= width)
                val bytes = requireNotNull(Image.makeFromBitmap(onNodeWithTag("capture").captureToImage().asSkiaBitmap())
                    .encodeToData(EncodedImageFormat.PNG)).bytes
                File(output, "work-${theme.name}-$width-${scale.toInt()}.png").writeBytes(bytes)
            }
        }
    }

    @Test
    fun save_states_keep_accessible_action_and_target_in_work_detail() {
        val output = File("build/reports/save-665").apply { mkdirs() }
        val empty = ShellCatalogView(works = listOf(work.copy(episodes = emptyList())))
        for (theme in ShellTheme.entries) for (saved in listOf(false, true)) for ((width, scale) in listOf(390 to 1f, 320 to 2f)) {
            runDesktopComposeUiTest(width = width, height = 844) {
                val supplied = state(theme = theme).copy(savedWorks = if (saved) setOf(id) else emptySet())
                setContent {
                    Box(Modifier.requiredSize(width.dp, 844.dp).testTag("capture")) {
                        CantosPresentation(supplied, empty, {}, textScale = scale)
                    }
                }
                val action = onNodeWithTag("save-work").performScrollTo().assertIsEnabled()
                action.assert(hasContentDescription(if (saved) "Bỏ lưu tác phẩm Người gom tiếng chuông" else "Lưu tác phẩm Người gom tiếng chuông"))
                val bounds = action.fetchSemanticsNode().boundsInRoot
                kotlin.test.assertTrue(bounds.height >= 48f, "Save needs a 48dp touch target")
                kotlin.test.assertTrue(bounds.left >= 0 && bounds.right <= width, "Save remains within the viewport")
                val bytes = requireNotNull(Image.makeFromBitmap(onNodeWithTag("capture").captureToImage().asSkiaBitmap())
                    .encodeToData(EncodedImageFormat.PNG)).bytes
                File(output, "work-${theme.name}-${if (saved) "saved" else "save"}-$width-text${scale.toInt()}.png").writeBytes(bytes)
            }
        }
    }

    @Test
    fun absent_audio_keeps_short_reason_without_help_control() = runDesktopComposeUiTest(width = 390, height = 844) {
        val events = mutableListOf<ShellEvent>()
        val empty = ShellCatalogView(works = listOf(work.copy(episodes = emptyList())))
        var supplied by mutableStateOf(state())
        setContent { CantosPresentation(supplied, empty, events::add) }
        for ((locale, reason) in listOf(
            Pair(ShellLocale.Vietnamese, "Chưa có audio"),
            Pair(ShellLocale.English, "No audio yet"),
        )) {
            runOnIdle { supplied = state(locale = locale) }
            onNodeWithTag("work-listen").performScrollTo().assertIsNotEnabled().performClick()
            // The control's reason and the empty list's note: two short phrases, never the long copy.
            onAllNodesWithText(reason).assertCountEquals(2)
            onNodeWithTag("work-listen-help").assertDoesNotExist()
            onNodeWithTag("work-listen-help-trigger").assertDoesNotExist()
        }
        assertEquals(emptyList(), events, "disabled Listen dispatches no playback or navigation")
    }

    @Test
    fun saved_write_failure_and_pending_offer_the_correct_retry_without_claiming_success() = runDesktopComposeUiTest(width = 390, height = 844) {
        val events = mutableListOf<ShellEvent>()
        val empty = ShellCatalogView(works = listOf(work.copy(episodes = emptyList())))
        var supplied by mutableStateOf(state().copy(savedWorks = setOf(id), confirmedSavedWorks = emptySet(), failedSavedWorks = setOf(id), retryableSavedWorks = setOf(id), savedAccount = "fixture"))
        val output = File("build/reports/save-665").apply { mkdirs() }
        setContent { Box(Modifier.requiredSize(390.dp, 844.dp).testTag("retry-capture")) { CantosPresentation(supplied, empty, events::add) } }
        fun capture(name: String) {
            val bytes = requireNotNull(Image.makeFromBitmap(onNodeWithTag("retry-capture").captureToImage().asSkiaBitmap())
                .encodeToData(EncodedImageFormat.PNG)).bytes
            File(output, "work-$name-390.png").writeBytes(bytes)
        }
        onNodeWithTag("save-work").performScrollTo().assert(hasContentDescription("Lưu tác phẩm Người gom tiếng chuông"))
        onNodeWithTag("save-work-status").assertTextEquals("Đồng bộ chưa thành công")
        onNodeWithTag("save-work-retry").performScrollTo()
        capture("failed-save")
        onNodeWithTag("save-work-retry").performClick()
        assertEquals(ShellEvent.RetrySaved(id, "fixture", true, false), events.last())
        runOnIdle { supplied = state().copy(savedWorks = setOf(id), confirmedSavedWorks = emptySet(), pendingSavedWorks = setOf(id), savedAccount = "fixture") }
        onNodeWithTag("save-work").performScrollTo().assertIsNotEnabled()
        onNodeWithTag("save-work-status").assertTextEquals("Đang cập nhật trạng thái lưu…")
        onNodeWithTag("save-work-retry").performScrollTo()
        capture("pending-save")
        onNodeWithTag("save-work-retry").performClick()
        assertEquals(ShellEvent.RetrySaved(id, "fixture", true, true), events.last())
        runOnIdle { supplied = state().copy(savedWorks = emptySet(), confirmedSavedWorks = setOf(id), failedSavedWorks = setOf(id), retryableSavedWorks = setOf(id), savedAccount = "fixture") }
        onNodeWithTag("save-work").performScrollTo().assert(hasContentDescription("Bỏ lưu tác phẩm Người gom tiếng chuông"))
        onNodeWithTag("save-work-retry").performScrollTo()
        capture("failed-unsave")
        onNodeWithTag("save-work-retry").performClick()
        assertEquals(ShellEvent.RetrySaved(id, "fixture", false, false), events.last())
        runOnIdle { supplied = state().copy(failedSavedWorks = setOf(id)) }
        onNodeWithTag("save-work").performScrollTo().assertIsNotEnabled()
        onNodeWithTag("save-work-retry").assertDoesNotExist()
    }
}
