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
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.domain.identity.CompletionFailure
import com.cantos.theatre.domain.identity.EmailVerificationState
import com.cantos.theatre.domain.identity.PasswordIssue
import com.cantos.theatre.domain.identity.VerificationPhase
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.CantosTypography
import com.cantos.ui.Checkbox
import com.cantos.ui.DisabledReason
import com.cantos.ui.FieldState
import com.cantos.ui.TextField
import com.cantos.ui.TextFieldVariant
import com.cantos.ui.tokenDp

/** Explicit listener intents on the completion screen; the app root routes each to its owner. */
sealed interface VerificationIntent {
    class Password(val value: String) : VerificationIntent { override fun toString() = "VerificationIntent.Password([redacted])" }
    class Repeat(val value: String) : VerificationIntent { override fun toString() = "VerificationIntent.Repeat([redacted])" }
    data class Confirm(val checked: Boolean) : VerificationIntent
    data object Create : VerificationIntent
    data object Retry : VerificationIntent
    data object ChooseAnother : VerificationIntent
    /** Signed out: Login. Signed in: Account, where the explicit sign-out is. */
    data object Continue : VerificationIntent
    data object RequestAnother : VerificationIntent
}

/**
 * Values and intents only. The password lives in the owner's RAM while it is typed and is cleared
 * before the request leaves; nothing here is saveable, restored or shown after confirmation.
 */
