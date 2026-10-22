package com.cantos.theatre.features.auth.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.domain.identity.RegistrationState
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.ui.AccessibleName
import com.cantos.ui.BackButton
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.CantosTypography
import com.cantos.ui.DisabledReason
import com.cantos.ui.Logo
import com.cantos.ui.LogoTone
import com.cantos.ui.Space
import com.cantos.ui.Surface
import com.cantos.ui.SurfaceTone
import com.cantos.ui.SurfaceVariant
import com.cantos.ui.rememberCantosIconVector
import com.cantos.ui.tokenDp


/** Explicit listener intents. The app root owns every decision; this layer only emits them. */
sealed interface AuthIntent {
    class Login(val email: com.cantos.theatre.domain.identity.RegistrationEmail, val password: String) : AuthIntent {
        override fun toString() = "AuthIntent.Login([redacted])"
    }
    data object CancelLogin : AuthIntent
    data object RestoreSession : AuthIntent
    data object RetryCleanup : AuthIntent
    data object OpenLogin : AuthIntent
    data object OpenRegister : AuthIntent
    data object OpenHome : AuthIntent
    data object OpenLibrary : AuthIntent
    data object OpenMembership : AuthIntent
    data object OpenSettings : AuthIntent
    data object ToggleTheme : AuthIntent
    data object Logout : AuthIntent
    data object Back : AuthIntent
}

/** Where Back leads, derived from the typed stack below the auth routes, never from a URL. */
enum class AuthReturn(internal val label: AuthText) {
    Home(AuthText.ReturnHome),
    Account(AuthText.ReturnAccount),
    Membership(AuthText.ReturnMembership),
    Login(AuthText.ReturnLogin),
}

fun authReturnFor(stack: List<ShellRoute>): AuthReturn {
    if (stack.lastOrNull() == ShellRoute.Register) return AuthReturn.Login
    val origin = stack.getOrNull(stack.indexOfLast { it == ShellRoute.Login } - 1)
    return when (origin) {
        ShellRoute.Account -> AuthReturn.Account
        ShellRoute.MembershipPreview -> AuthReturn.Membership
        else -> AuthReturn.Home
    }
}

@Composable
fun AuthHeader(
    locale: ShellLocale,
    theme: ShellTheme,
    destination: AuthReturn,
    onIntent: (AuthIntent) -> Unit,
    modifier: Modifier = Modifier,
) {
    val copy = AuthCopy(locale)
    Row(
        modifier.fillMaxWidth().padding(horizontal = tokenDp("space.section"), vertical = tokenDp("space.sm")),
        horizontalArrangement = Arrangement.spacedBy(tokenDp("space.inset")),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Back is icon-only so a long destination label can never squeeze the logo; the full
        // localized destination stays in its accessible name.
        BackButton(
            AccessibleName.of(copy.text(destination.label)), { onIntent(AuthIntent.Back) },
            iconOnly = true, controlModifier = Modifier.testTag("back"),
        )
        Logo(
            modifier = Modifier.width(tokenDp("size.authLogo")).heightIn(max = tokenDp("size.backControl")).testTag("header-title"),
            tone = if (theme == ShellTheme.Dark) LogoTone.Dark else LogoTone.Light,
            accessibleName = AccessibleName.of(copy.text(AuthText.Brand)),
        )
        Spacer(Modifier.weight(1f))
        IconButton(onClick = { onIntent(AuthIntent.ToggleTheme) }, modifier = Modifier.size(tokenDp("size.backControl")).testTag("auth-theme")) {
            Icon(
                if (theme == ShellTheme.Dark) rememberCantosIconVector("light_mode") else rememberCantosIconVector("dark_mode"),
                contentDescription = copy.text(AuthText.ToggleTheme),
            )
        }
    }
}

