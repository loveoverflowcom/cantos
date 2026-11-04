package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellThemeMode
import com.cantos.ui.rememberCantosIconVector
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp

/** Local chooser/focus state only. Values and effects stay with the root PreferencesController. */
@Composable
internal fun MePreferences(
    state: ShellState,
    copy: ShellCopy,
    dispatch: (ShellEvent) -> Unit,
    themeMode: ShellThemeMode,
    compactActions: (@Composable (onTheme: () -> Unit, onLanguage: () -> Unit, themeFocus: FocusRequester, languageFocus: FocusRequester) -> Unit)? = null,
    backOverlay: @Composable (() -> Unit) -> Unit = {},
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
    if (compactActions != null) compactActions(
        { panel = PreferencePanel.Theme }, { panel = PreferencePanel.Language },
        requesters.getValue(PreferencePanel.Theme), requesters.getValue(PreferencePanel.Language),
    ) else {
        FlowRow(Modifier.fillMaxWidth().testTag("account-preferences"),
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            for (choice in listOf(PreferencePanel.Theme, PreferencePanel.Language)) {
                val icon = when (choice) {
                    PreferencePanel.Theme -> when (themeMode) {
                        ShellThemeMode.Light -> "light_mode"
                        ShellThemeMode.Dark -> "dark_mode"
                        ShellThemeMode.System -> "theme_system"
                    }
                    else -> null
                }
                val name = copy.text(if (choice == PreferencePanel.Theme) CopyKey.Theme else CopyKey.PreferenceLanguage)
                val value = if (choice == PreferencePanel.Theme) copy.text(when (themeMode) {
                        ShellThemeMode.Light -> CopyKey.PreferenceLight
                        ShellThemeMode.Dark -> CopyKey.PreferenceDark
                        ShellThemeMode.System -> CopyKey.System
                    }) else state.locale.autonym
                val visibleValue = if (choice == PreferencePanel.Theme) value else
                    state.locale.tag.substringBefore('-').replaceFirstChar { it.uppercase() }
                Surface(onClick = { panel = choice },
                    modifier = Modifier.testTag("setting-$choice").focusRequester(requesters.getValue(choice))
                        .semantics { role = Role.Button; contentDescription = name; stateDescription = value },
                    shape = MaterialTheme.shapes.large, color = tokenColor("color.surfaceContainerHigh")) {
                    Row(Modifier.sizeIn(minWidth = tokenDp("size.control"), minHeight = tokenDp("size.control"))
                        .padding(horizontal = tokenDp("space.sm"), vertical = tokenDp("space.xs")),
                        horizontalArrangement = Arrangement.spacedBy(tokenDp("space.xs")),
                        verticalAlignment = Alignment.CenterVertically) {
                        if (icon != null) Icon(rememberCantosIconVector(icon), null)
                        Text(visibleValue, style = MaterialTheme.typography.labelLarge)
                        Icon(rememberCantosIconVector("expand_more"), null)
                    }
                }
            }
        }
    }
    panel?.let { current ->
        val dismiss = { panel = null; restoreFocus = current }
        backOverlay(dismiss)
        PreferenceDialog(current, state, copy, dispatch, themeMode, dismiss)
    }
}
