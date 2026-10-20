package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.adapters.preview.PreviewAccount
import com.cantos.theatre.adapters.preview.PreviewLoginMode
import com.cantos.theatre.app.PreviewOwners
import com.cantos.theatre.app.PreviewTestRoot
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

/**
 * The real guest shell, TheatreShell and account owner over deterministic identity ports. These
 * JVM interactions prove presentation, intents and fencing; they say nothing about the provider,
 * secure storage, the system browser, OS Back or a screen reader on a device.
 */
@OptIn(ExperimentalTestApi::class)
class AuthAccountUiTest {
    private fun run(
        width: Int = 390,
        height: Int = 844,
        textScale: Float = 1f,
        body: ComposeUiTest.(PreviewOwners) -> Unit,
    ) = runDesktopComposeUiTest(width = width, height = height) {
        lateinit var owners: PreviewOwners
        setContent {
            Box(Modifier.requiredSize(width.dp, height.dp)) {
                // The provider is held so each test decides when, and whether, the browser returns.
                PreviewTestRoot(textScale = textScale, onOwners = { it.identity.mode = PreviewLoginMode.Hold; owners = it })
            }
        }
        body(owners)
    }

    private fun ComposeUiTest.openLoginFromAccount() {
        onNodeWithTag("account").performClick()
        onNodeWithTag("sign-in").performScrollTo().assertIsEnabled().performClick()
        waitForIdle()
    }

    @Test
    fun login_is_a_focused_task_with_vertical_actions_and_back_restores_the_account_entry() = run { h ->
        openLoginFromAccount()
        onNodeWithTag("back").assertIsDisplayed().assertContentDescriptionEquals("Về Tài khoản")
        val primary = onNodeWithTag("login-primary").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        val create = onNodeWithTag("login-create").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        assertTrue(primary.bottom <= create.top)
        assertEquals(primary.width, create.width)
        // No tab bar or mini-player can cover Back or an action; no credential field or legal consent exists.
        onNodeWithTag("nav-discovery").assertDoesNotExist()
        onNodeWithTag("mini-player").assertDoesNotExist()
        onAllNodes(hasSetTextAction()).assertCountEquals(0)
        onNodeWithText("Điều khoản", substring = true).assertDoesNotExist()
        onNodeWithText("Quyền riêng tư", substring = true).assertDoesNotExist()
        // Opening Login starts nothing: only the listener's explicit action contacts the provider.
        assertEquals(0, h.identity.begins)
        assertEquals(0, h.identity.browserOpens)

        onNodeWithTag("back").performClick()
        waitForIdle()
        onNodeWithTag("sign-in").assertIsFocused()
        assertEquals(NativeAccountState.SignedOut, h.account.state.value)
    }

    @Test
    fun guest_account_offers_sign_in_then_create_account_as_one_vertical_hierarchy() = run { h ->
        onNodeWithTag("account").performClick()
        val profile = onNodeWithTag("account-profile").fetchSemanticsNode().boundsInRoot
        val signIn = onNodeWithTag("sign-in").performScrollTo().assertIsEnabled().fetchSemanticsNode().boundsInRoot
        val create = onNodeWithTag("create-account").performScrollTo().assertIsEnabled().fetchSemanticsNode().boundsInRoot
        assertTrue(signIn.bottom <= create.top)
        assertEquals(signIn.width, create.width)
        assertTrue(profile.left <= signIn.left && create.right <= profile.right)
        onNodeWithTag("account-membership").assertExists()
        onNodeWithTag("settings").assertExists()
        // Nothing here is an invented name, total, conflict or Membership entitlement.
        onNodeWithText("Minh An", substring = true).assertDoesNotExist()
        onNodeWithText("12:48 / 18:25", substring = true).assertDoesNotExist()
        assertEquals(0, h.identity.begins)
    }

    @Test
    fun pending_login_blocks_a_second_request_and_cancel_returns_to_the_entry_without_reopening() = run { h ->
        openLoginFromAccount()
        onNodeWithTag("login-primary").performClick()
        waitForIdle()
        assertEquals(1, h.identity.browserOpens)
        onNodeWithTag("login-primary").assertIsNotEnabled()
        onNodeWithTag("login-create").assertDoesNotExist()
        onNodeWithText("Đang xác minh phiên đăng nhập…").assertExists()

        onNodeWithTag("auth-cancel").performClick()
        waitForIdle()
        onNodeWithTag("login-primary").assertIsEnabled()
        assertEquals(NativeAccountState.SignedOut, h.account.state.value)
        assertEquals(1, h.identity.browserOpens)
    }

