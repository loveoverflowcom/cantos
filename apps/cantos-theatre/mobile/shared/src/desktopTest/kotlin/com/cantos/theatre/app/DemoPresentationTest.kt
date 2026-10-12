package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.demo.DemoCatalogView
import com.cantos.theatre.demo.DemoBookmark
import com.cantos.theatre.demo.DemoEpisode
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoEvent
import com.cantos.theatre.demo.DemoLibraryTab
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoWork
import com.cantos.theatre.demo.DemoWorkId
import com.cantos.theatre.demo.PlaybackUiStatus
import com.cantos.theatre.features.shell.presentation.CantosDemoPresentation
import org.junit.Test
import kotlin.test.assertEquals

/** Supplied projections and callbacks exercise the real presentation without an app owner. */
@OptIn(ExperimentalTestApi::class)
class DemoPresentationTest {
    @Test
    fun bookmark_actions_and_marked_state_preserve_distinct_points_within_one_second() {
        val bookmark = DemoBookmark(DemoEpisodeId.SonHa03, 30, 30_123L)
        var state by mutableStateOf(DemoState(
            stack = listOf(DemoRoute.Discovery, DemoRoute.Player),
            positionSeconds = 30, positionMs = 30_123L, bookmarks = listOf(bookmark),
        ))
        val events = mutableListOf<DemoEvent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosDemoPresentation(state, catalog(), { events += it })
                }
            }
            onNodeWithTag("bookmark").performScrollTo().assert(hasText("Đã ghi dấu điểm này"))
            runOnIdle { state = state.copy(positionMs = 30_456L) }
            onNodeWithTag("bookmark").assert(hasText("Ghi dấu điểm này"))
            runOnIdle {
                state = state.copy(stack = listOf(DemoRoute.Library), libraryTab = DemoLibraryTab.Bookmarks)
            }
            onNodeWithText("Người giữ tiếng chuông · 0:30").performScrollTo().performClick()
            assertEquals<List<DemoEvent>>(listOf(DemoEvent.OpenBookmark(bookmark)), events)
            assertEquals(30_123L, (events.single() as DemoEvent.OpenBookmark).bookmark.positionMs)
        }
    }

    @Test
    fun requested_transport_remains_pausable_while_observation_is_loading() {
        val events = mutableListOf<DemoEvent>()
        val state = DemoState(
            stack = listOf(DemoRoute.Discovery, DemoRoute.Player),
            playbackStatus = PlaybackUiStatus.Loading,
            transportRequested = true,
        )
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosDemoPresentation(state, catalog(), { events += it })
                }
            }
            onNodeWithTag("simulation-status").assertTextEquals("Đang mở mô phỏng · Không có âm thanh thật")
            onNodeWithTag("transport").performScrollTo()
                .assert(hasContentDescription("Dừng mô phỏng Người giữ tiếng chuông"))
                .assert(hasText("Dừng mô phỏng"))
                .performClick()
            assertEquals<List<DemoEvent>>(listOf(DemoEvent.ToggleSimulation), events)
            onNodeWithTag("simulation-status").assertTextEquals("Đang mở mô phỏng · Không có âm thanh thật")
            onNodeWithTag("elapsed").assertTextEquals("12:48")
        }
    }

    @Test
    fun every_observed_status_has_an_honest_label_without_mutating_the_projection() {
        var state by mutableStateOf(DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Player)))
        val expected = listOf(
            PlaybackUiStatus.Idle to "Chưa mở mô phỏng · Không có âm thanh thật",
            PlaybackUiStatus.Loading to "Đang mở mô phỏng · Không có âm thanh thật",
            PlaybackUiStatus.Paused to "Mô phỏng đang dừng",
            PlaybackUiStatus.Simulating to "Đang mô phỏng · Không phát âm thanh",
            PlaybackUiStatus.Buffering to "Mô phỏng đang chờ · Không có âm thanh thật",
            PlaybackUiStatus.Ended to "Mô phỏng đã kết thúc · Không có âm thanh thật",
        )
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosDemoPresentation(state, catalog(), {})
                }
            }
            expected.forEach { (status, label) ->
                runOnIdle { state = state.copy(playbackStatus = status) }
                onNodeWithTag("simulation-status").assertTextEquals(label)
                onNodeWithTag("elapsed").assertTextEquals("12:48")
            }
        }
    }

    @Test
    fun absent_work_route_renders_recovery_and_emits_navigation_without_guessing_metadata() {
        val events = mutableListOf<DemoEvent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosDemoPresentation(
                        DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Work(DemoWorkId.CanhBuom))),
                        catalog(),
                        { events += it },
                    )
                }
            }
            onNodeWithTag("selection-unavailable").assertExists()
            onNodeWithText("Chưa có nội dung minh họa cho lựa chọn này").assertExists()
            onNodeWithTag("selection-discover").performClick()
            assertEquals<List<DemoEvent>>(listOf(DemoEvent.Navigate(DemoRoute.Discovery)), events)
            onNodeWithTag("selection-unavailable").assertExists()
        }
    }

    @Test
    fun unknown_opaque_route_projection_offers_back_and_discovery_without_substituting_a_work() {
        val events = mutableListOf<DemoEvent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosDemoPresentation(
                        DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Unavailable)),
                        catalog(),
                        { events += it },
                    )
                }
            }
            onNodeWithTag("selection-unavailable").assertExists()
            onNodeWithTag("work-SonHa").assertDoesNotExist()
            onNodeWithTag("back").performClick()
            onNodeWithTag("selection-discover").performClick()
            assertEquals<List<DemoEvent>>(listOf(DemoEvent.Back, DemoEvent.Navigate(DemoRoute.Discovery)), events)
        }
    }

    @Test
    fun empty_catalogue_keeps_tab_navigation_and_shows_empty_discovery() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosDemoPresentation(DemoState(), DemoCatalogView(), {})
                }
            }
            onNodeWithText("Chưa tìm thấy câu chuyện").assertExists()
            onNodeWithTag("continue").assertDoesNotExist()
            onNodeWithTag("mini-open").assertDoesNotExist()
            onNodeWithTag("nav-discovery").assertExists()
            onNodeWithTag("nav-search").assertExists()
            onNodeWithTag("nav-library").assertExists()
        }
    }

    @Test
    fun missing_episode_recovery_replaces_player_and_queue_controls() {
        var state by mutableStateOf(DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Player)))
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosDemoPresentation(state, DemoCatalogView(), {})
                }
            }
            onNodeWithTag("selection-unavailable").assertExists()
            onNodeWithTag("transport").assertDoesNotExist()
            onNodeWithTag("seek").assertDoesNotExist()
            runOnIdle { state = state.copy(stack = listOf(DemoRoute.Discovery, DemoRoute.Queue)) }
            onNodeWithTag("selection-unavailable").assertExists()
            onNodeWithTag("episode-SonHa03").assertDoesNotExist()
        }
    }

    @Test
    fun a_work_with_no_episodes_has_a_discoverable_disabled_start_and_can_request_save() {
        val events = mutableListOf<DemoEvent>()
        val emptyWork = catalog().work(DemoWorkId.SonHa)?.copy(episodes = emptyList())
        val view = DemoCatalogView(listOf(requireNotNull(emptyWork)))
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosDemoPresentation(
                        DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Work(DemoWorkId.SonHa)), savedWorks = emptySet()),
                        view,
                        { events += it },
                    )
                }
            }
            onNodeWithText("Mở tập đầu").performScrollTo().assertIsNotEnabled()
            onNodeWithText("Thông tin hiện tại chưa đủ để mở lựa chọn này. Trở về khám phá để chọn lại.").assertExists()
            onNodeWithTag("save-work").performScrollTo().performClick()
            assertEquals<List<DemoEvent>>(listOf(DemoEvent.SaveWork(DemoWorkId.SonHa)), events)
            onNodeWithTag("save-work").assert(hasText("Lưu tác phẩm"))
        }
    }

    private fun catalog(): DemoCatalogView = DemoCatalogView(listOf(
        DemoWork(
            id = DemoWorkId.SonHa,
            title = "Sơn Hà Ký",
            category = "Tiên hiệp",
            description = "Metadata supplied by the presentation test.",
            episodes = listOf(DemoEpisode(DemoEpisodeId.SonHa03, DemoWorkId.SonHa, 3, "Người giữ tiếng chuông", 1722)),
        ),
    ))
}
