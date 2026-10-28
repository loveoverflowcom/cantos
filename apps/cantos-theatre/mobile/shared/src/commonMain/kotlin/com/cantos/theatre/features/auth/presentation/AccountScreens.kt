package com.cantos.theatre.features.auth.presentation

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.ui.Badge
import com.cantos.ui.Button
import com.cantos.ui.ButtonVariant
import com.cantos.ui.CantosTypography
import com.cantos.ui.LocalEditorialFontFamily
import com.cantos.ui.Space
import com.cantos.ui.StatusTone
import com.cantos.ui.Surface
import com.cantos.ui.SurfaceTone
import com.cantos.ui.SurfaceVariant
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp
import com.cantos.ui.rememberCantosIconVector

/**
 * Guest or signed-in Account. It shows only facts the account owner may project: no actor, no
 * session, no display name, no total or conflict. [loginFocus] lets the app root restore focus
 * to the entry that opened Login when Back returns here. [loginAttempt] names the failure that is
 * feedback for an attempt made on Login, which the guest card does not repeat. [backOverlay] is how
 * the open sign-out confirmation sheet hands the host its dismissal, so Back closes the sheet first.
 */
@Composable
fun AccountContent(
    locale: ShellLocale,
    theme: ShellTheme,
    account: AccountView,
    onIntent: (AuthIntent) -> Unit,
    loginFocus: FocusRequester,
    loginAttempt: LoginAttempt = LoginAttempt.None,
    backOverlay: @Composable (onBack: () -> Unit) -> Unit,
) {
    val copy = AuthCopy(locale)
    if (account == AccountView.SignedIn) SignedInAccount(copy, theme, onIntent, backOverlay)
    else GuestAccount(copy, theme, account, loginAttempt, onIntent, loginFocus)
}

@Composable
private fun GuestAccount(
    copy: AuthCopy,
    theme: ShellTheme,
    account: AccountView,
    loginAttempt: LoginAttempt,
    onIntent: (AuthIntent) -> Unit,
    loginFocus: FocusRequester,
) {
    Surface(Modifier.fillMaxWidth().testTag("account-profile"), tone = SurfaceTone.PrimaryContainer,
        variant = SurfaceVariant.Profile, inset = Space.Lg) {
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                AccountAvatar()
                Badge(copy.text(AuthText.GuestBadge), tone = StatusTone.Neutral)
            }
            Text(copy.text(AuthText.GuestHeading),
                style = MaterialTheme.typography.headlineSmall.copy(fontFamily = LocalEditorialFontFamily.current),
                modifier = Modifier.semantics { heading() })
            Text(copy.text(AuthText.GuestHelp), style = MaterialTheme.typography.bodyMedium)
            // A failed attempt's message stays on Login; recovery, pending and other failures stay here.
            if (!loginAttempt.owns(account)) AccountStatus(account, copy)
            Button(
                copy.text(AuthText.SignIn), { onIntent(AuthIntent.OpenLogin) },
                variant = ButtonVariant.AuthPrimary, state = loginState(account, copy),
                icon = authActionIcon(AuthIntent.OpenLogin),
                controlModifier = Modifier.fillMaxWidth().focusRequester(loginFocus).testTag("sign-in"),
            )
            if (account.offersRegistration()) Button(
                copy.text(AuthText.CreateAccountShort), { onIntent(AuthIntent.OpenRegister) },
                variant = ButtonVariant.AuthOutlined,
                icon = authActionIcon(AuthIntent.OpenRegister),
                controlModifier = Modifier.fillMaxWidth().testTag("create-account"),
            )
            if (account == AccountView.CleanupRequired) Button(
                copy.text(AuthText.RetryCleanup), { onIntent(AuthIntent.RetryCleanup) },
                icon = authActionIcon(AuthIntent.RetryCleanup),
                variant = ButtonVariant.AuthOutlined, controlModifier = Modifier.fillMaxWidth().testTag("auth-retry-cleanup"),
            )
        }
    }
    Surface(Modifier.fillMaxWidth().testTag("account-membership"), tone = SurfaceTone.Container,
        variant = SurfaceVariant.Membership, inset = Space.Lg) {
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            Badge(copy.text(AuthText.MembershipBadge), tone = StatusTone.Info)
            Text(copy.text(AuthText.MembershipName), style = MaterialTheme.typography.labelLarge)
            Text(copy.text(AuthText.MembershipHeading), style = CantosTypography.membershipTitle)
            Text(copy.text(AuthText.MembershipHelp), style = MaterialTheme.typography.bodyMedium)
            Button(
                copy.text(AuthText.MembershipLearn), { onIntent(AuthIntent.OpenMembership) },
                variant = ButtonVariant.AuthSecondary, icon = "arrow_forward",
                controlModifier = Modifier.fillMaxWidth().testTag("membership-preview-entry"),
            )
        }
    }
    Surface(Modifier.fillMaxWidth().testTag("account-settings-group"), tone = SurfaceTone.Surface,
        variant = SurfaceVariant.AccountGroup, inset = Space.Sm) {
        Column {
            AppearanceRow(copy, theme, onIntent)
            AccountRow(rememberCantosIconVector("settings"), copy.text(AuthText.ListeningSettings),
                copy.text(AuthText.ListeningSettingsHint), "settings") { onIntent(AuthIntent.OpenSettings) }
        }
    }
}

/**
 * The signed-in Account (design v4.1, page 3): identity first, one grouped list second, the sign-out
 * entry last. The owner projects no name, email or entitlement, so the profile is the honest
 * fallback (page 4) and nothing here invents a person, a plan or a support address.
 */
