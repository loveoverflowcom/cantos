package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.*
import androidx.compose.ui.unit.dp
import com.cantos.theatre.adapters.preview.PreviewLoginMode
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import kotlin.test.assertEquals
import org.junit.Test

/** JVM production form/shell/controller interactions; no native secure-store or real auth claim. */
@OptIn(ExperimentalTestApi::class)
class IdentityV1LoginUiTest {
    private fun run(locale: ShellLocale = ShellLocale.English, body: ComposeUiTest.(PreviewOwners) -> Unit) =
        runDesktopComposeUiTest(width = 390, height = 844) {
            lateinit var owners: PreviewOwners
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    PreviewTestRoot(PreviewSeed(previewState(stack = listOf(ShellRoute.Discovery, ShellRoute.Login), locale = locale)),
                        onOwners = { owners = it; it.identity.mode = PreviewLoginMode.Hold })
                }
            }
            body(owners)
        }
    private fun ComposeUiTest.input() {
        onNodeWithTag("login-email").performScrollTo().performTextInput("listener@example.test")
        onNodeWithTag("login-password").performScrollTo().performTextInput("form-only-secret")
    }
    private fun ComposeUiTest.submit() { onNodeWithTag("login-primary").performScrollTo().performClick(); waitForIdle() }
    private fun ComposeUiTest.password() = onNodeWithTag("login-password").fetchSemanticsNode().config[SemanticsProperties.EditableText].text

    @Test fun generated_locale_updates_keep_focused_input_and_a_pending_login_in_the_same_mount() = run(ShellLocale.Vietnamese) { h ->
        input()
        onNodeWithTag("login-password").performScrollTo().performClick()
        onNodeWithTag("login-password").assertIsFocused()
        val maskedPassword = password()
        runOnIdle { h.preferences.setLocale(ShellLocale.English.toPreferenceLocale()) }
        onNodeWithText("Show password").assertExists()
        onNodeWithTag("login-password").assertIsFocused()
        assertEquals(maskedPassword, password())
        onNodeWithTag("login-email").assert(hasText("listener@example.test"))
        onNodeWithText("Show password").performScrollTo().performClick()
        assertEquals("form-only-secret", password())
        submit()
        val account = h.account.state.value
        val player = h.runtime.listening.playback.state.value
        val library = h.shell.library.value
        runOnIdle { h.preferences.setLocale(ShellLocale.Vietnamese.toPreferenceLocale()) }
        onNodeWithTag("login-primary").assertIsNotEnabled()
        onNodeWithText("Đang xác minh phiên đăng nhập…").assertExists()
        assertEquals(1, h.identity.loginRequests)
        assertEquals("", password())
        assertEquals(account, h.account.state.value)
        assertEquals(player, h.runtime.listening.playback.state.value)
        assertEquals(library, h.shell.library.value)
    }

    @Test fun invalid_fields_never_dispatch_and_keyboard_moves_email_to_password() = run { h ->
        submit(); assertEquals(0, h.identity.loginRequests)
        onNodeWithText("Enter a valid email address.").assertExists()
        onNodeWithText("Enter your password.").assertExists()
        onNodeWithTag("login-email").performScrollTo().performTextInput("listener@example.test")
        onNodeWithTag("login-email").performImeAction()
        onNodeWithTag("login-password").assertIsFocused()
        onNodeWithTag("login-password").performTextInput("form-only-secret")
        onNodeWithTag("login-password").performImeAction(); waitForIdle()
        assertEquals(1, h.identity.loginRequests); assertEquals("", password())
        onNodeWithTag("login-primary").assertIsNotEnabled()
    }
    @Test fun credentials_clear_on_leave_and_visibility_is_local_to_the_form() = run { h ->
        input()
        onNodeWithText("Show password").performScrollTo().performClick()
        onNodeWithText("Hide password").assertExists()
        onNodeWithTag("back").performClick(); waitForIdle()
        assertEquals(0, h.identity.loginRequests)
        h.shell.openLogin(); waitForIdle()
        assertEquals("", password()); onNodeWithText("Show password").assertExists()
    }
    @Test fun generic_401_429_and_503_require_explicit_retry_with_a_fresh_password_in_both_locales() {
        for (locale in ShellLocale.entries) for (failure in listOf(IdentityV1Failure.Unauthorized, IdentityV1Failure.RateLimited(17), IdentityV1Failure.ServiceUnavailable)) run(locale) { h ->
            h.identity.loginFailure = failure
            input(); submit(); h.identity.completeLogin(true); waitForIdle()
            assertEquals("", password()); assertEquals(1, h.identity.loginRequests)
            val message = when (failure) {
                IdentityV1Failure.Unauthorized -> if (locale == ShellLocale.English) "Unable to sign in with these credentials." else "Không thể đăng nhập bằng thông tin này."
                is IdentityV1Failure.RateLimited -> if (locale == ShellLocale.English) "Please wait 17 seconds before trying again." else "Vui lòng đợi 17 giây trước khi thử lại."
                else -> if (locale == ShellLocale.English) "Sign-in is temporarily unavailable. Please try again later." else "Đăng nhập tạm thời chưa khả dụng. Vui lòng thử lại sau."
            }
            onNodeWithText(message).assertExists()
            submit(); assertEquals(1, h.identity.loginRequests)
        }
    }
    @Test fun signup_collects_email_only_and_check_mail_never_adopts_or_polls_a_session() = run { h ->
        onNodeWithTag("login-create").performScrollTo().performClick(); waitForIdle()
        onNodeWithTag("register-email").performScrollTo().performTextInput("listener@example.test")
        onNodeWithTag("login-password").assertDoesNotExist()
        onNodeWithTag("register-submit").performScrollTo().performClick(); waitForIdle()
        onNodeWithTag("register-status").assertTextContains("come back to this app to sign in", substring = true)
        onNodeWithTag("register-link").assertDoesNotExist(); onNodeWithTag("register-confirm").assertDoesNotExist()
        onNodeWithTag("register-setup").assertDoesNotExist()
        assertEquals(0, h.identity.loginRequests); assertEquals(0, h.identity.admissions)
        onNodeWithTag("register-login").performScrollTo().performClick(); waitForIdle()
        assertEquals("", password()); assertEquals(0, h.identity.loginRequests)
    }
}
