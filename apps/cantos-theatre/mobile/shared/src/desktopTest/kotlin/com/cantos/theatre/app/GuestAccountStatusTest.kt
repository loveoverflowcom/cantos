package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.identity.AccountLoginFailure
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.features.auth.presentation.AccountContent
import com.cantos.theatre.features.auth.presentation.LoginAttempt
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.ui.CantosTheme
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import org.junit.Test

/**
 * The guest Account card over a fixed [AccountView] and [LoginAttempt]: which owner states it states,
 * and that a failed attempt's message is left to Login (#579). The real owner, shell and Back are in
 * the preview-sandbox journeys; nothing here is a device, system Back or a screen reader.
 */
@OptIn(ExperimentalTestApi::class)
class GuestAccountStatusTest {
    private val refused = AccountView.Failed(NativeAccountFailure.Api, AccountLoginFailure.InvalidCredentials)

    private fun render(
        account: AccountView,
        attempt: LoginAttempt,
        locale: ShellLocale = ShellLocale.Vietnamese,
        theme: ShellTheme = ShellTheme.Light,
        body: androidx.compose.ui.test.ComposeUiTest.() -> Unit,
    ) = runDesktopComposeUiTest(width = 390, height = 844) {
        setContent {
            Box(Modifier.requiredSize(390.dp, 844.dp)) {
                CantosTheme(UiPreferences(theme = if (theme == ShellTheme.Dark) Theme.Dark else Theme.Light)) {
                    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                        AccountContent(locale, theme, account, {}, FocusRequester(), attempt) { }
                    }
                }
            }
        }
        body()
    }

    @Test
    fun the_failed_attempts_own_message_is_not_repeated_on_the_guest_card_in_either_locale_or_theme() {
        for (locale in ShellLocale.entries) for (theme in ShellTheme.entries) render(refused, LoginAttempt.Settled(refused), locale, theme) {
            onNodeWithTag("account-profile").assertExists()
            onNodeWithTag("auth-status").assertDoesNotExist()
            onNodeWithText("Không thể đăng nhập bằng thông tin này.").assertDoesNotExist()
            onNodeWithText("Unable to sign in with these credentials.").assertDoesNotExist()
            // The guest is still a resting state that may try again; nothing is faked as signed in.
            onNodeWithTag("sign-in").performScrollTo().assertIsEnabled()
            onNodeWithTag("create-account").performScrollTo().assertIsEnabled()
            onNodeWithTag("open-library").assertDoesNotExist()
        }
    }

    @Test
    fun the_same_failure_without_a_login_attempt_stays_visible() = render(refused, LoginAttempt.None) {
        onNodeWithTag("auth-status").assertExists()
        onNodeWithText("Không thể đăng nhập bằng thông tin này.").assertExists()
    }

    @Test
    fun a_different_failure_stays_visible_while_an_attempt_is_settled() {
        val expired = AccountView.Failed(NativeAccountFailure.Expired)
        render(expired, LoginAttempt.Settled(refused)) {
            onNodeWithTag("auth-status").assertExists()
        }
    }

    @Test
    fun cleanup_and_pending_stay_visible_with_their_own_controls() {
        render(AccountView.CleanupRequired, LoginAttempt.Settled(refused)) {
            onNodeWithTag("auth-status").assertExists()
            onNodeWithText("Cần hoàn tất xóa phiên cũ trước khi đăng nhập lại.").assertExists()
            onNodeWithTag("sign-in").performScrollTo().assertIsNotEnabled()
            onNodeWithTag("auth-retry-cleanup").performScrollTo().assertIsEnabled()
        }
        render(AccountView.Pending, LoginAttempt.Open) {
            onNodeWithTag("auth-status").assertExists()
            onNodeWithTag("sign-in").performScrollTo().assertIsNotEnabled()
        }
    }
}
