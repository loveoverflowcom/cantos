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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.outlined.Logout
import androidx.compose.material.icons.outlined.DarkMode
import androidx.compose.material.icons.outlined.LightMode
import androidx.compose.material.icons.outlined.Person
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoTheme
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

/**
 * Guest or signed-in Account. It shows only facts the account owner may project: no actor, no
 * session, no display name, no total or conflict. [loginFocus] lets the app root restore focus
 * to the entry that opened Login when Back returns here.
 */
@Composable
internal fun AccountContent(
    locale: DemoLocale,
    theme: DemoTheme,
    account: AccountView,
    onIntent: (AuthIntent) -> Unit,
    loginFocus: FocusRequester,
) {
    val copy = AuthCopy(locale)
    val signedIn = account == AccountView.SignedIn
    Surface(Modifier.fillMaxWidth().testTag("account-profile"), tone = SurfaceTone.PrimaryContainer,
        variant = SurfaceVariant.Profile, inset = Space.Lg) {
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                // Decorative: the heading and badge beside it carry the meaning. The profile surface is
                // the lavender container, so the generic person mark uses the primary pair to stay visible.
                Box(
                    Modifier.size(tokenDp("size.avatarLarge")).clip(CircleShape)
                        .background(tokenColor("color.primary")).testTag("account-avatar"),
                    contentAlignment = Alignment.Center,
                ) { Icon(Icons.Outlined.Person, null, tint = tokenColor("color.onPrimary")) }
                Badge(copy.text(if (signedIn) AuthText.SignedBadge else AuthText.GuestBadge),
                    tone = if (signedIn) StatusTone.Success else StatusTone.Neutral)
            }
            Text(copy.text(if (signedIn) AuthText.SignedHeading else AuthText.GuestHeading),
                style = MaterialTheme.typography.headlineSmall.copy(fontFamily = LocalEditorialFontFamily.current),
                modifier = Modifier.semantics { heading() })
            Text(copy.text(if (signedIn) AuthText.SignedHelp else AuthText.GuestHelp), style = MaterialTheme.typography.bodyMedium)
            AccountStatus(account, copy)
            if (signedIn) {
                Button(
                    copy.text(AuthText.OpenLibrary), { onIntent(AuthIntent.OpenLibrary) },
                    variant = ButtonVariant.AuthPrimary, icon = "arrow_forward",
                    controlModifier = Modifier.fillMaxWidth().testTag("open-library"),
                )
                Text(copy.text(AuthText.LibraryNote), style = MaterialTheme.typography.bodySmall)
            } else {
                Button(
                    copy.text(AuthText.SignIn), { onIntent(AuthIntent.OpenLogin) },
                    variant = ButtonVariant.AuthPrimary, state = loginState(account, copy), icon = "arrow_forward",
                    controlModifier = Modifier.fillMaxWidth().focusRequester(loginFocus).testTag("sign-in"),
                )
                if (account.offersRegistration()) Button(
                    copy.text(AuthText.CreateAccountShort), { onIntent(AuthIntent.OpenRegister) },
                    variant = ButtonVariant.AuthOutlined,
                    controlModifier = Modifier.fillMaxWidth().testTag("create-account"),
                )
                if (account == AccountView.CleanupRequired) Button(
                    copy.text(AuthText.RetryCleanup), { onIntent(AuthIntent.RetryCleanup) },
                    variant = ButtonVariant.AuthOutlined, controlModifier = Modifier.fillMaxWidth().testTag("auth-retry-cleanup"),
                )
            }
        }
    }
    Surface(Modifier.fillMaxWidth().testTag("account-membership"), tone = SurfaceTone.Container,
        variant = SurfaceVariant.Membership, inset = Space.Lg) {
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            Badge(copy.text(AuthText.MembershipBadge), tone = StatusTone.Info)
            Text(copy.text(AuthText.MembershipName), style = MaterialTheme.typography.labelLarge)
            Text(copy.text(AuthText.MembershipHeading), style = CantosTypography.membershipTitle)
            Text(copy.text(if (signedIn) AuthText.MembershipSignedHelp else AuthText.MembershipHelp),
                style = MaterialTheme.typography.bodyMedium)
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
            AccountRow(
                if (theme == DemoTheme.Dark) Icons.Outlined.LightMode else Icons.Outlined.DarkMode,
                copy.text(AuthText.Appearance),
                copy.text(if (theme == DemoTheme.Dark) AuthText.AppearanceDark else AuthText.AppearanceLight),
                "theme-toggle",
            ) { onIntent(AuthIntent.ToggleTheme) }
            AccountRow(Icons.Outlined.Settings, copy.text(AuthText.ListeningSettings),
                copy.text(AuthText.ListeningSettingsHint), "settings") { onIntent(AuthIntent.OpenSettings) }
            if (signedIn) AccountRow(Icons.AutoMirrored.Outlined.Logout, copy.text(AuthText.Logout),
                copy.text(AuthText.LogoutHint), "logout") { onIntent(AuthIntent.Logout) }
        }
    }
}

@Composable
private fun AccountRow(icon: ImageVector, title: String, value: String, tag: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = tokenDp("size.control")).clip(MaterialTheme.shapes.large)
            .clickable(role = Role.Button, onClick = onClick).padding(tokenDp("space.md")).testTag(tag)
            .semantics(mergeDescendants = true) { stateDescription = value },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
    ) {
        Icon(icon, null, tint = MaterialTheme.colorScheme.primary)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
            Text(title, style = MaterialTheme.typography.labelLarge)
            Text(value, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Icon(Icons.AutoMirrored.Outlined.KeyboardArrowRight, null)
    }
}
