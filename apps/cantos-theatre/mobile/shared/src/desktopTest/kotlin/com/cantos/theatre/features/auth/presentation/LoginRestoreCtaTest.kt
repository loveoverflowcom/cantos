package com.cantos.theatre.features.auth.presentation

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.identity.AccountLoginFailure
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.CantosTheme
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.Test

/**
 * #540 (AC-14, AC-15, AC-16): Login offers no permanent restore action, keeps one task heading and
 * its password eye inside the field. The shell's startup restore and the typed owner are untouched;
 * JVM interactions say nothing about a device, TalkBack or the keyboard.
 */
@OptIn(ExperimentalTestApi::class)
class LoginRestoreCtaTest {
    private fun render(account: AccountView, intents: MutableList<AuthIntent> = mutableListOf(), body: ComposeUiTest.() -> Unit) =
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosTheme(UiPreferences(theme = Theme.Light)) {
                        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                            LoginScreen(ShellLocale.Vietnamese, account, nothingSaved = false) { intents += it }
                        }
                    }
                }
            }
            body()
        }

    @Test
    fun a_clean_or_credential_refused_login_has_no_restore_action() {
        val states = listOf(
            AccountView.SignedOut,
            AccountView.Failed(NativeAccountFailure.Api, AccountLoginFailure.InvalidCredentials),
            AccountView.Failed(NativeAccountFailure.Api, AccountLoginFailure.RetryLater),
            AccountView.Failed(NativeAccountFailure.Expired),
            AccountView.Failed(NativeAccountFailure.Protocol),
            AccountView.Failed(NativeAccountFailure.Exhausted),
        )
        for (state in states) render(state) {
            onAllNodesWithTag("auth-restore").assertCountEquals(0)
            onNodeWithText("Khôi phục phiên đã lưu").assertDoesNotExist()
            onNodeWithTag("login-primary").assertExists()
        }
    }

    @Test
    fun a_saved_session_that_could_not_be_read_keeps_one_retry_beside_its_message() {
        for (reason in listOf(NativeAccountFailure.Storage, NativeAccountFailure.Api)) {
            val intents = mutableListOf<AuthIntent>()
            render(AccountView.Failed(reason), intents) {
                onNodeWithTag("auth-status").assertExists()
                onNodeWithTag("auth-restore").performScrollTo().performClick()
                assertEquals(listOf<AuthIntent>(AuthIntent.RestoreSession), intents, reason.name)
            }
        }
    }

    @Test
    fun the_restore_retry_is_decided_by_the_typed_owner_and_never_by_a_credential_attempt() {
        assertTrue(AccountView.Failed(NativeAccountFailure.Storage).offersRestoreRetry())
        assertTrue(AccountView.Failed(NativeAccountFailure.Api).offersRestoreRetry())
        assertFalse(AccountView.Failed(NativeAccountFailure.Api, AccountLoginFailure.InvalidCredentials).offersRestoreRetry())
        assertFalse(AccountView.Failed(NativeAccountFailure.Storage, AccountLoginFailure.RetryLater).offersRestoreRetry())
        for (reason in listOf(NativeAccountFailure.Expired, NativeAccountFailure.Protocol, NativeAccountFailure.Exhausted)) {
            assertFalse(AccountView.Failed(reason).offersRestoreRetry(), reason.name)
        }
    }

    @Test
    fun login_keeps_one_heading_and_the_password_eye_inside_the_field() {
        render(AccountView.SignedOut) {
            onAllNodesWithTag("auth-heading").assertCountEquals(1)
            onNodeWithTag("login-password-eye").assertExists()
            onNodeWithContentDescription("Hiện mật khẩu").assertExists()
            onNodeWithText("Hiện mật khẩu").assertDoesNotExist() // no standalone text button
            onAllNodesWithTag("auth-brand").assertCountEquals(0)
        }
    }
}