    @Test
    fun provider_cancellation_stays_on_login_and_waits_for_another_explicit_attempt() = run { h ->
        openLoginFromAccount()
        onNodeWithTag("login-primary").performClick()
        waitForIdle()
        h.identity.returnFromBrowser(approve = false)
        waitForIdle()
        onNodeWithText("Đăng nhập đã hủy hoặc hết hạn. Vui lòng thử lại.").assertExists()
        onNodeWithTag("login-primary").assertIsEnabled()
        assertEquals(1, h.identity.browserOpens)
        assertEquals(0, h.identity.exchanges)

        onNodeWithTag("login-primary").performClick()
        waitForIdle()
        assertEquals(2, h.identity.browserOpens)
    }

    @Test
    fun back_during_a_pending_login_fences_a_late_provider_return() = run { h ->
        openLoginFromAccount()
        onNodeWithTag("login-primary").performClick()
        waitForIdle()
        onNodeWithTag("back").performClick()
        waitForIdle()
        onNodeWithTag("sign-in").assertExists()

        h.identity.returnFromBrowser(approve = true)
        waitForIdle()
        assertEquals(0, h.identity.exchanges)
        assertEquals(NativeAccountState.SignedOut, h.account.state.value)
        onNodeWithTag("sign-in").assertIsEnabled()
        onNodeWithTag("open-library").assertDoesNotExist()
    }

    @Test
    fun a_confirmed_session_returns_to_account_and_shows_only_what_the_owner_may_project() = run { h ->
        openLoginFromAccount()
        onNodeWithTag("login-primary").performClick()
        waitForIdle()
        h.identity.returnFromBrowser(approve = true)
        waitForIdle()
        onNodeWithTag("sign-in").assertDoesNotExist()
        onNodeWithTag("open-library").performScrollTo().assertIsDisplayed()
        onNodeWithText("Đã đăng nhập").assertExists()
        onNodeWithTag("logout").performScrollTo().assertIsDisplayed()
        // No actor, session or credential, and no invented name, total or conflict.
        onAllNodes(hasText(PreviewAccount.Linh.actor, substring = true)).assertCountEquals(0)
        onAllNodes(hasText(PreviewAccount.Linh.session, substring = true)).assertCountEquals(0)
        onAllNodes(hasText("ca1_n_", substring = true)).assertCountEquals(0)
        onNodeWithText("Minh An", substring = true).assertDoesNotExist()
    }

    @Test
    fun logout_hides_the_account_first_and_keeps_login_blocked_until_cleanup_succeeds() = run { h ->
        openLoginFromAccount()
        onNodeWithTag("login-primary").performClick()
        waitForIdle()
        h.identity.returnFromBrowser(approve = true)
        waitForIdle()

        h.identity.clearFails = true
        onNodeWithTag("logout").performScrollTo().performClick()
        waitForIdle()
        onNodeWithTag("open-library").assertDoesNotExist()
        onNodeWithText("Cần hoàn tất xóa phiên cũ trước khi đăng nhập lại.").assertExists()
        onNodeWithTag("sign-in").performScrollTo().assertIsNotEnabled()

        h.identity.clearFails = false
        onNodeWithTag("auth-retry-cleanup").performScrollTo().performClick()
        waitForIdle()
        onNodeWithTag("sign-in").performScrollTo().assertIsEnabled()
        assertEquals(NativeAccountState.SignedOut, h.account.state.value)
    }

    @Test
    fun registration_is_an_honest_unavailable_step_between_login_and_its_origin() = run { h ->
        openLoginFromAccount()
        onNodeWithTag("login-create").performClick()
        waitForIdle()
        onNodeWithTag("back").assertContentDescriptionEquals("Về Đăng nhập")
        onNodeWithTag("register-unavailable").assertIsDisplayed()
        onNodeWithText("Đăng ký chưa sẵn sàng", substring = true).assertExists()
        onAllNodes(hasSetTextAction()).assertCountEquals(0)
        onNodeWithText("Đăng ký hoàn tất", substring = true).assertDoesNotExist()

        onNodeWithTag("back").performClick()
        waitForIdle()
        onNodeWithTag("login-primary").assertIsDisplayed()
        onNodeWithTag("back").performClick()
        waitForIdle()
        onNodeWithTag("sign-in").assertIsFocused()
        assertEquals(0, h.identity.begins)
        assertEquals(0, h.identity.browserOpens)
    }

    @Test
    fun large_text_on_a_narrow_screen_keeps_back_logo_theme_and_both_actions_reachable() = run(width = 320, height = 568, textScale = 2f) {
        openLoginFromAccount()
        val back = onNodeWithTag("back").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        val theme = onNodeWithTag("auth-theme").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        // Large text wraps Back's label; it never squeezes the toggle below its 48dp control size.
        assertTrue(back.left >= 0f && back.right <= theme.left)
        assertTrue(theme.right <= 320f * density.density)
        assertTrue(theme.width >= 48f * density.density - 1f && theme.height >= 48f * density.density - 1f)
        onNodeWithTag("login-primary").performScrollTo().assertIsDisplayed()
        onNodeWithTag("login-create").performScrollTo().assertIsDisplayed()
    }
}
