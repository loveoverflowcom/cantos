package com.cantos.theatre.app

import com.cantos.theatre.features.shell.presentation.WithDemoArtwork
import com.cantos.theatre.app.WithDemoShellPresentation
import com.cantos.theatre.demo.demoState
import com.cantos.theatre.demo.DemoWorkIds
import com.cantos.theatre.demo.DemoEpisodeIds
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellBookmark
import com.cantos.theatre.presentation.model.ShellEpisode
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellProgress
import com.cantos.theatre.presentation.model.ShellListeningFailure
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.presentation.model.PlaybackUiStatus
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import org.junit.Test
import kotlin.test.assertEquals

/** Supplied projections and callbacks exercise the real presentation without an app owner. */
@OptIn(ExperimentalTestApi::class)
class DemoPresentationTest {
    @Test
    fun bookmark_actions_and_marked_state_preserve_distinct_points_within_one_second() {
        val bookmark = ShellBookmark(DemoEpisodeIds.SonHa03, 30, 30_123L)
        var state by mutableStateOf(demoState(
            stack = listOf(ShellRoute.Discovery, ShellRoute.Player),
            positionSeconds = 30, positionMs = 30_123L, bookmarks = listOf(bookmark),
        ))
        val events = mutableListOf<ShellEvent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(state, catalog(), { events += it }) } }
                }
            }
            onNodeWithTag("bookmark").performScrollTo().assert(hasText("Đã ghi dấu điểm này"))
            runOnIdle { state = state.copy(positionMs = 30_456L) }
            onNodeWithTag("bookmark").assert(hasText("Ghi dấu điểm này"))
            runOnIdle {
                state = state.copy(stack = listOf(ShellRoute.Discovery, ShellRoute.Player))
            }
            onNodeWithText("Người giữ tiếng chuông · 0:30").performScrollTo().performClick()
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.OpenBookmark(bookmark)), events)
            assertEquals(30_123L, (events.single() as ShellEvent.OpenBookmark).bookmark.positionMs)
        }
    }

    @Test
    fun requested_transport_remains_pausable_while_observation_is_loading() {
        val events = mutableListOf<ShellEvent>()
        val state = demoState(
            stack = listOf(ShellRoute.Discovery, ShellRoute.Player),
            playbackStatus = PlaybackUiStatus.Loading,
            transportRequested = true,
        )
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(state, catalog(), { events += it }) } }
                }
            }
            onNodeWithTag("simulation-status").assertTextEquals("Đang mở mô phỏng · Không có âm thanh thật")
            onNodeWithTag("transport").performScrollTo()
                .assert(hasContentDescription("Dừng mô phỏng Người giữ tiếng chuông"))
                .performClick()
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.ToggleSimulation), events)
            onNodeWithTag("simulation-status").assertTextEquals("Đang mở mô phỏng · Không có âm thanh thật")
            onNodeWithTag("elapsed").assertTextEquals("12:48")
        }
    }

    @Test
    fun every_observed_status_has_an_honest_label_without_mutating_the_projection() {
        var state by mutableStateOf(demoState(stack = listOf(ShellRoute.Discovery, ShellRoute.Player)))
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
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(state, catalog(), {}) } }
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
        val events = mutableListOf<ShellEvent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(
                        demoState(stack = listOf(ShellRoute.Discovery, ShellRoute.Work(DemoWorkIds.CanhBuom))),
                        catalog(),
                        { events += it },
                    ) } }
                }
            }
            onNodeWithTag("selection-unavailable").assertExists()
            onNodeWithText("Chưa có nội dung minh họa cho lựa chọn này").assertExists()
            onNodeWithTag("selection-discover").performClick()
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.Navigate(ShellRoute.Discovery)), events)
            onNodeWithTag("selection-unavailable").assertExists()
        }
    }

    @Test
    fun unknown_opaque_route_projection_offers_back_and_discovery_without_substituting_a_work() {
        val events = mutableListOf<ShellEvent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(
                        demoState(stack = listOf(ShellRoute.Discovery, ShellRoute.Unavailable)),
                        catalog(),
                        { events += it },
                    ) } }
                }
            }
            onNodeWithTag("selection-unavailable").assertExists()
            onNodeWithTag("work-SonHa").assertDoesNotExist()
            onNodeWithTag("back").performClick()
            onNodeWithTag("selection-discover").performClick()
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.Back, ShellEvent.Navigate(ShellRoute.Discovery)), events)
        }
    }

    @Test
    fun empty_catalogue_keeps_tab_navigation_and_shows_empty_discovery() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(demoState(), ShellCatalogView(), {}) } }
                }
            }
            onNodeWithText("Chưa tìm thấy câu chuyện").assertExists()
            onNodeWithTag("continue").assertDoesNotExist()
            onNodeWithTag("mini-open").assertDoesNotExist()
            onNodeWithTag("nav-discovery").assertExists()
            onNodeWithTag("nav-search").assertDoesNotExist()
            onNodeWithTag("account").assertExists()
            onNodeWithTag("nav-library").assertExists()
        }
    }

    @Test
    fun missing_episode_recovery_replaces_player_and_queue_controls() {
        var state by mutableStateOf(demoState(stack = listOf(ShellRoute.Discovery, ShellRoute.Player)))
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(state, ShellCatalogView(), {}) } }
                }
            }
            onNodeWithTag("selection-unavailable").assertExists()
            onNodeWithTag("transport").assertDoesNotExist()
            onNodeWithTag("seek").assertDoesNotExist()
            runOnIdle { state = state.copy(stack = listOf(ShellRoute.Discovery, ShellRoute.Queue)) }
            onNodeWithTag("selection-unavailable").assertExists()
            onNodeWithTag("episode-SonHa03").assertDoesNotExist()
        }
    }

    @Test
    fun a_work_with_no_episodes_has_a_discoverable_disabled_start_and_can_request_save() {
        val events = mutableListOf<ShellEvent>()
        val emptyWork = catalog().work(DemoWorkIds.SonHa)?.copy(episodes = emptyList())
        val view = ShellCatalogView(listOf(requireNotNull(emptyWork)))
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(
                        demoState(stack = listOf(ShellRoute.Discovery, ShellRoute.Work(DemoWorkIds.SonHa)), savedWorks = emptySet()),
                        view,
                        { events += it },
                    ) } }
                }
            }
            onNodeWithText("Mở tập đầu").performScrollTo().assertIsNotEnabled()
            onNodeWithText("Thông tin hiện tại chưa đủ để mở lựa chọn này. Trở về khám phá để chọn lại.").assertExists()
            onNodeWithTag("save-work").performScrollTo().performClick()
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.SetSaved(DemoWorkIds.SonHa, saved = true)), events)
            onNodeWithTag("save-work").assert(hasText("Lưu tác phẩm"))
        }
    }

    @Test
    fun save_card_repeated_callbacks_request_the_same_desired_state_and_do_not_open_details() {
        val events = mutableListOf<ShellEvent>()
        var state by mutableStateOf(demoState(savedWorks = emptySet()))
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(state, catalog(), { events += it }) } }
                }
            }
            onNodeWithTag("save-card-SonHa").performScrollTo().performClick().performClick()
            assertEquals<List<ShellEvent>>(listOf(
                ShellEvent.SetSaved(DemoWorkIds.SonHa, saved = true),
                ShellEvent.SetSaved(DemoWorkIds.SonHa, saved = true),
            ), events)
            runOnIdle { state = state.copy(savedWorks = setOf(DemoWorkIds.SonHa)) }
            onNodeWithTag("save-card-SonHa").assert(hasContentDescription("Bỏ lưu tác phẩm Sơn Hà Ký"))
                .performClick().performClick()
            assertEquals<List<ShellEvent>>(listOf(
                ShellEvent.SetSaved(DemoWorkIds.SonHa, saved = true),
                ShellEvent.SetSaved(DemoWorkIds.SonHa, saved = true),
                ShellEvent.SetSaved(DemoWorkIds.SonHa, saved = false),
                ShellEvent.SetSaved(DemoWorkIds.SonHa, saved = false),
            ), events)
        }
    }

    @Test
    fun failed_memory_write_shows_retry_without_claiming_a_saved_work_or_hiding_the_player() {
        val events = mutableListOf<ShellEvent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(
                        demoState(savedWorks = emptySet(), listeningFailure = ShellListeningFailure.MemoryStoreUnavailable),
                        catalog(),
                        { events += it },
                    ) } }
                }
            }
            onNodeWithTag("listening-failure").assertExists()
            onNodeWithText("Chưa lưu được thay đổi trong bộ nhớ. Dữ liệu trước đó được giữ lại.").assertExists()
            onNodeWithTag("listening-retry").performClick()
            assertEquals<List<ShellEvent>>(listOf(ShellEvent.RetryListening), events)
            onNodeWithTag("save-card-SonHa").performScrollTo().assert(hasText("Lưu tác phẩm"))
            onNodeWithTag("mini-open").assert(hasText("12:48"))
            onNodeWithTag("listening-failure").assertExists()
        }
    }

    @Test
    fun unavailable_latest_episode_never_offers_an_older_episode_from_the_same_work() {
        val latest = ShellProgress(DemoEpisodeIds.SonHa03, 768)
        val oldEpisode = ShellEpisode(DemoEpisodeIds.SonHa02, DemoWorkIds.SonHa, 2, "Con đường qua núi", 1640)
        val work = requireNotNull(catalog().work(DemoWorkIds.SonHa)).copy(episodes = listOf(oldEpisode))
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(
                        demoState(
                            stack = listOf(ShellRoute.Library),
                            progress = listOf(ShellProgress(DemoEpisodeIds.SonHa02, 50), latest),
                            savedWorks = emptySet(),
                        ),
                        ShellCatalogView(listOf(work)),
                        {},
                    ) } }
                }
            }
            onNodeWithTag("library-continue").performScrollTo()
            onNodeWithTag("resume-SonHa02").assertDoesNotExist()
            onNodeWithTag("resume-SonHa03").assertDoesNotExist()
            onNodeWithText("Thông tin hiện tại chưa đủ để mở lựa chọn này. Trở về khám phá để chọn lại.").assertExists()
        }
    }

    @Test
    fun diagnostic_search_and_history_roots_select_their_owning_primary_destination() {
        var state by mutableStateOf(demoState(stack = listOf(ShellRoute.Search)))
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    WithDemoArtwork { WithDemoShellPresentation { CantosPresentation(state, catalog(), {}) } }
                }
            }
            onNodeWithTag("nav-discovery").assertIsSelected()
            onNodeWithTag("nav-library").assertIsNotSelected()
            onNodeWithTag("nav-search").assertDoesNotExist()
            runOnIdle { state = state.copy(stack = listOf(ShellRoute.History)) }
            onNodeWithTag("nav-discovery").assertIsNotSelected()
            onNodeWithTag("nav-library").assertIsSelected()
        }
    }

    private fun catalog(): ShellCatalogView = ShellCatalogView(listOf(
        ShellWork(
            id = DemoWorkIds.SonHa,
            title = "Sơn Hà Ký",
            category = "Tiên hiệp",
            description = "Metadata supplied by the presentation test.",
            episodes = listOf(ShellEpisode(DemoEpisodeIds.SonHa03, DemoWorkIds.SonHa, 3, "Người giữ tiếng chuông", 1722)),
        ),
    ))
}
