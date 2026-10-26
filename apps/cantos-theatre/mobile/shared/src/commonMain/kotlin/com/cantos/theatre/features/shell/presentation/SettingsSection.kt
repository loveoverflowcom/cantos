package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
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
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.ui.tokenDp

@Composable
internal fun SettingsScreen(state: ShellState, copy: ShellCopy, dispatch: (ShellEvent) -> Unit) {
    val diagnostics = LocalShellDiagnostics.current
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
        PreferencePanel.entries.filter { it != PreferencePanel.Diagnostics || diagnostics != null }.forEach { choice ->
            val value = when (choice) {
                PreferencePanel.Theme -> copy.text(if (state.theme == ShellTheme.Light) CopyKey.Light else CopyKey.Dark)
                PreferencePanel.Language -> state.locale.autonym
                PreferencePanel.Speed -> copy.speed(state.speedPercent)
                PreferencePanel.Timer -> copy.text(timerKey(state.timer))
                PreferencePanel.Diagnostics -> diagnostics?.let {
                    it.summary
                }.orEmpty()
            }
            val title = choice.title?.let { copy.text(it) } ?: diagnostics?.let {
                it.title
            }.orEmpty()
            PreferenceRow(title, value, { panel = choice },
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
