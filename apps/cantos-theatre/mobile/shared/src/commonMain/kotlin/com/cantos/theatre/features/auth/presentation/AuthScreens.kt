package com.cantos.theatre.features.auth.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import com.cantos.theatre.application.identity.AccountView
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
import com.cantos.ui.tokenDp
import androidx.compose.ui.unit.Dp
import androidx.compose.material.icons.outlined.DarkMode
import androidx.compose.material.icons.outlined.LightMode
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton

/** Explicit listener intents. The app root owns every decision; this layer only emits them. */
sealed interface AuthIntent {
    data object BeginLogin : AuthIntent
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
    AuthHeaderLayout(
        modifier.fillMaxWidth().padding(horizontal = tokenDp("space.section"), vertical = tokenDp("space.sm")),
        gap = tokenDp("space.inset"),
        minLogo = tokenDp("size.authLogo") / 2,
    ) {
        BackButton(
            AccessibleName.of(copy.text(destination.label)), { onIntent(AuthIntent.Back) },
            iconOnly = false, controlModifier = Modifier.testTag("back"),
        )
        Logo(
            modifier = Modifier.width(tokenDp("size.authLogo")).heightIn(max = tokenDp("size.backControl")).testTag("header-title"),
            tone = if (theme == ShellTheme.Dark) LogoTone.Dark else LogoTone.Light,
            accessibleName = AccessibleName.of(copy.text(AuthText.Brand)),
        )
        IconButton(onClick = { onIntent(AuthIntent.ToggleTheme) }, modifier = Modifier.size(tokenDp("size.backControl")).testTag("auth-theme")) {
            Icon(
                if (theme == ShellTheme.Dark) Icons.Outlined.LightMode else Icons.Outlined.DarkMode,
                contentDescription = copy.text(AuthText.ToggleTheme),
            )
        }
    }
}

/**
 * Children are Back, the brand logo and the theme toggle, in priority order: the toggle keeps its
 * size, Back takes what remains and wraps its label, and the decorative logo appears only in the
 * space left over, so large text can never squeeze a control away.
 */
@Composable
private fun AuthHeaderLayout(modifier: Modifier, gap: Dp, minLogo: Dp, content: @Composable () -> Unit) {
    Layout(content, modifier) { measurables, constraints ->
        val gapPx = gap.roundToPx()
        val loose = constraints.copy(minWidth = 0, minHeight = 0)
        val theme = measurables[2].measure(loose)
        val back = measurables[0].measure(loose.copy(maxWidth = (constraints.maxWidth - theme.width - gapPx).coerceAtLeast(0)))
        val room = constraints.maxWidth - back.width - theme.width - 2 * gapPx
        val logo = if (room >= minLogo.roundToPx()) measurables[1].measure(loose.copy(maxWidth = room)) else null
        val height = maxOf(back.height, theme.height, logo?.height ?: 0)
        layout(constraints.maxWidth, height) {
            back.placeRelative(0, (height - back.height) / 2)
            theme.placeRelative(constraints.maxWidth - theme.width, (height - theme.height) / 2)
            logo?.placeRelative(constraints.maxWidth - theme.width - gapPx - logo.width, (height - logo.height) / 2)
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
        is AccountView.Failed -> copy.failure(account.reason)
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
            Button(
                copy.text(AuthText.CantosIdLogin), { onIntent(AuthIntent.BeginLogin) },
                variant = ButtonVariant.AuthPrimary, state = loginState(account, copy),
                icon = if (previewActionIcons) authActionIcon(AuthIntent.BeginLogin) else "arrow_forward",
                controlModifier = Modifier.fillMaxWidth().testTag("login-primary"),
            )
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
            SupportingText(copy.text(AuthText.ProviderHelp))
        }
    }
}

/** Honest registration: the provider has no verified sign-up entry here, so no form and no success. */
@Composable
fun RegisterScreen(locale: ShellLocale, onIntent: (AuthIntent) -> Unit, previewActionIcons: Boolean = false) {
    val copy = AuthCopy(locale)
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.lg"))) {
        AuthHeading(copy.text(AuthText.RegisterEyebrow), copy.text(AuthText.RegisterHeading), copy.text(AuthText.RegisterHelp))
        Surface(Modifier.fillMaxWidth().testTag("register-unavailable").semantics { liveRegion = LiveRegionMode.Polite },
            tone = SurfaceTone.Container, variant = SurfaceVariant.Auth, inset = Space.Lg) {
            Text(copy.text(AuthText.SignupUnavailable), style = MaterialTheme.typography.bodyMedium)
        }
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Button(
                copy.text(AuthText.ReturnLogin), { onIntent(AuthIntent.OpenLogin) },
                variant = ButtonVariant.AuthPrimary,
                icon = if (previewActionIcons) authActionIcon(AuthIntent.OpenLogin) else "arrow_back",
                controlModifier = Modifier.fillMaxWidth().testTag("register-login"),
            )
            Button(
                copy.text(AuthText.RegisterLater), { onIntent(AuthIntent.OpenHome) },
                icon = if (previewActionIcons) authActionIcon(AuthIntent.OpenHome) else null,
                variant = ButtonVariant.AuthOutlined, controlModifier = Modifier.fillMaxWidth().testTag("register-home"),
            )
        }
    }
}