@Composable
fun VerifyEmailScreen(
    locale: ShellLocale,
    state: EmailVerificationState,
    account: AccountView,
    retryWaitSeconds: Long?,
    onIntent: (VerificationIntent) -> Unit,
) {
    val copy = AuthCopy(locale)
    val phase = state.phase
    val signedIn = account == AccountView.SignedIn
    AuthTonalScope {
    Column(Modifier.fillMaxWidth().testTag("verify-email"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.lg"))) {
        Text(copy.text(verificationHeading(phase)), style = CantosTypography.authHeading,
            modifier = Modifier.testTag("auth-heading").semantics { heading() })
        val message = copy.text(verificationMessage(phase, signedIn))
        val failed = phase is VerificationPhase.CompleteRefused || phase == VerificationPhase.LinkRefused ||
            phase == VerificationPhase.Idle
        val live = Modifier.testTag("verify-status").semantics { liveRegion = LiveRegionMode.Polite }
        if (failed) ErrorText(message, live)
        else Text(message, live, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        when (phase) {
            VerificationPhase.AwaitingConfirmation, VerificationPhase.Completing -> PasswordForm(state, copy, onIntent)
            is VerificationPhase.CompleteRetry -> Button(copy.text(AuthText.VerifyRetrySame), { onIntent(VerificationIntent.Retry) },
                variant = ButtonVariant.AuthPrimary, icon = "refresh",
                state = if (retryWaitSeconds == null) ButtonState.Enabled
                else ButtonState.Disabled(DisabledReason.of(legacyRetryAfter(locale, retryWaitSeconds))),
                controlModifier = Modifier.fillMaxWidth().testTag("verify-retry"))
            VerificationPhase.CompleteRefused(CompletionFailure.Invalid) -> Button(copy.text(AuthText.VerifyChoosePassword),
                { onIntent(VerificationIntent.ChooseAnother) }, variant = ButtonVariant.AuthPrimary,
                controlModifier = Modifier.fillMaxWidth().testTag("verify-choose"))
            is VerificationPhase.Completed -> Button(copy.text(if (signedIn) AuthText.VerifyOpenAccount else AuthText.SignIn),
                { onIntent(VerificationIntent.Continue) }, variant = ButtonVariant.AuthPrimary,
                icon = if (signedIn) "arrow_forward" else authActionIcon(AuthIntent.OpenLogin),
                controlModifier = Modifier.fillMaxWidth().testTag("verify-continue"))
            is VerificationPhase.CompleteRefused, VerificationPhase.LinkRefused, VerificationPhase.Idle -> if (!signedIn) {
                Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                    Button(copy.text(AuthText.SignIn), { onIntent(VerificationIntent.Continue) }, variant = ButtonVariant.AuthPrimary,
                        icon = authActionIcon(AuthIntent.OpenLogin), controlModifier = Modifier.fillMaxWidth().testTag("verify-continue"))
                    Button(copy.text(AuthText.CreateAccount), { onIntent(VerificationIntent.RequestAnother) },
                        variant = ButtonVariant.AuthSecondary, icon = authActionIcon(AuthIntent.OpenRegister),
                        controlModifier = Modifier.fillMaxWidth().testTag("verify-request"))
                }
            }
        }
    }
    }
}

internal fun verificationHeading(phase: VerificationPhase): AuthText = when (phase) {
    is VerificationPhase.Completed -> AuthText.VerifyHeadingDone
    VerificationPhase.AwaitingConfirmation, VerificationPhase.Completing, is VerificationPhase.CompleteRetry,
    VerificationPhase.CompleteRefused(CompletionFailure.Invalid) -> AuthText.VerifyHeading
    VerificationPhase.Idle, VerificationPhase.LinkRefused, is VerificationPhase.CompleteRefused -> AuthText.VerifyHeadingUnusable
}

internal fun verificationMessage(phase: VerificationPhase, signedIn: Boolean): AuthText = when (phase) {
    VerificationPhase.Idle, VerificationPhase.LinkRefused -> AuthText.VerifyLinkUnreadable
    VerificationPhase.AwaitingConfirmation -> AuthText.VerifyConfirmHelp
    VerificationPhase.Completing -> AuthText.VerifyCompleting
    is VerificationPhase.CompleteRetry ->
        if (phase.failure is CompletionFailure.RateLimited) AuthText.VerifyRetryRateLimited else AuthText.VerifyRetryHelp
    is VerificationPhase.CompleteRefused -> when (phase.failure) {
        CompletionFailure.Invalid -> AuthText.VerifyInvalidPassword
        CompletionFailure.Forbidden -> AuthText.VerifyForbidden
        CompletionFailure.NotConfigured -> AuthText.VerifyNotConfigured
        CompletionFailure.Refused, is CompletionFailure.RateLimited, CompletionFailure.Unavailable,
        CompletionFailure.UnknownOutcome -> AuthText.VerifyLinkSpent
    }
    is VerificationPhase.Completed -> if (signedIn) AuthText.VerifyCompletedSignedIn else AuthText.VerifyCompletedSignedOut
}

@Composable
private fun PasswordForm(state: EmailVerificationState, copy: AuthCopy, onIntent: (VerificationIntent) -> Unit) {
    // Two independent eyes: revealing one password never reveals the other.
    var passwordVisible by remember { mutableStateOf(false) }
    var repeatVisible by remember { mutableStateOf(false) }
    val pending = state.phase == VerificationPhase.Completing
    val fieldState = if (pending) FieldState.ReadOnly else FieldState.Editable
    // A hint appears once both fields have input, so an untouched form is not shown as an error.
    val issue = state.passwordIssue.takeIf { state.password.isNotEmpty() && state.repeat.isNotEmpty() }
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        TextField(copy.text(AuthText.VerifyPassword), state.password, { onIntent(VerificationIntent.Password(it)) },
            modifier = Modifier.fillMaxWidth(), variant = TextFieldVariant.FloatingTonal, state = fieldState,
            help = copy.text(AuthText.VerifyPasswordHelp),
            visualTransformation = if (passwordVisible) VisualTransformation.None else PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Next),
            trailing = { PasswordEye(passwordVisible, copy, Modifier.testTag("verify-password-eye")) { passwordVisible = !passwordVisible } },
            controlModifier = Modifier.testTag("verify-password"))
        TextField(copy.text(AuthText.VerifyPasswordRepeat), state.repeat, { onIntent(VerificationIntent.Repeat(it)) },
            modifier = Modifier.fillMaxWidth(), variant = TextFieldVariant.FloatingTonal, state = fieldState,
            error = issue?.let { copy.text(it.label) },
            visualTransformation = if (repeatVisible) VisualTransformation.None else PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
            trailing = { PasswordEye(repeatVisible, copy, Modifier.testTag("verify-repeat-eye")) { repeatVisible = !repeatVisible } },
            controlModifier = Modifier.testTag("verify-repeat"))
        Checkbox(copy.text(AuthText.VerifyConfirmCheckbox), state.confirmed, { onIntent(VerificationIntent.Confirm(it)) },
            modifier = Modifier.fillMaxWidth(),
            state = if (pending) ButtonState.Disabled(DisabledReason.of(copy.text(AuthText.VerifyCompleting))) else ButtonState.Enabled,
            controlModifier = Modifier.testTag("verify-confirm"))
        Button(copy.text(AuthText.VerifyCreate), { onIntent(VerificationIntent.Create) },
            variant = ButtonVariant.AuthPrimary, icon = authActionIcon(AuthIntent.OpenRegister),
            state = when {
                pending -> ButtonState.Loading(DisabledReason.of(copy.text(AuthText.VerifyCompleting)))
                state.canConfirm -> ButtonState.Enabled
                else -> ButtonState.Disabled(DisabledReason.of(copy.text(AuthText.VerifyCreateReason)))
            },
            controlModifier = Modifier.fillMaxWidth().testTag("verify-create"))
    }
}

private val PasswordIssue.label: AuthText get() = when (this) {
    PasswordIssue.TooShort -> AuthText.VerifyPasswordShort
    PasswordIssue.TooLong -> AuthText.VerifyPasswordLong
    PasswordIssue.Mismatch -> AuthText.VerifyPasswordMismatch
}
