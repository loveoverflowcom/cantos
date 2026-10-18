package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.SupportedUiSpeedPercents
import com.cantos.ui.tokenDp
import com.cantos.ui.rememberCantosIconVector

internal enum class PreferencePanel(val title: CopyKey?) {
    Theme(CopyKey.Theme), Language(CopyKey.Language), Speed(CopyKey.Speed), Timer(CopyKey.Timer),
    Diagnostics(null),
}

internal fun timerKey(timer: ShellTimer): CopyKey = when (timer) {
    ShellTimer.Off -> CopyKey.TimerOff
    ShellTimer.Fifteen -> CopyKey.Timer15
    ShellTimer.Thirty -> CopyKey.Timer30
    ShellTimer.EndOfEpisode -> CopyKey.TimerEnd
}

@Composable
internal fun PreferenceRow(title: String, value: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    Surface(onClick = onClick, modifier = modifier.fillMaxWidth().semantics { role = Role.Button },
        color = MaterialTheme.colorScheme.background) {
        Row(Modifier.padding(vertical = tokenDp("space.inset")), verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                Text(title, style = MaterialTheme.typography.labelLarge)
                Text(value, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Icon(rememberCantosIconVector("keyboard_arrow_right"), null)
        }
    }
}

/** Dialog state is composition-local. Preference values still have their one app-owned authority. */
@Composable
internal fun PreferenceDialog(
    panel: PreferencePanel,
    state: ShellState,
    copy: ShellCopy,
    dispatch: (ShellEvent) -> Unit,
    onDismiss: () -> Unit,
) {
    val diagnostics = LocalShellDiagnostics.current
    val selectedFocus = remember { FocusRequester() }
    LaunchedEffect(panel) {
        withFrameNanos { }
        if (panel != PreferencePanel.Diagnostics) selectedFocus.requestFocus()
    }
    fun choose(event: ShellEvent) {
        dispatch(event)
        if (panel != PreferencePanel.Diagnostics) onDismiss()
    }
    AlertDialog(
        shape = MaterialTheme.shapes.extraLarge,
        modifier = Modifier.testTag("preference-dialog"),
        onDismissRequest = onDismiss,
        title = {
            val title = panel.title
            if (title != null) Text(copy.text(title))
            else diagnostics?.let { Text(if (state.locale == ShellLocale.Vietnamese) it.title.vi else it.title.en) }
        },
        text = {
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                when (panel) {
                    PreferencePanel.Theme -> ShellTheme.entries.forEach { theme ->
                        PreferenceOption(copy.text(if (theme == ShellTheme.Light) CopyKey.Light else CopyKey.Dark),
                            state.theme == theme, "theme-$theme", selectedFocus) { choose(ShellEvent.SetTheme(theme)) }
                    }
                    PreferencePanel.Language -> ShellLocale.entries.forEach { locale ->
                        PreferenceOption(copy.text(if (locale == ShellLocale.Vietnamese) CopyKey.Vietnamese else CopyKey.English),
                            state.locale == locale, "locale-$locale", selectedFocus) { choose(ShellEvent.SetLocale(locale)) }
                    }
                    PreferencePanel.Speed -> SupportedUiSpeedPercents.forEach { speed ->
                        PreferenceOption(copy.speed(speed), state.speedPercent == speed, "speed-$speed", selectedFocus) {
                            choose(ShellEvent.SetSpeed(speed))
                        }
                    }
                    PreferencePanel.Timer -> {
                        Text(copy.text(CopyKey.TimerSimulationNote), style = MaterialTheme.typography.bodySmall)
                        ShellTimer.entries.forEach { timer ->
                            PreferenceOption(copy.text(timerKey(timer)), state.timer == timer, "timer-$timer", selectedFocus) {
                                choose(ShellEvent.SetTimer(timer))
                            }
                        }
                    }
                    PreferencePanel.Diagnostics -> diagnostics?.content?.invoke(state, dispatch, onDismiss)
                }
            }
        },
        confirmButton = {
            TextButton(shape = MaterialTheme.shapes.small, onClick = onDismiss, modifier = Modifier.testTag("preference-close")) { Text(copy.text(CopyKey.Done)) }
        },
    )
}

@Composable
private fun PreferenceOption(
    label: String,
    selected: Boolean,
    tag: String,
    selectedFocus: FocusRequester,
    onClick: () -> Unit,
) {
    Row(Modifier.fillMaxWidth().heightIn(min = tokenDp("size.control")).testTag(tag)
        .then(if (selected) Modifier.focusRequester(selectedFocus) else Modifier)
        .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
        .padding(vertical = tokenDp("space.sm")),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        RadioButton(selected = selected, onClick = null)
        Text(label, modifier = Modifier.weight(1f))
    }
}
