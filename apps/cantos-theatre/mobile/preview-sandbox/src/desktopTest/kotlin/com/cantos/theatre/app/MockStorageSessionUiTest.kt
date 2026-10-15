package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.adapters.mock.sandbox.sessions.MockStorageSession
import org.junit.Test

/** The Android preview fixtures rendered by the JVM desktop renderer; not Android rendering. */
@OptIn(ExperimentalTestApi::class)
class MockStorageSessionUiTest {
    @Test
    fun fresh_session_shows_empty_continue_saved_and_history_groups() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) { MockStorageSessionApp(MockStorageSession.Fresh) }
            }
            onNodeWithTag("library-continue").assertExists()
            onNodeWithText("Chưa có câu chuyện để nghe tiếp").assertExists()
            onNodeWithTag("library-saved").performScrollTo()
            onNodeWithText("Chưa lưu câu chuyện nào").assertExists()
            onNodeWithTag("library-history").performScrollTo()
            onNodeWithText("Chưa có lịch sử nghe").assertExists()
            listOf("resume-SonHa03", "resume-DenKhua01", "work-SonHa", "work-DenKhua", "recent-SonHa03")
                .forEach { onNodeWithTag(it).assertDoesNotExist() }
            onNodeWithTag("mini-open").performClick()
            onNodeWithTag("elapsed").assertTextEquals("0:00")
            onNodeWithText("Dấu trang").assertDoesNotExist()
        }
    }

    @Test
    fun restored_session_shows_continue_saved_history_and_bookmark_from_one_snapshot() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) { MockStorageSessionApp(MockStorageSession.Restored) }
            }
            onNodeWithTag("resume-SonHa03")
                .assert(hasContentDescription("Mở vị trí nghe mẫu Người giữ tiếng chuông tại 12 phút 48 giây"))
            onNodeWithTag("resume-DenKhua01")
                .assert(hasContentDescription("Mở vị trí nghe mẫu Ánh đèn bên cửa sổ tại 1 phút 31 giây"))
            onNodeWithTag("library-saved").performScrollTo()
            onNodeWithTag("work-SonHa").assertExists()
            onNodeWithTag("work-DenKhua").assertExists()
            onNodeWithTag("work-CanhBuom").assertDoesNotExist()
            onNodeWithTag("library-history").performScrollTo()
            onNodeWithTag("recent-SonHa03").assertExists()
            onNodeWithTag("recent-DenKhua01").assertExists()
            onNodeWithTag("mini-open").performClick()
            onNodeWithTag("elapsed").assertTextEquals("12:48")
            onAllNodesWithContentDescription("Mở dấu trang Người giữ tiếng chuông, 10 phút 0 giây").assertCountEquals(1)
        }
    }
}
