package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.KeyboardArrowRight
import androidx.compose.material.icons.outlined.Person
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import com.cantos.theatre.demo.CopyKey
import com.cantos.theatre.demo.DemoCopy
import com.cantos.theatre.demo.DemoEvent
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.ui.LocalEditorialFontFamily
import com.cantos.ui.Theme
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp

@Composable
internal fun AccountScreen(copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    Row(
        Modifier.fillMaxWidth().testTag("account-profile"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
    ) {
        // Decorative: the guest name beside it carries the meaning.
        Box(
            Modifier.size(tokenDp("size.avatarLarge")).clip(CircleShape)
                .background(tokenColor("color.avatarLavender")).testTag("account-avatar"),
            contentAlignment = Alignment.Center,
        ) {
            Icon(Icons.Outlined.Person, null, tint = tokenColor("color.onAvatarLavender"))
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
            Text(
                copy.text(CopyKey.AccountGuest),
                style = MaterialTheme.typography.titleLarge.copy(fontFamily = LocalEditorialFontFamily.current),
            )
            Text(
                copy.text(CopyKey.PersonalRhythm),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
    Surface(
        modifier = Modifier.fillMaxWidth().testTag("account-preview"),
        color = MaterialTheme.colorScheme.primaryContainer,
        contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
        shape = MaterialTheme.shapes.large,
    ) {
        Column(Modifier.padding(tokenDp("space.md")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Text(copy.text(CopyKey.AccountPreviewTitle), style = MaterialTheme.typography.labelLarge)
            Text(copy.text(CopyKey.AccountHelp), style = MaterialTheme.typography.bodyMedium)
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Button(shape = MaterialTheme.shapes.medium, onClick = {}, enabled = false, modifier = Modifier.fillMaxWidth().testTag("sign-in").semantics {
            stateDescription = copy.text(CopyKey.SignInUnavailable)
        }) { Text(copy.text(CopyKey.SignIn)) }
        Text(
            copy.text(CopyKey.SignInUnavailable),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
    TextButton(onClick = { dispatch(DemoEvent.Navigate(DemoRoute.MembershipPreview)) },
        modifier = Modifier.fillMaxWidth().testTag("membership-preview-entry")) {
        Text(copy.text(CopyKey.MembershipPreview))
    }
    Surface(
        onClick = { dispatch(DemoEvent.Navigate(DemoRoute.Settings)) },
        modifier = Modifier.fillMaxWidth().testTag("settings").semantics {
            role = Role.Button
        },
        color = MaterialTheme.colorScheme.surfaceContainer,
        shape = MaterialTheme.shapes.large,
    ) {
        Row(
            Modifier.padding(tokenDp("space.md")),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
        ) {
            Icon(Icons.Outlined.Settings, null, tint = MaterialTheme.colorScheme.primary)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                Text(copy.text(CopyKey.Settings), style = MaterialTheme.typography.labelLarge)
                Text(
                    copy.text(CopyKey.AccountSettingsHint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Icon(Icons.AutoMirrored.Outlined.KeyboardArrowRight, null)
        }
    }
}

@Composable
internal fun SettingsScreen(state: DemoState, copy: DemoCopy, dispatch: (DemoEvent) -> Unit) {
    var panel by remember { mutableStateOf<PreferencePanel?>(null) }
    var restoreFocus by remember { mutableStateOf<PreferencePanel?>(null) }
    val requesters = remember { PreferencePanel.entries.associateWith { FocusRequester() } }
    LaunchedEffect(panel, restoreFocus) {
        if (panel == null) restoreFocus?.let {
            withFrameNanos { }
            requesters.getValue(it).requestFocus()
            restoreFocus = null
        }
    }
    Text(copy.text(CopyKey.PersonalRhythm), style = MaterialTheme.typography.headlineSmall,
        modifier = Modifier.semantics { heading() })
    Column {
        PreferencePanel.entries.forEach { choice ->
            val value = when (choice) {
                PreferencePanel.Theme -> copy.text(if (state.theme == DemoTheme.Light) CopyKey.Light else CopyKey.Dark)
                PreferencePanel.Language -> copy.text(if (state.locale == DemoLocale.Vietnamese) CopyKey.Vietnamese else CopyKey.English)
                PreferencePanel.Speed -> copy.speed(state.speedPercent)
                PreferencePanel.Timer -> copy.text(timerKey(state.timer))
                PreferencePanel.Diagnostics -> copy.text(CopyKey.CatalogState)
            }
            PreferenceRow(copy.text(choice.title), value, { panel = choice },
                Modifier.testTag("setting-$choice").focusRequester(requesters.getValue(choice)))
            if (choice == PreferencePanel.Timer) Text(copy.text(CopyKey.PreferencesNote),
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(vertical = tokenDp("space.md")))
        }
    }
    panel?.let { current ->
        PreferenceDialog(current, state, copy, dispatch) { panel = null; restoreFocus = current }
    }
}
