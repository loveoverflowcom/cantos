package com.cantos.theatre.app

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.domain.identity.RegistrationPhase
import com.cantos.theatre.domain.identity.RegistrationState
import com.cantos.theatre.features.auth.presentation.AuthHeader
import com.cantos.theatre.features.auth.presentation.AuthIntent
import com.cantos.theatre.features.auth.presentation.AuthReturn
import com.cantos.theatre.features.auth.presentation.RegisterScreen
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.ui.CantosExpression
import com.cantos.ui.CantosTheme
import com.cantos.ui.UiPreferences
import kotlin.test.assertEquals
import org.junit.Test

/** Production composables over every signup phase; JVM semantics, never native Back evidence. */
@OptIn(ExperimentalTestApi::class)
class RegistrationExitUiTest {
    @Test
    fun every_phase_has_one_normal_exit_and_unavailable_keeps_distinct_browsing_recovery() {
        val phases = listOf(
            RegistrationPhase.Idle, RegistrationPhase.Editing, RegistrationPhase.Submitting,
            RegistrationPhase.Waiting, RegistrationPhase.Cancelled,
        ) + RegistrationFailure.entries.map { RegistrationPhase.Failed(it) }
        for (locale in ShellLocale.entries) runDesktopComposeUiTest(width = 390, height = 844) {
            val state = mutableStateOf<RegistrationState?>(RegistrationState())
            val intents = mutableListOf<AuthIntent>()
            setContent {
                CantosTheme(UiPreferences(expression = CantosExpression.Theatre)) {
                    Column(Modifier.verticalScroll(rememberScrollState())) {
                        AuthHeader(locale, ShellTheme.Light, AuthReturn.Login, intents::add)
                        RegisterScreen(locale, intents::add, state.value)
                    }
                }
            }
            for (phase in phases) {
                runOnIdle { state.value = RegistrationState("listener@example.test", phase) }
                onAllNodesWithTag("back").assertCountEquals(1)
                for (tag in listOf("register-cancel", "register-login", "register-home")) onNodeWithTag(tag).assertDoesNotExist()
                for (text in listOf("Hủy đăng ký", "Cancel registration", "Về đăng nhập", "Return to sign in")) {
                    onAllNodesWithText(text).assertCountEquals(0)
                }
                val needsRecovery = phase == RegistrationPhase.Cancelled ||
                    phase is RegistrationPhase.Failed && phase.reason != RegistrationFailure.InvalidEmail
                onAllNodesWithTag("register-retry").assertCountEquals(if (needsRecovery) 1 else 0)
            }
            runOnIdle { state.value = null }
            onNodeWithTag("register-unavailable").assertExists()
            onNodeWithTag("register-login").assertDoesNotExist()
            onNodeWithTag("register-home").performScrollTo().performClick()
            assertEquals(listOf<AuthIntent>(AuthIntent.OpenHome), intents)
            onNodeWithTag("back").performScrollTo().performClick()
            assertEquals(listOf<AuthIntent>(AuthIntent.OpenHome, AuthIntent.Back), intents)
        }
    }
}
