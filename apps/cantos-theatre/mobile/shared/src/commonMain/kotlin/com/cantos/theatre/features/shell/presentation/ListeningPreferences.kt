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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.KeyboardArrowRight
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
import com.cantos.theatre.app.CopyKey
import com.cantos.theatre.app.DemoCopy
import com.cantos.theatre.demo.DemoCatalogMode
import com.cantos.theatre.demo.DemoEvent
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.demo.DemoTimer
import com.cantos.theatre.demo.SupportedUiSpeedPercents
import com.cantos.ui.tokenDp

internal enum class PreferencePanel(val title: CopyKey) {
    Theme(CopyKey.Theme), Language(CopyKey.Language), Speed(CopyKey.Speed), Timer(CopyKey.Timer),
    Diagnostics(CopyKey.DemoTools),
}

internal fun timerKey(timer: DemoTimer): CopyKey = when (timer) {
    DemoTimer.Off -> CopyKey.TimerOff
    DemoTimer.Fifteen -> CopyKey.Timer15
    DemoTimer.Thirty -> CopyKey.Timer30
    DemoTimer.EndOfEpisode -> CopyKey.TimerEnd
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
            Icon(Icons.AutoMirrored.Outlined.KeyboardArrowRight, null)
        }
    }
}

/** Dialog state is composition-local. Preference values still have their one app-owned authority. */
@Composable
internal fun PreferenceDialog(
    panel: PreferencePanel,
    state: DemoState,
    copy: DemoCopy,
    dispatch: (DemoEvent) -> Unit,
    onDismiss: () -> Unit,
) {
    val selectedFocus = remember { FocusRequester() }
    LaunchedEffect(panel) {
        withFrameNanos { }
        selectedFocus.requestFocus()
    }
    fun choose(event: DemoEvent) {
        dispatch(event)
        if (panel != PreferencePanel.Diagnostics) onDismiss()
    }
    AlertDialog(
        shape = MaterialTheme.shapes.extraLarge,
        modifier = Modifier.testTag("preference-dialog"),
        onDismissRequest = onDismiss,
        title = { Text(copy.text(panel.title)) },
        text = {
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                when (panel) {
                    PreferencePanel.Theme -> DemoTheme.entries.forEach { theme ->
                        PreferenceOption(copy.text(if (theme == DemoTheme.Light) CopyKey.Light else CopyKey.Dark),
                            state.theme == theme, "theme-$theme", selectedFocus) { choose(DemoEvent.SetTheme(theme)) }
                    }
                    PreferencePanel.Language -> DemoLocale.entries.forEach { locale ->
                        PreferenceOption(copy.text(if (locale == DemoLocale.Vietnamese) CopyKey.Vietnamese else CopyKey.English),
                            state.locale == locale, "locale-$locale", selectedFocus) { choose(DemoEvent.SetLocale(locale)) }
                    }
                    PreferencePanel.Speed -> SupportedUiSpeedPercents.forEach { speed ->
                        PreferenceOption(copy.speed(speed), state.speedPercent == speed, "speed-$speed", selectedFocus) {
                            choose(DemoEvent.SetSpeed(speed))
                        }
                    }
                    PreferencePanel.Timer -> {
                        Text(copy.text(CopyKey.TimerSimulationNote), style = MaterialTheme.typography.bodySmall)
                        DemoTimer.entries.forEach { timer ->
                            PreferenceOption(copy.text(timerKey(timer)), state.timer == timer, "timer-$timer", selectedFocus) {
                                choose(DemoEvent.SetTimer(timer))
                            }
                        }
                    }
                    PreferencePanel.Diagnostics -> {
                        Text(copy.text(CopyKey.CatalogStateNote), style = MaterialTheme.typography.bodySmall)
                        DemoCatalogMode.entries.forEach { mode ->
                            val key = when (mode) {
                                DemoCatalogMode.Ready -> CopyKey.Ready
                                DemoCatalogMode.Loading -> CopyKey.LoadingMode
                                DemoCatalogMode.Empty -> CopyKey.EmptyMode
                                DemoCatalogMode.Error -> CopyKey.ErrorMode
                            }
                            PreferenceOption(copy.text(key), state.catalogMode == mode, "catalog-$mode", selectedFocus) {
                                choose(DemoEvent.CatalogMode(mode))
                            }
                        }
                        Text(copy.text(CopyKey.IntegrationsPending), style = MaterialTheme.typography.bodySmall)
                        Text(copy.text(CopyKey.ResetNote), style = MaterialTheme.typography.bodySmall)
                        TextButton(shape = MaterialTheme.shapes.small, onClick = {
                            onDismiss()
                            dispatch(DemoEvent.Reset)
                        }, modifier = Modifier.testTag("reset")) {
                            Text(copy.text(CopyKey.Reset))
                        }
                    }
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
