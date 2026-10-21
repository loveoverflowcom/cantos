package com.cantos.theatre.features.auth.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.domain.identity.RegistrationPhase
import com.cantos.theatre.domain.identity.RegistrationState
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.DisabledReason
import com.cantos.ui.FieldState
import com.cantos.ui.TextField
import com.cantos.ui.TextFieldVariant
import com.cantos.ui.tokenDp

sealed interface RegistrationIntent {
    class Email(val value: String) : RegistrationIntent { override fun toString() = "RegistrationIntent.Email([redacted])" }
    class Import(val link: String) : RegistrationIntent { override fun toString() = "RegistrationIntent.Import([redacted])" }
    data object Submit : RegistrationIntent
    data object Confirm : RegistrationIntent
    data object Cancel : RegistrationIntent
    data object OpenSetup : RegistrationIntent
    data object Restart : RegistrationIntent
}

/** Values and intents only. Link input is local RAM, never rememberSaveable or a route value. */
@Composable
internal fun RegistrationForm(state: RegistrationState, locale: ShellLocale, onIntent: (RegistrationIntent) -> Unit) {
    val copy = AuthCopy(locale)
    val phase = state.phase
    val failure = (phase as? RegistrationPhase.Failed)?.reason
    Column(Modifier.fillMaxWidth().testTag("registration-form"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        val editing = phase == RegistrationPhase.Editing || phase == RegistrationPhase.Submitting || failure == RegistrationFailure.InvalidEmail
        if (editing) {
            TextField(copy.text(AuthText.RegisterEmail), state.email, { onIntent(RegistrationIntent.Email(it)) },
                modifier = Modifier.fillMaxWidth(), variant = TextFieldVariant.AuthTonal,
                state = if (phase == RegistrationPhase.Submitting) FieldState.ReadOnly else FieldState.Editable,
                error = if (failure == RegistrationFailure.InvalidEmail) copy.text(AuthText.RegisterInvalidEmail) else null,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email), controlModifier = Modifier.testTag("register-email"))
            Button(copy.text(AuthText.RegisterSend), { onIntent(RegistrationIntent.Submit) },
                variant = ButtonVariant.AuthPrimary, icon = "mail",
                state = if (phase == RegistrationPhase.Submitting) ButtonState.Loading(DisabledReason.of(copy.text(AuthText.RegisterSending))) else ButtonState.Enabled,
                controlModifier = Modifier.fillMaxWidth().testTag("register-submit"))
        }
        val status = when (phase) {
            RegistrationPhase.Idle, RegistrationPhase.Checking -> copy.text(AuthText.RegisterChecking)
            RegistrationPhase.Editing -> null
            RegistrationPhase.Submitting -> copy.text(AuthText.RegisterSending)
            RegistrationPhase.Waiting -> copy.text(AuthText.RegisterWaiting)
            is RegistrationPhase.Confirmable -> copy.text(AuthText.RegisterConfirmHelp)
            is RegistrationPhase.Verifying -> copy.text(AuthText.RegisterVerifying)
            is RegistrationPhase.SetupReady -> copy.text(AuthText.RegisterSetupReady)
            RegistrationPhase.CredentialSetup -> copy.text(AuthText.RegisterCredentialSetup)
            RegistrationPhase.Cancelling -> copy.text(AuthText.RegisterCancelling)
            RegistrationPhase.Cancelled -> copy.text(AuthText.RegisterCancelled)
            is RegistrationPhase.Failed -> copy.registrationFailure(phase.reason)
        }
        if (status != null) Text(status, Modifier.testTag("register-status").semantics {
            liveRegion = LiveRegionMode.Polite
            if (failure != null) error(status)
        }, color = if (failure == null) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.error)
        if (!state.firstParty && (phase == RegistrationPhase.Waiting || failure == RegistrationFailure.InvalidAction || failure == RegistrationFailure.StartUnknownOutcome)) {
            var link by remember { mutableStateOf("") }
            TextField(copy.text(AuthText.RegisterLink), link, { link = it.take(8192) },
                modifier = Modifier.fillMaxWidth(), variant = TextFieldVariant.AuthTonal,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri), controlModifier = Modifier.testTag("register-link"))
            Button(copy.text(AuthText.RegisterImport), { val candidate = link; link = ""; onIntent(RegistrationIntent.Import(candidate)) },
                variant = ButtonVariant.AuthPrimary, controlModifier = Modifier.fillMaxWidth().testTag("register-import"))
        }
        if (phase is RegistrationPhase.Confirmable) Button(copy.text(AuthText.RegisterConfirm), { onIntent(RegistrationIntent.Confirm) },
            variant = ButtonVariant.AuthPrimary, controlModifier = Modifier.fillMaxWidth().testTag("register-confirm"))
        if (phase is RegistrationPhase.SetupReady) Button(copy.text(AuthText.RegisterOpenSetup), { onIntent(RegistrationIntent.OpenSetup) },
            variant = ButtonVariant.AuthPrimary, controlModifier = Modifier.fillMaxWidth().testTag("register-setup"))
        if (phase != RegistrationPhase.Idle && phase != RegistrationPhase.Checking && phase != RegistrationPhase.Cancelled && phase !is RegistrationPhase.Failed && phase != RegistrationPhase.CredentialSetup) {
            Button(copy.text(AuthText.RegisterCancel), { onIntent(RegistrationIntent.Cancel) },
                variant = ButtonVariant.Text, icon = "close", controlModifier = Modifier.fillMaxWidth().testTag("register-cancel"))
        }
        if (phase is RegistrationPhase.Failed && phase.reason != RegistrationFailure.InvalidEmail && phase.reason != RegistrationFailure.InvalidAction || phase == RegistrationPhase.Cancelled) {
            Button(copy.text(if (failure == RegistrationFailure.Unavailable || failure == RegistrationFailure.Disabled) AuthText.RegisterRetry else AuthText.RegisterRestart),
                { onIntent(RegistrationIntent.Restart) }, variant = ButtonVariant.AuthPrimary,
                controlModifier = Modifier.fillMaxWidth().testTag("register-retry"))
        }
    }
}