@Composable
private fun AuthHeading(eyebrow: String, heading: String, body: String) {
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
        Text(eyebrow, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.tertiary)
        Text(heading, style = CantosTypography.authHeading, modifier = Modifier.testTag("auth-heading").semantics { heading() })
        Text(body, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Primary state follows the account owner; a disabled or loading action always states why. */
internal fun loginState(account: AccountView, copy: AuthCopy): ButtonState = when (account) {
    AccountView.SignedOut, is AccountView.Failed -> ButtonState.Enabled
    AccountView.Pending, AccountView.SignedIn -> ButtonState.Loading(DisabledReason.of(copy.text(AuthText.PendingReason)))
    AccountView.CleanupRequired -> ButtonState.Disabled(DisabledReason.of(copy.text(AuthText.CleanupReason)))
    AccountView.Unavailable -> ButtonState.Disabled(DisabledReason.of(copy.text(AuthText.UnavailableReason)))
}

/**
 * Create account belongs to the same identity capability as Login, and it never reaches the owner.
 * It is offered only when the listener can act on it: not while an attempt is pending, not when
 * identity is absent (the disabled Login already says why, once) and not when already signed in.
 */
internal fun AccountView.offersRegistration(): Boolean =
    this == AccountView.SignedOut || this is AccountView.Failed || this == AccountView.CleanupRequired

@Composable
internal fun AccountStatus(account: AccountView, copy: AuthCopy, nothingSaved: Boolean = false) {
    val message = when (account) {
        is AccountView.Failed -> copy.failure(account.reason, account.identityFailure)
        AccountView.CleanupRequired -> copy.text(AuthText.CleanupRequired)
        AccountView.Pending -> copy.text(AuthText.PendingStatus)
        AccountView.SignedOut -> if (nothingSaved) copy.text(AuthText.NothingSaved) else null
        AccountView.SignedIn, AccountView.Unavailable -> null
    } ?: return
    Column(Modifier.fillMaxWidth().testTag("auth-status").semantics { liveRegion = LiveRegionMode.Polite }) {
        if (account is AccountView.Failed || account == AccountView.CleanupRequired) ErrorText(message) else SupportingText(message)
    }
}

// App-level text over canonical theme roles: the shared field wrappers are not admitted for this app.
@Composable
private fun SupportingText(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

@Composable
private fun ErrorText(text: String) {
    Text(text, Modifier.semantics { error(text) }, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
}

@Composable
fun LoginScreen(
    locale: ShellLocale,
    account: AccountView,
    nothingSaved: Boolean,
    onIntent: (AuthIntent) -> Unit,
    previewActionIcons: Boolean = false,
) {
    val copy = AuthCopy(locale)
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.lg"))) {
        AuthHeading(copy.text(AuthText.LoginEyebrow), copy.text(AuthText.LoginHeading), copy.text(AuthText.LoginHelp))
        Surface(Modifier.fillMaxWidth().testTag("auth-brand"), tone = SurfaceTone.PrimaryContainer,
            variant = SurfaceVariant.Auth, inset = Space.Lg) {
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Text(copy.text(AuthText.BrandGenres), style = MaterialTheme.typography.labelSmall)
                Text(copy.text(AuthText.BrandHeading), style = CantosTypography.membershipTitle)
                Text(copy.text(AuthText.BrandHelp), style = MaterialTheme.typography.bodyMedium)
            }
        }
        AccountStatus(account, copy, nothingSaved)
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            LoginForm(account, copy, onIntent)
            if (account.offersRegistration()) Button(
                copy.text(AuthText.CreateAccount), { onIntent(AuthIntent.OpenRegister) },
                variant = ButtonVariant.AuthSecondary,
                icon = if (previewActionIcons) authActionIcon(AuthIntent.OpenRegister) else null,
                controlModifier = Modifier.fillMaxWidth().testTag("login-create"),
            )
            when (account) {
                AccountView.Pending -> Button(
                    copy.text(AuthText.CancelLogin), { onIntent(AuthIntent.CancelLogin) },
                    icon = if (previewActionIcons) authActionIcon(AuthIntent.CancelLogin) else null,
                    variant = ButtonVariant.AuthOutlined, controlModifier = Modifier.fillMaxWidth().testTag("auth-cancel"),
                )
                AccountView.CleanupRequired -> Button(
                    copy.text(AuthText.RetryCleanup), { onIntent(AuthIntent.RetryCleanup) },
                    icon = if (previewActionIcons) authActionIcon(AuthIntent.RetryCleanup) else null,
                    variant = ButtonVariant.AuthOutlined, controlModifier = Modifier.fillMaxWidth().testTag("auth-retry-cleanup"),
                )
                AccountView.SignedOut, is AccountView.Failed -> Button(
                    copy.text(AuthText.RestoreSession), { onIntent(AuthIntent.RestoreSession) },
                    icon = if (previewActionIcons) authActionIcon(AuthIntent.RestoreSession) else null,
                    variant = ButtonVariant.Text, controlModifier = Modifier.fillMaxWidth().testTag("auth-restore"),
                )
                AccountView.SignedIn, AccountView.Unavailable -> Unit
            }
            SupportingText(copy.text(AuthText.PasswordHelp))
        }
    }
}

/** Production Register renders the enrollment owner; an absent owner remains explained. */
@Composable
fun RegisterScreen(locale: ShellLocale, onIntent: (AuthIntent) -> Unit, previewActionIcons: Boolean = false,
                   registration: RegistrationState? = null, onRegistration: (RegistrationIntent) -> Unit = {}) {
    val copy = AuthCopy(locale)
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.lg"))) {
        AuthHeading(copy.text(AuthText.RegisterEyebrow), copy.text(AuthText.RegisterHeading), copy.text(AuthText.RegisterHelp))
        if (registration != null) RegistrationForm(registration, locale, onRegistration)
        else Surface(Modifier.fillMaxWidth().testTag("register-unavailable").semantics { liveRegion = LiveRegionMode.Polite },
            tone = SurfaceTone.Container, variant = SurfaceVariant.Auth, inset = Space.Lg) {
            Text(copy.text(AuthText.SignupUnavailable), style = MaterialTheme.typography.bodyMedium)
        }
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Button(
                copy.text(AuthText.ReturnLogin), { onIntent(AuthIntent.OpenLogin) },
                variant = if (registration == null) ButtonVariant.AuthPrimary else ButtonVariant.Text,
                icon = if (previewActionIcons || registration != null) authActionIcon(AuthIntent.OpenLogin) else "arrow_back",
                controlModifier = Modifier.fillMaxWidth().testTag("register-login"),
            )
            Button(
                copy.text(AuthText.RegisterLater), { onIntent(AuthIntent.OpenHome) },
                icon = if (previewActionIcons) authActionIcon(AuthIntent.OpenHome) else null,
                variant = if (registration == null) ButtonVariant.AuthOutlined else ButtonVariant.Text,
                controlModifier = Modifier.fillMaxWidth().testTag("register-home"),
            )
        }
    }
}
/** Form RAM only: no saveable state, shell projection, snapshot or automatic password retry. */
@Composable
private fun LoginForm(account: AccountView, copy: AuthCopy, onIntent: (AuthIntent) -> Unit) {
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var visible by remember { mutableStateOf(false) }
    var invalidEmail by remember { mutableStateOf(false) }
    var invalidPassword by remember { mutableStateOf(false) }
    val passwordFocus = remember { FocusRequester() }
    val focus = LocalFocusManager.current
    DisposableEffect(Unit) { onDispose { password = ""; visible = false } }
    // Submission clears the field before dispatch. The account owner synchronously enters Pending.
    val submit = {
        if (account.canBeginLogin) {
            val checked = RegistrationEmail.parse(email)
            invalidEmail = checked !is RegistrationResult.Accepted
            invalidPassword = password.isEmpty()
            if (checked is RegistrationResult.Accepted && !invalidPassword) {
                val command = AuthIntent.Login(checked.value, password)
                password = ""
                visible = false
                focus.clearFocus()
                onIntent(command)
            }
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        // Existing app-owned Material fields use the theme roles. The shared TextField has no
        // password transformation/visibility contract; extending its two renderers is separate.
        OutlinedTextField(email, { email = it.take(254); invalidEmail = false },
            modifier = Modifier.fillMaxWidth().testTag("login-email"), singleLine = true,
            label = { androidx.compose.material3.Text(copy.text(AuthText.RegisterEmail)) },
            enabled = account.canBeginLogin, isError = invalidEmail,
            supportingText = if (invalidEmail) ({ androidx.compose.material3.Text(copy.text(AuthText.RegisterInvalidEmail)) }) else null,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, imeAction = ImeAction.Next),
            keyboardActions = KeyboardActions(onNext = { passwordFocus.requestFocus() }))
        OutlinedTextField(password, { password = it; invalidPassword = false },
            modifier = Modifier.fillMaxWidth().focusRequester(passwordFocus).testTag("login-password"), singleLine = true,
            label = { androidx.compose.material3.Text(copy.text(AuthText.Password)) },
            enabled = account.canBeginLogin, isError = invalidPassword,
            supportingText = if (invalidPassword) ({ androidx.compose.material3.Text(copy.text(AuthText.PasswordRequired)) }) else null,
            visualTransformation = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { submit() }))
        // A separate control keeps the password label legible at narrow widths and 2x text.
        TextButton({ visible = !visible }, modifier = Modifier.align(Alignment.End), enabled = account.canBeginLogin) {
            androidx.compose.material3.Text(copy.text(if (visible) AuthText.HidePassword else AuthText.ShowPassword))
        }
        Button(copy.text(AuthText.CantosIdLogin), { submit() }, variant = ButtonVariant.AuthPrimary,
            state = loginState(account, copy), icon = "login",
            controlModifier = Modifier.fillMaxWidth().testTag("login-primary"))
    }
}
