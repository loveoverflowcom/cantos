package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import com.cantos.theatre.adapters.mock.MemoryProgressPort
import com.cantos.theatre.adapters.mock.MockCatalogPort
import com.cantos.theatre.adapters.mock.MockMediaEngine
import com.cantos.theatre.adapters.mock.mockInventory
import com.cantos.theatre.adapters.mock.toEpisodeKey
import com.cantos.theatre.adapters.mock.toPlaybackSource
import com.cantos.theatre.domain.listening.ListeningSnapshot
import com.cantos.theatre.domain.listening.ProgressRecord
import com.cantos.theatre.domain.playback.PlaybackSnapshot
import com.cantos.theatre.demo.DemoBookmark
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoProgress
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.demo.DemoWorkId
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Drives the real demo composition on the JVM renderer; no native engine or OS input claim. */
@OptIn(ExperimentalTestApi::class)
class DemoInteractionTest {
    @Test
    fun continue_transport_bookmark_queue_and_close_preserve_one_demo_session() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { DemoTestRoot() }
            onNodeWithTag("continue").performScrollTo().performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Phòng nghe"))
            onNodeWithTag("elapsed").assertTextEquals("12:48")
            onNodeWithTag("simulation-status").assertTextEquals("Mô phỏng đang dừng")
            onNodeWithTag("transport").performScrollTo().performClick()
            onNodeWithTag("simulation-status").assertTextEquals("Đang mô phỏng · Không phát âm thanh")
            onNodeWithTag("rewind").performScrollTo().performClick()
            onNodeWithTag("elapsed").assertTextEquals("12:33")
            onNodeWithTag("bookmark").performScrollTo().performClick().performClick()
            onNodeWithTag("bookmark").assert(hasText("Đã ghi dấu điểm này"))
            onNodeWithTag("queue").performScrollTo().performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Tiếp theo"))
            onNodeWithTag("episode-SonHa03").performScrollTo().performClick()
            onNodeWithTag("elapsed").assertTextEquals("12:33")
            onNodeWithTag("simulation-status").assertTextEquals("Đang mô phỏng · Không phát âm thanh")
            onNodeWithTag("back").performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Khám phá"))
            onNodeWithTag("mini-open").assert(hasText("Người giữ tiếng chuông")).assert(hasText("12:33"))
            onNodeWithTag("mini-transport").assert(hasContentDescription("Dừng mô phỏng Người giữ tiếng chuông"))
            onNodeWithTag("nav-library").performClick()
            onNodeWithTag("library-continue").assertExists()
            onNodeWithTag("library-saved").assertExists()
            onNodeWithTag("library-history").assertExists()
            onNodeWithTag("recent-SonHa03").assertExists()
            onNodeWithTag("mini-open").performClick()
            onAllNodesWithContentDescription("Mở dấu trang Người giữ tiếng chuông, 12 phút 33 giây").assertCountEquals(1)
        }
    }

    @Test
    fun vietnamese_search_empty_result_and_clear_recover_the_visible_catalogue() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { DemoTestRoot() }
            onNodeWithTag("search").performScrollTo().performClick()
            onNodeWithTag("nav-discovery").assertIsSelected()
            val input = onNodeWithTag("search-input")
            input.performTextReplacement("SƠN HÀ")
            onNodeWithText("1 tác phẩm minh họa").assertExists()
            onNodeWithTag("work-SonHa").assertExists()
            onNodeWithTag("work-DenKhua").assertDoesNotExist()
            input.performTextReplacement("không có trong tủ sách")
            onNodeWithText("0 tác phẩm minh họa").assertExists()
            onNodeWithText("Chưa tìm thấy câu chuyện").assertExists()
            onNodeWithTag("work-SonHa").assertDoesNotExist()
            onNodeWithContentDescription("Xóa tìm kiếm").performClick()
            input.assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("")))
            onNodeWithText("3 tác phẩm minh họa").assertExists()
            onNodeWithTag("work-SonHa").assertExists()
            onNodeWithTag("work-DenKhua").assertExists()
            onNodeWithTag("work-CanhBuom").assertExists()
        }
    }

    @Test
    fun save_and_unsave_are_reflected_in_the_saved_library() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { DemoTestRoot(initialState = DemoState(savedWorks = emptySet())) }
            onNodeWithTag("work-SonHa").performScrollTo().performClick()
            onNodeWithTag("save-work").performScrollTo().assert(hasText("Lưu tác phẩm")).performClick()
            onNodeWithTag("save-work").assert(hasText("Đã lưu · Bỏ lưu"))
            onNodeWithTag("nav-library").performClick()
            onNodeWithTag("library-saved").performScrollTo()
            onNodeWithTag("work-SonHa").assertExists()
            onNodeWithTag("work-DenKhua").assertDoesNotExist()
            onNodeWithTag("work-SonHa").performScrollTo().performClick()
            onNodeWithTag("save-work").performScrollTo().performClick()
            onNodeWithTag("save-work").assert(hasText("Lưu tác phẩm"))
            onNodeWithTag("nav-library").performClick()
            onNodeWithTag("library-saved").performScrollTo()
            onNodeWithText("Chưa lưu câu chuyện nào").assertExists()
            onNodeWithTag("work-SonHa").assertDoesNotExist()
        }
    }

    @Test
    fun settings_back_returns_to_the_player_with_the_same_position_and_stopped_simulation() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                DemoTestRoot(initialState = DemoState(
                    stack = listOf(DemoRoute.Discovery, DemoRoute.Player),
                    positionSeconds = 900,
                ))
            }
            onNodeWithTag("account").performClick()
            onNodeWithTag("settings").performScrollTo().performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Cài đặt nghe"))
            onNodeWithTag("setting-Theme").performScrollTo().performClick()
            onNodeWithTag("theme-Dark").performClick()
            onNodeWithTag("setting-Theme").assert(hasText("Mực tím · Tối"))
            onNodeWithTag("back").performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Tài khoản"))
            onNodeWithTag("back").performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Phòng nghe"))
            onNodeWithTag("elapsed").assertTextEquals("15:00")
            onNodeWithTag("simulation-status").assertTextEquals("Mô phỏng đang dừng")
            onNodeWithTag("transport").assert(hasContentDescription("Mô phỏng phát Người giữ tiếng chuông"))
        }
    }

    @Test
    fun escape_from_a_focused_settings_control_returns_to_the_search_query() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { DemoTestRoot() }
            onNodeWithTag("search").performScrollTo().performClick()
            onNodeWithTag("search-input").performTextReplacement("Đèn Khuya")
            onNodeWithTag("account").performClick()
            onNodeWithTag("settings").performScrollTo().performClick()
            val choice = onNodeWithTag("setting-Theme").performScrollTo()
            choice.performSemanticsAction(SemanticsActions.RequestFocus) { assertTrue(it()) }
            choice.assertIsFocused().performKeyInput { pressKey(Key.Escape) }
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Tài khoản"))
            onNodeWithTag("back").performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Tìm kiếm"))
            onNodeWithTag("search-input").assert(
                SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("Đèn Khuya")),
            )
            onNodeWithText("1 tác phẩm minh họa").assertExists()
            onNodeWithTag("work-DenKhua").assertExists()
        }
    }

    @Test
    fun error_scenario_and_retry_keep_the_episode_and_resume_position() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { DemoTestRoot() }
            onNodeWithTag("account").performClick()
            onNodeWithTag("settings").performScrollTo().performClick()
            onNodeWithTag("setting-Diagnostics").performScrollTo().performClick()
            onNodeWithTag("catalog-Error").performScrollTo().performClick().assertIsSelected()
            onNodeWithTag("preference-close").performClick()
            onNodeWithTag("back").performClick()
            onNodeWithTag("back").performClick()
            onNodeWithText("Chưa mở được danh sách minh họa").assertExists()
            onNodeWithText("Đây là lỗi mô phỏng. Tập và điểm nghe của bạn vẫn được giữ.").assertExists()
            onNodeWithTag("mini-open").assert(hasText("Người giữ tiếng chuông")).assert(hasText("12:48"))
            onNodeWithTag("retry").performScrollTo().performClick()
            onNodeWithText("Chưa mở được danh sách minh họa").assertDoesNotExist()
            onNodeWithTag("work-SonHa").assertExists()
            onNodeWithTag("mini-open").assert(hasText("12:48"))
        }
    }

    @Test
    fun loading_recovery_keeps_the_search_query_and_does_not_require_a_network_result() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                DemoTestRoot(initialState = DemoState(
                    stack = listOf(DemoRoute.Search),
                    query = "Sơn Hà",
                    catalogMode = DemoCatalogMode.Loading,
                ))
            }
            onNodeWithText("Đang tải bản minh họa…").assertExists()
            onNodeWithText("Trạng thái tải được chọn thủ công; không có yêu cầu mạng.").assertExists()
            onNodeWithTag("retry").performScrollTo().performClick()
            onNodeWithTag("search-input").assert(
                SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("Sơn Hà")),
            )
            onNodeWithText("1 tác phẩm minh họa").assertExists()
            onNodeWithTag("work-SonHa").assertExists()
        }
    }

    @Test
    fun seek_semantics_change_the_exact_visible_position_without_starting_simulation() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { DemoTestRoot(initialState = DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Player))) }
            val seek = onNodeWithTag("seek").performScrollTo()
            seek.assert(hasContentDescription("Điểm nghe minh họa"))
            seek.assert(SemanticsMatcher.expectValue(
                SemanticsProperties.StateDescription,
                "12 phút 48 giây trên 28 phút 42 giây",
            ))
            seek.performSemanticsAction(SemanticsActions.SetProgress) { assertTrue(it(900f)) }
            onNodeWithTag("elapsed").assertTextEquals("15:00")
            seek.assert(SemanticsMatcher.expectValue(
                SemanticsProperties.ProgressBarRangeInfo,
                ProgressBarRangeInfo(900f, 0f..1722f),
            ))
            onNodeWithTag("simulation-status").assertTextEquals("Mô phỏng đang dừng")
        }
    }

    @Test
    fun narrow_large_text_work_and_player_actions_remain_scrollable_and_named() {
        var rendererDensity = Float.NaN
        runDesktopComposeUiTest(width = 320, height = 800) {
            setContent {
                rendererDensity = LocalDensity.current.density
                DemoTestRoot(
                    initialState = DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Work(DemoWorkId.SonHa))),
                    textScale = 2f,
                    width = 320,
                    height = 800,
                )
            }
            val episode = onNodeWithTag("episode-SonHa03").performScrollTo()
            assertEquals(1f, rendererDensity, "This explicitly sized JVM bounds oracle uses density 1")
            episode.assertIsDisplayed().assert(hasContentDescription("Mô phỏng phát Người giữ tiếng chuông"))
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
            val bounds = episode.fetchSemanticsNode().boundsInRoot
            assertTrue(bounds.left >= 0f && bounds.right <= 320f, "Episode action must fit the explicitly sized JVM root")
            assertTrue(bounds.height >= 48f, "Episode action must preserve the demo host's 48 dp touch floor at density 1")
            episode.performClick()
            onNodeWithTag("bookmark").performScrollTo().assertIsDisplayed().performClick()
            onNodeWithTag("bookmark").assert(hasText("Đã ghi dấu điểm này"))
            onNodeWithTag("back").performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Tác phẩm"))
            onNodeWithTag("mini-open").assert(hasText("12:48"))
        }
    }

    @Test
    fun history_confirmation_clears_only_history_and_latest_work_position_reopens_paused() {
        val points = listOf(
            DemoProgress(DemoEpisodeId.SonHa02, 100),
            DemoProgress(DemoEpisodeId.DenKhua01, 40),
            DemoProgress(DemoEpisodeId.SonHa03, 768),
        )
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                DemoTestRoot(initialState = DemoState(
                    stack = listOf(DemoRoute.Library),
                    bookmarks = listOf(DemoBookmark(DemoEpisodeId.SonHa03, 768)),
                    progress = points,
                    history = listOf(points.last(), points[1]),
                ))
            }
            onNodeWithTag("nav-search").assertDoesNotExist()
            onNodeWithTag("resume-SonHa02").assertDoesNotExist()
            onNodeWithTag("resume-SonHa03").assertExists()
            onNodeWithTag("resume-DenKhua01").assertExists()
            onNodeWithTag("history-all").performScrollTo().performClick()
            onNodeWithTag("history-clear").performScrollTo().performClick()
            onNodeWithText("Chỉ xóa danh sách lịch sử. Vị trí nghe, truyện đã lưu, dấu trang và trình phát vẫn được giữ lại.").assertExists()
            val cancel = onNodeWithTag("history-cancel")
            cancel.performSemanticsAction(SemanticsActions.RequestFocus) { assertTrue(it()) }
            cancel.assertIsFocused().performKeyInput { pressKey(Key.Escape) }
            onNodeWithTag("history-dialog").assertDoesNotExist()
            onNodeWithTag("history-SonHa03").assertExists()
            onNodeWithTag("history-clear").performClick()
            onNodeWithTag("history-cancel").performClick()
            onNodeWithTag("history-SonHa03").assertExists()
            onNodeWithTag("history-clear").performClick()
            onNodeWithTag("history-confirm").performClick()
            onNodeWithText("Chưa có lịch sử nghe").assertExists()
            onNodeWithTag("history-SonHa03").assertDoesNotExist()
            onNodeWithTag("mini-open").assert(hasText("12:48"))
            onNodeWithTag("back").performClick()
            onNodeWithTag("work-SonHa").assertExists()
            onNodeWithTag("resume-SonHa03").performScrollTo().performClick()
            onNodeWithTag("simulation-status").assertTextEquals("Mô phỏng đang dừng")
            onNodeWithTag("elapsed").assertTextEquals("12:48")
            onNodeWithTag("bookmark").performScrollTo().assert(hasText("Đã ghi dấu điểm này"))
        }
    }

    @Test
    fun avatar_account_honestly_disables_sign_in_and_preserves_mini_player() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { DemoTestRoot() }
            onNodeWithTag("account").performClick()
            onNodeWithTag("sign-in").performScrollTo().assertIsNotEnabled()
            onNodeWithText("Chưa hỗ trợ trong bản xem thử.").assertExists()
            onNodeWithText("Người nghe khách").assertExists()
            onNodeWithText("Truyện đã lưu và điểm nghe chỉ được giữ trong phiên này.").assertExists()
            onNodeWithTag("header-title").assertTextEquals("Không gian của bạn")
            onNodeWithText("Tài khoản").assertDoesNotExist()
            onNodeWithTag("mini-open").assert(hasText("12:48"))
            onNodeWithTag("nav-library").performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Của tôi"))
            onNodeWithTag("mini-open").assert(hasText("12:48"))
        }
    }

    @Test
    fun refused_resume_shows_recovery_and_keeps_the_current_player_and_stored_point() {
        lateinit var app: AppContainer
        val key = DemoEpisodeId.SonHa03.toEpisodeKey()
        val mismatch = ProgressRecord(key, 768_000L, 1_800_000L)
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                val scope = rememberCoroutineScope()
                val container = remember {
                    val inventory = mockInventory()
                    val episode = requireNotNull(inventory.episode(key))
                    val snapshot = ListeningSnapshot(progress = listOf(mismatch))
                    AppContainer(
                        scope,
                        MockCatalogPort(inventory),
                        MemoryProgressPort(snapshot),
                        MockMediaEngine(),
                        AppStartState(
                            inventory,
                            snapshot,
                            PlaybackSnapshot(requireNotNull(episode.toPlaybackSource()), 768_000L, 1000),
                            NavigationState(listOf(TheatreRoute.Library)),
                        ),
                    ).also { app = it }
                }
                DisposableEffect(container) { onDispose { container.close() } }
                DemoTestRoot(appContainer = container)
            }
            onNodeWithTag("resume-SonHa03").performScrollTo().performClick()
            onNodeWithTag("selection-unavailable").assertExists()
            onNodeWithTag("transport").assertDoesNotExist()
            onNodeWithTag("mini-open").assert(hasText("12:48"))
            runOnIdle {
                assertEquals(TheatreRoute.Unavailable, app.navigation.value.route)
                assertEquals(listOf(mismatch), app.listening.state.value.snapshot.progress)
                assertEquals(768_000L, app.playback.state.value.session?.positionMs)
            }
        }
    }

    @Test
    fun settings_and_player_dialogs_edit_the_same_preferences_without_moving_progress() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { DemoTestRoot(initialState = DemoState(stack = listOf(DemoRoute.Discovery, DemoRoute.Settings))) }
            onNodeWithTag("setting-Language").performScrollTo().performClick()
            onNodeWithTag("locale-English").performClick()
            onNodeWithTag("setting-Language").assert(hasText("English"))
            onNodeWithTag("setting-Speed").performScrollTo().performClick()
            onNodeWithTag("speed-125").performClick()
            onNodeWithTag("setting-Speed").assert(hasText("1.25×"))
            onNodeWithTag("setting-Timer").performScrollTo().performClick()
            onNodeWithTag("timer-Fifteen").performClick()
            onNodeWithTag("setting-Timer").assert(hasText("15 minutes"))
            onNodeWithTag("mini-open").performClick()
            onNodeWithTag("elapsed").assertTextEquals("12:48")
            onNodeWithTag("player-speed").performScrollTo().assert(hasText("1.25×")).performClick()
            onNodeWithTag("speed-150").performClick()
            onNodeWithTag("player-speed").assertIsFocused().assert(hasText("1.5×"))
            onNodeWithTag("player-timer").performClick()
            onNodeWithText("Select a demo state only; no countdown or automatic audio stop runs.").assertExists()
            onNodeWithTag("timer-Off").performClick()
            onNodeWithTag("player-timer").assertIsFocused().assert(hasText("Off"))
            onNodeWithTag("elapsed").assertTextEquals("12:48")
            onNodeWithTag("back").performClick()
            onNodeWithTag("setting-Speed").performScrollTo().assert(hasText("1.5×"))
            onNodeWithTag("setting-Timer").assert(hasText("Off"))
        }
    }

    @Test
    fun reset_from_settings_restores_initial_locale_position_and_empty_bookmarks() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                DemoTestRoot(initialState = DemoState(
                    stack = listOf(DemoRoute.Search, DemoRoute.Settings),
                    positionSeconds = 900,
                    simulating = true,
                    savedWorks = emptySet(),
                    bookmarks = listOf(DemoBookmark(DemoEpisodeId.SonHa03, 900)),
                    query = "Đèn Khuya",
                    catalogMode = DemoCatalogMode.Error,
                    theme = DemoTheme.Dark,
                    locale = DemoLocale.English,
                    speedPercent = 150,
                ))
            }
            onNodeWithTag("setting-Diagnostics").performScrollTo().performClick()
            onNodeWithTag("reset").performScrollTo().assert(hasText("Reset demo")).performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Khám phá"))
            onNodeWithTag("mini-open").assert(hasText("12:48"))
            onNodeWithTag("mini-transport").assert(hasContentDescription("Mô phỏng phát Người giữ tiếng chuông"))
            onNodeWithTag("nav-library").performClick()
            onNodeWithText("Chưa có lịch sử nghe").assertExists()
            onNodeWithTag("nav-discovery").performClick()
            onNodeWithTag("search").performScrollTo().performClick()
            onNodeWithTag("search-input").assert(
                SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("")),
            )
            onNodeWithText("3 tác phẩm minh họa").assertExists()
        }
    }
}

@Composable
private fun DemoTestRoot(
    initialState: DemoState = DemoState(),
    textScale: Float = 1f,
    width: Int = 390,
    height: Int = 844,
    appContainer: AppContainer? = null,
) {
    Box(Modifier.requiredSize(width.dp, height.dp)) {
        CantosDemoApp(initialState = initialState, textScale = textScale, appContainer = appContainer)
    }
}
