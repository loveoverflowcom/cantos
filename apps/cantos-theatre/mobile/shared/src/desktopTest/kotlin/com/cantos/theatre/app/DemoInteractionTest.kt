package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.Composable
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
import com.cantos.theatre.demo.DemoBookmark
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoEpisodeId
import com.cantos.theatre.demo.DemoLibraryTab
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
            onNodeWithTag("simulation-status").assertTextEquals("Đang mô phỏng · Không phát âm thanh")
            onNodeWithTag("transport").performScrollTo().performClick()
            onNodeWithTag("simulation-status").assertTextEquals("Mô phỏng đang dừng")
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
            onNodeWithTag("library-Bookmarks").performScrollTo().performClick().assertIsSelected()
            onAllNodesWithContentDescription("Mở dấu trang Người giữ tiếng chuông, 12 phút 33 giây")
                .assertCountEquals(1)
        }
    }

    @Test
    fun vietnamese_search_empty_result_and_clear_recover_the_visible_catalogue() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { DemoTestRoot() }
            onNodeWithTag("nav-search").performClick().assertIsSelected()
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
            onNodeWithTag("library-Saved").performScrollTo().performClick().assertIsSelected()
            onNodeWithTag("work-SonHa").assertExists()
            onNodeWithTag("work-DenKhua").assertDoesNotExist()
            onNodeWithTag("work-SonHa").performScrollTo().performClick()
            onNodeWithTag("save-work").performScrollTo().performClick()
            onNodeWithTag("save-work").assert(hasText("Lưu tác phẩm"))
            onNodeWithTag("nav-library").performClick()
            onNodeWithTag("library-Saved").assertIsSelected()
            onNodeWithText("Tủ sách đang chờ bạn").assertExists()
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
            onNodeWithTag("settings").performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Cài đặt nghe"))
            onNodeWithTag("theme-Dark").performScrollTo().performClick().assertIsSelected()
            onNodeWithTag("back").performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Phòng nghe"))
            onNodeWithTag("elapsed").assertTextEquals("15:00")
            onNodeWithTag("simulation-status").assertTextEquals("Mô phỏng đang dừng")
            onNodeWithTag("transport").assert(hasText("Mô phỏng phát"))
        }
    }

    @Test
    fun escape_from_a_focused_settings_control_returns_to_the_search_query() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { DemoTestRoot() }
            onNodeWithTag("nav-search").performClick()
            onNodeWithTag("search-input").performTextReplacement("Đèn Khuya")
            onNodeWithTag("settings").performClick()
            val choice = onNodeWithTag("theme-Dark").performScrollTo()
            choice.performSemanticsAction(SemanticsActions.RequestFocus) { assertTrue(it()) }
            choice.assertIsFocused().performKeyInput { pressKey(Key.Escape) }
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
            onNodeWithTag("settings").performClick()
            onNodeWithTag("catalog-Error").performScrollTo().performClick().assertIsSelected()
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
    fun reset_from_settings_restores_initial_locale_position_and_empty_bookmarks() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                DemoTestRoot(initialState = DemoState(
                    stack = listOf(DemoRoute.Search, DemoRoute.Settings),
                    positionSeconds = 900,
                    simulating = true,
                    savedWorks = emptySet(),
                    bookmarks = listOf(DemoBookmark(DemoEpisodeId.SonHa03, 900)),
                    libraryTab = DemoLibraryTab.Bookmarks,
                    query = "Đèn Khuya",
                    catalogMode = DemoCatalogMode.Error,
                    theme = DemoTheme.Dark,
                    locale = DemoLocale.English,
                    speedPercent = 150,
                ))
            }
            onNodeWithTag("reset").performScrollTo().assert(hasText("Reset demo")).performClick()
            onNodeWithTag("screen").assert(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, "Khám phá"))
            onNodeWithTag("mini-open").assert(hasText("12:48"))
            onNodeWithTag("mini-transport").assert(hasContentDescription("Mô phỏng phát Người giữ tiếng chuông"))
            onNodeWithTag("nav-library").performClick()
            onNodeWithTag("library-Bookmarks").performScrollTo().performClick()
            onNodeWithText("Tủ sách đang chờ bạn").assertExists()
            onNodeWithTag("nav-search").performClick()
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
) {
    Box(Modifier.requiredSize(width.dp, height.dp)) {
        CantosDemoApp(initialState = initialState, textScale = textScale)
    }
}
