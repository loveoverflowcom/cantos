package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellThemeMode
import com.cantos.ui.Space
import com.cantos.ui.Surface
import com.cantos.ui.SurfaceTone
import com.cantos.ui.SurfaceVariant
import com.cantos.ui.tokenDp

/** Local chooser/focus state only. Values and effects stay with the root PreferencesController. */
@Composable
internal fun MePreferences(
    state: ShellState,
    copy: ShellCopy,
    dispatch: (ShellEvent) -> Unit,
    themeMode: ShellThemeMode,
) {
    var panel by remember { mutableStateOf<PreferencePanel?>(null) }
    var restoreFocus by remember { mutableStateOf<PreferencePanel?>(null) }
    val requesters = remember { listOf(PreferencePanel.Theme, PreferencePanel.Language).associateWith { FocusRequester() } }
    LaunchedEffect(panel, restoreFocus) {
        if (panel == null) restoreFocus?.let { choice ->
            withFrameNanos { }
            requesters.getValue(choice).requestFocus()
            restoreFocus = null
        }
    }
    Surface(Modifier.fillMaxWidth().testTag("account-preferences"), tone = SurfaceTone.Surface,
        variant = SurfaceVariant.AccountGroup, inset = Space.Md) {
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Text(copy.text(CopyKey.PreferencesHeading), style = MaterialTheme.typography.titleLarge,
                modifier = Modifier.semantics { heading() })
            for (choice in listOf(PreferencePanel.Theme, PreferencePanel.Language)) {
                val icon = when (choice) {
                    PreferencePanel.Theme -> when (themeMode) {
                        ShellThemeMode.Light -> "light_mode"
                        ShellThemeMode.Dark -> "dark_mode"
                        ShellThemeMode.System -> "theme_system"
                    }
                    else -> "language"
                }
                PreferenceRow(
                    title = copy.text(if (choice == PreferencePanel.Theme) CopyKey.Theme else CopyKey.PreferenceLanguage),
                    value = if (choice == PreferencePanel.Theme) copy.text(when (themeMode) {
                        ShellThemeMode.Light -> CopyKey.PreferenceLight
                        ShellThemeMode.Dark -> CopyKey.PreferenceDark
                        ShellThemeMode.System -> CopyKey.System
                    }) else state.locale.autonym,
                    onClick = { panel = choice },
                    modifier = Modifier.testTag("setting-$choice").focusRequester(requesters.getValue(choice)),
                    leadingIcon = icon,
                    compact = true,
                )
            }
        }
    }
    panel?.let { current ->
        PreferenceDialog(current, state, copy, dispatch, themeMode) { panel = null; restoreFocus = current }
    }
}