@Composable
private fun SignedInAccount(copy: AuthCopy, theme: ShellTheme, onIntent: (AuthIntent) -> Unit, backOverlay: @Composable (onBack: () -> Unit) -> Unit) {
    var confirming by remember { mutableStateOf(false) }
    var submitted by remember { mutableStateOf(false) }
    var returnFocus by remember { mutableStateOf(false) }
    val logoutFocus = remember { FocusRequester() }
    // Focus goes back to the entry once the sheet is gone, not while the sheet still holds it.
    LaunchedEffect(confirming) {
        if (confirming) returnFocus = true
        else if (returnFocus) {
            returnFocus = false
            withFrameNanos { }
            runCatching { logoutFocus.requestFocus() }
        }
    }
    Surface(Modifier.fillMaxWidth().testTag("account-profile"), tone = SurfaceTone.PrimaryContainer,
        variant = SurfaceVariant.Profile, inset = Space.Lg) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            AccountAvatar()
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                Text(copy.text(AuthText.SignedProfileName),
                    style = MaterialTheme.typography.titleLarge.copy(fontFamily = LocalEditorialFontFamily.current),
                    modifier = Modifier.semantics { heading() }.testTag("account-name"))
                Text(copy.text(AuthText.SignedBadge), style = MaterialTheme.typography.bodyMedium)
            }
        }
    }
    Surface(Modifier.fillMaxWidth().testTag("account-nav-group"), tone = SurfaceTone.Container,
        variant = SurfaceVariant.AccountGroup, inset = Space.Sm) {
        Column {
            AccountRow(rememberCantosIconVector("star"), copy.text(AuthText.MyMembership),
                copy.text(AuthText.MembershipExplore), "membership-preview-entry") { onIntent(AuthIntent.OpenMembership) }
            AccountRow(rememberCantosIconVector("menu_book"), copy.text(AuthText.LibraryAndHistory), null, "open-library") {
                onIntent(AuthIntent.OpenLibrary)
            }
            AppearanceRow(copy, theme, onIntent)
            AccountRow(rememberCantosIconVector("settings"), copy.text(AuthText.ListeningSettings),
                copy.text(AuthText.ListeningSettingsHint), "settings") { onIntent(AuthIntent.OpenSettings) }
            // No verified support destination exists, so this is information, never a control.
            SupportInfoRow(copy)
        }
    }
    Surface(Modifier.fillMaxWidth().testTag("account-logout-group"), tone = SurfaceTone.Container,
        variant = SurfaceVariant.AccountGroup, inset = Space.Sm) {
        Row(
            Modifier.fillMaxWidth().heightIn(min = tokenDp("size.control")).clip(MaterialTheme.shapes.large)
                .focusRequester(logoutFocus).clickable(role = Role.Button) { confirming = true }
                .padding(tokenDp("space.md")).testTag("logout").semantics(mergeDescendants = true) { },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
        ) {
            Icon(rememberCantosIconVector(authActionIcon(AuthIntent.Logout) ?: "logout"), null, tint = tokenColor("color.accent"))
            Text(copy.text(AuthText.Logout), Modifier.weight(1f), style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary)
        }
    }
    if (confirming) LogoutSheet(
        copy, backOverlay, pending = submitted,
        onConfirm = {
            // One confirmed intent per opening of the sheet; the owner decides what it means.
            if (!submitted) {
                submitted = true
                onIntent(AuthIntent.Logout)
            }
        },
        onDismiss = {
            // Closing never cancels a dispatched request; the owner's state alone says what happened.
            confirming = false
            submitted = false
        },
    )
}

/** Decorative: the heading and status beside it carry the meaning; the lavender container needs the primary pair. */
@Composable
private fun AccountAvatar() {
    Box(
        Modifier.size(tokenDp("size.avatarLarge")).clip(CircleShape)
            .background(tokenColor("color.primary")).testTag("account-avatar"),
        contentAlignment = Alignment.Center,
    ) { Icon(rememberCantosIconVector("person"), null, tint = tokenColor("color.onPrimary")) }
}

@Composable
private fun AppearanceRow(copy: AuthCopy, theme: ShellTheme, onIntent: (AuthIntent) -> Unit) {
    AccountRow(
        if (theme == ShellTheme.Dark) rememberCantosIconVector("light_mode") else rememberCantosIconVector("dark_mode"),
        copy.text(AuthText.Appearance),
        copy.text(if (theme == ShellTheme.Dark) AuthText.AppearanceDark else AuthText.AppearanceLight),
        "theme-toggle",
    ) { onIntent(AuthIntent.ToggleTheme) }
}

@Composable
private fun SupportInfoRow(copy: AuthCopy) {
    Column(
        // Aligned under the row titles: the icon and its gap, then the same text column.
        Modifier.fillMaxWidth().heightIn(min = tokenDp("size.control"))
            .padding(start = tokenDp("space.md") + tokenDp("size.icon") + tokenDp("space.md"), top = tokenDp("space.md"),
                end = tokenDp("space.md"), bottom = tokenDp("space.md"))
            .semantics(mergeDescendants = true) { }.testTag("account-support-info"),
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs")),
    ) {
        Text(copy.text(AuthText.SupportTitle), style = MaterialTheme.typography.labelLarge)
        Text(copy.text(AuthText.SupportPending), style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun AccountRow(icon: ImageVector, title: String, value: String?, tag: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = tokenDp("size.control")).clip(MaterialTheme.shapes.large)
            .clickable(role = Role.Button, onClick = onClick).padding(tokenDp("space.md")).testTag(tag)
            .semantics(mergeDescendants = true) { if (value != null) stateDescription = value },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
    ) {
        Icon(icon, null, tint = MaterialTheme.colorScheme.primary)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
            Text(title, style = MaterialTheme.typography.labelLarge)
            if (value != null) Text(value, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Icon(rememberCantosIconVector("keyboard_arrow_right"), null)
    }
}
