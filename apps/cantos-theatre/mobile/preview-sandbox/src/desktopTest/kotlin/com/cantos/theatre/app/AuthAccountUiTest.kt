package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.Composable
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
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.adapters.preview.PreviewAccount
import com.cantos.theatre.adapters.preview.PreviewLoginMode
import com.cantos.theatre.app.PreviewOwners
import com.cantos.theatre.app.PreviewTestRoot
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
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
        backHandler: @Composable (Boolean, () -> Unit) -> Unit = { _, _ -> },
        body: ComposeUiTest.(PreviewOwners) -> Unit,
    ) = runDesktopComposeUiTest(width = width, height = height) {
        lateinit var owners: PreviewOwners
        setContent {
            Box(Modifier.requiredSize(width.dp, height.dp)) {
                // The login response is held so each test decides when, and whether, it arrives.
                PreviewTestRoot(textScale = textScale, backHandler = backHandler,
                    onOwners = { it.identity.mode = PreviewLoginMode.Hold; owners = it })
            }
        }
        body(owners)
    }

    private fun ComposeUiTest.openLoginFromAccount() {
        onNodeWithTag("account").performClick()
        onNodeWithTag("sign-in").performScrollTo().assertIsEnabled().performClick()
        waitForIdle()
    }

    private fun ComposeUiTest.submitLogin() {
        onNodeWithTag("login-email").performScrollTo().performTextReplacement("listener@example.test")
        onNodeWithTag("login-password").performScrollTo().performTextReplacement("preview-only")
        onNodeWithTag("login-primary").performScrollTo().performClick()
    }

    @Test
    fun login_is_a_focused_task_with_vertical_actions_and_back_restores_the_account_entry() = run { h ->
        openLoginFromAccount()
        onNodeWithTag("back").assertIsDisplayed().assertContentDescriptionEquals("Về Tài khoản")
        onNodeWithTag("login-create").performScrollTo().assertIsDisplayed()
        // Compare both controls in the same scrolled coordinate frame.
        val primary = onNodeWithTag("login-primary").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        val create = onNodeWithTag("login-create").fetchSemanticsNode().boundsInRoot
        assertTrue(primary.bottom <= create.top)
        assertEquals(primary.width, create.width)
        // Auth owns the form; tab navigation and mini-player never cover its controls.
        onNodeWithTag("nav-discovery").assertDoesNotExist()
        onNodeWithTag("mini-player").assertDoesNotExist()
        onAllNodes(hasSetTextAction()).assertCountEquals(2)
        onNodeWithText("Điều khoản", substring = true).assertDoesNotExist()
        onNodeWithText("Quyền riêng tư", substring = true).assertDoesNotExist()
        // Opening Login starts nothing: only the listener's explicit action sends credentials.
        assertEquals(0, h.identity.loginRequests)

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
        assertEquals(signIn.height, create.height, "Both active actions form one equal-height pill pair")
        val avatar = onNodeWithTag("account-avatar").fetchSemanticsNode().boundsInRoot
        assertEquals(40f, avatar.width, "The decorative guest avatar is compact, not a control")
        assertEquals(avatar.width, avatar.height)
        onNodeWithText("Người nghe khách").assertDoesNotExist()
        onNodeWithText("Đăng nhập để mở thư viện của bạn.").assertExists()
        onNodeWithText("Về chế độ nghe không đăng nhập").assertDoesNotExist()
        onNodeWithTag("account-membership").assertExists()
        onNodeWithTag("settings").assertExists()
        // Nothing here is an invented name, total, conflict or Membership entitlement.
        onNodeWithText("Minh An", substring = true).assertDoesNotExist()
        onNodeWithText("12:48 / 18:25", substring = true).assertDoesNotExist()
        assertEquals(0, h.identity.loginRequests)
    }

    @Test
    fun pending_login_blocks_a_second_request_and_cancel_returns_to_the_entry_without_reopening() = run { h ->
        openLoginFromAccount()
        submitLogin()
        waitForIdle()
        assertEquals(1, h.identity.loginRequests)
        onNodeWithTag("login-primary").performScrollTo().assertIsNotEnabled()
        onNodeWithTag("login-create").assertDoesNotExist()
        onNodeWithText("Đang xác minh phiên đăng nhập…").assertExists()

        onNodeWithTag("auth-cancel").performScrollTo().performClick()
        waitForIdle()
        onNodeWithTag("login-primary").performScrollTo().assertIsEnabled()
        assertEquals(NativeAccountState.SignedOut, h.account.state.value)
        assertEquals(1, h.identity.loginRequests)
    }

    @Test
    fun credential_refusal_stays_on_login_and_waits_for_another_explicit_attempt() = run { h ->
        openLoginFromAccount()
        submitLogin()
        waitForIdle()
        h.identity.completeLogin(approve = false)
        waitForIdle()
        onNodeWithText("Không thể đăng nhập bằng thông tin này. Hãy kiểm tra email và mật khẩu rồi thử lại.").assertExists()
        onNodeWithTag("login-primary").performScrollTo().assertIsEnabled()
        assertEquals(1, h.identity.loginRequests)
        assertEquals(0, h.identity.admissions)

        submitLogin()
        waitForIdle()
        assertEquals(2, h.identity.loginRequests)
    }

    @Test
    fun back_during_a_pending_login_fences_a_late_login_response() = run { h ->
        openLoginFromAccount()
        submitLogin()
        waitForIdle()
        onNodeWithTag("back").performClick()
        waitForIdle()
        onNodeWithTag("sign-in").assertExists()

        h.identity.completeLogin(approve = true)
        waitForIdle()
        assertEquals(0, h.identity.admissions)
        assertEquals(NativeAccountState.SignedOut, h.account.state.value)
        onNodeWithTag("sign-in").assertIsEnabled()
        onNodeWithTag("open-library").assertDoesNotExist()
    }

    private enum class BackKind { Header, System }

    // #579: the attempt's message is Login feedback. Header and system Back share one shell path,
    // and the owner keeps its Failed state: Back hides a label, it never resets the account.
    @Test
    fun a_failed_attempt_stays_on_login_and_back_leaves_no_stale_status_on_the_guest_card() {
        val cases = listOf(
            IdentityV1Failure.Unauthorized to "Không thể đăng nhập bằng thông tin này. Hãy kiểm tra email và mật khẩu rồi thử lại.",
            IdentityV1Failure.RateLimited(17) to "Vui lòng đợi 17 giây trước khi thử lại.",
            IdentityV1Failure.ServiceUnavailable to "Đăng nhập tạm thời chưa khả dụng. Vui lòng thử lại sau.",
        )
        for ((failure, message) in cases) for (kind in BackKind.entries) {
            var systemBack: (() -> Unit)? = null
            run(backHandler = { enabled, onBack -> systemBack = onBack.takeIf { enabled } }) { h ->
                h.identity.loginFailure = failure
                openLoginFromAccount()
                submitLogin()
                waitForIdle()
                h.identity.completeLogin(approve = false)
                waitForIdle()
                onNodeWithTag("auth-status").assertExists()
                onNodeWithText(message).assertExists()

                when (kind) {
                    BackKind.Header -> onNodeWithTag("back").performClick()
                    BackKind.System -> runOnIdle { assertNotNull(systemBack, "Login owns system Back").invoke() }
                }
                waitForIdle()
                onNodeWithTag("sign-in").assertIsFocused().assertIsEnabled()
                onNodeWithTag("auth-status").assertDoesNotExist()
                onNodeWithText(message).assertDoesNotExist()
                assertIs<NativeAccountState.Failed>(h.account.state.value, "$failure after $kind Back")
                assertEquals(1, h.identity.loginRequests, "Back never replays the request")

                // Re-entry starts clean, and a retry reports its own result on Login again.
                onNodeWithTag("sign-in").performScrollTo().performClick()
                waitForIdle()
                onNodeWithTag("auth-status").assertDoesNotExist()
                onNodeWithText(message).assertDoesNotExist()
                submitLogin()
                waitForIdle()
                h.identity.completeLogin(approve = false)
                waitForIdle()
                onNodeWithText(message).assertExists()
                assertEquals(2, h.identity.loginRequests)
            }
        }
    }

    @Test
    fun back_during_a_pending_login_leaves_no_status_when_a_late_refusal_arrives() = run { h ->
        openLoginFromAccount()
        submitLogin()
        waitForIdle()
        onNodeWithTag("back").performClick()
        waitForIdle()
        h.identity.completeLogin(approve = false)
        waitForIdle()
        assertEquals(NativeAccountState.SignedOut, h.account.state.value)
        onNodeWithTag("auth-status").assertDoesNotExist()
        onNodeWithText("Không thể đăng nhập bằng thông tin này. Hãy kiểm tra email và mật khẩu rồi thử lại.").assertDoesNotExist()
        onNodeWithTag("sign-in").assertIsEnabled()
    }

    @Test
    fun a_failure_the_login_entry_did_not_request_stays_on_the_guest_card() = run { h ->
        // Negative control: the same typed refusal, reached without a Login attempt, is account status.
        onNodeWithTag("account").performClick()
        waitForIdle()
        h.identity.mode = PreviewLoginMode.Deny
        val email = assertIs<RegistrationResult.Accepted<RegistrationEmail>>(RegistrationEmail.parse("listener@example.test")).value
        runOnIdle { h.account.login(email, "preview-only") }
        waitUntil(timeoutMillis = 5_000) { h.account.state.value is NativeAccountState.Failed }
        waitForIdle()
        onNodeWithTag("auth-status").assertExists()
        onNodeWithText("Không thể đăng nhập bằng thông tin này. Hãy kiểm tra email và mật khẩu rồi thử lại.").assertExists()
    }

    @Test
    fun a_confirmed_session_returns_to_account_and_shows_only_what_the_owner_may_project() = run { h ->
        openLoginFromAccount()
        submitLogin()
        waitForIdle()
        h.identity.completeLogin(approve = true)
        waitForIdle()
        onNodeWithTag("sign-in").assertDoesNotExist()
        onNodeWithTag("open-library").performScrollTo().assertIsDisplayed()
        onNodeWithText("Đã đăng nhập").assertExists()
        onNodeWithTag("logout").performScrollTo().assertIsDisplayed()
        // No actor, session or credential, and no invented name, total or conflict.
        onAllNodes(hasText(PreviewAccount.Linh.actor, substring = true)).assertCountEquals(0)
        onAllNodes(hasText(PreviewAccount.Linh.session, substring = true)).assertCountEquals(0)
        onAllNodes(hasText("ci1_n_", substring = true)).assertCountEquals(0)
        onNodeWithText("Minh An", substring = true).assertDoesNotExist()
    }

    @Test
    fun logout_hides_the_account_first_and_keeps_login_blocked_until_cleanup_succeeds() = run { h ->
        openLoginFromAccount()
        submitLogin()
        waitForIdle()
        h.identity.completeLogin(approve = true)
        waitForIdle()

        h.identity.clearFails = true
        onNodeWithTag("logout").performScrollTo().performClick()
        waitForIdle()
        onNodeWithTag("logout-confirm").performClick()
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
    fun registration_collects_only_email_and_returns_through_login_to_its_origin() = run { h ->
        openLoginFromAccount()
        onNodeWithTag("login-create").performScrollTo().performClick()
        waitForIdle()
        onNodeWithTag("back").assertContentDescriptionEquals("Về Đăng nhập")
        onNodeWithTag("register-email").performScrollTo().assertIsDisplayed()
        onNodeWithTag("login-password").assertDoesNotExist()
        onAllNodes(hasSetTextAction()).assertCountEquals(1)
        onNodeWithText("Đăng ký hoàn tất", substring = true).assertDoesNotExist()

        onNodeWithTag("back").performClick()
        waitForIdle()
        onNodeWithTag("login-primary").performScrollTo().assertIsDisplayed()
        onNodeWithTag("back").performClick()
        waitForIdle()
        onNodeWithTag("sign-in").assertIsFocused()
        assertEquals(0, h.identity.loginRequests)
    }

    @Test
    fun large_text_on_a_narrow_screen_keeps_back_logo_theme_and_both_actions_reachable() = run(width = 320, height = 568, textScale = 2f) {
        openLoginFromAccount()
        val back = onNodeWithTag("back").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        val theme = onNodeWithTag("auth-theme").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        val logo = onNodeWithTag("header-title").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        // Back is icon-only: it keeps a 48dp touch target, and the logo sits beside it, never dropped.
        assertTrue(back.width >= 48f * density.density - 1f && back.height >= 48f * density.density - 1f)
        assertTrue(back.left >= 0f && back.right <= logo.left && logo.right <= theme.left)
        assertTrue(theme.right <= 320f * density.density)
        assertTrue(theme.width >= 48f * density.density - 1f && theme.height >= 48f * density.density - 1f)
        onNodeWithTag("login-primary").performScrollTo().assertIsDisplayed()
        onNodeWithTag("login-create").performScrollTo().assertIsDisplayed()
    }
}
