package com.cantos.theatre.app

import com.cantos.theatre.features.shell.presentation.*
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import com.cantos.theatre.demo.DemoWorkIds
import com.cantos.theatre.features.membership.presentation.LocalMembershipIntroArtwork
import com.cantos.theatre.features.membership.presentation.LocalMembershipDiagnostics
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.ui.tokenDp

/** The sandbox owns sample wording, notices, fixture artwork selection and diagnostic controls. */
@Composable
fun WithDemoShellPresentation(content: @Composable () -> Unit) {
    CompositionLocalProvider(
        LocalShellCopyOverrides provides DemoShellCopyOverrides,
        LocalShellNotice provides { state, _ ->
            Text(
                DemoShellText.Notice.localized(state.locale),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.testTag("demo-notice"),
            )
        },
        LocalShellDiagnostics provides demoDiagnostics,
        LocalMembershipIntroArtwork provides { modifier ->
            LocalWorkArtwork.current?.invoke(
                DemoWorkIds.SonHa, modifier, RoundedCornerShape(tokenDp("shape.membershipContainer")),
            )
        },
        LocalMembershipDiagnostics provides { state, locale, dispatch, plansLabel ->
            MembershipFixtureControls(state, locale, dispatch, plansLabel)
        },
        content = content,
    )
}

private val demoDiagnostics = ShellDiagnosticsSlot(
    title = DemoShellText.DiagnosticsTitle.resource,
    summary = DemoShellText.DiagnosticsSummary.resource,
    content = { state, dispatch, onDismiss -> DemoDiagnostics(state, dispatch, onDismiss) },
)

@Composable
private fun DemoDiagnostics(state: ShellState, dispatch: (ShellEvent) -> Unit, onDismiss: () -> Unit) {
    val selectedFocus = remember { FocusRequester() }
    LaunchedEffect(Unit) {
        withFrameNanos { }
        selectedFocus.requestFocus()
    }
    Text(DemoShellText.CatalogStateNote.localized(state.locale), style = MaterialTheme.typography.bodySmall)
    ShellCatalogMode.entries.forEach { mode ->
        val selected = state.catalogMode == mode
        val label = when (mode) {
            ShellCatalogMode.Ready -> DemoShellText.Ready
            ShellCatalogMode.Loading -> DemoShellText.Loading
            ShellCatalogMode.Empty -> DemoShellText.Empty
            ShellCatalogMode.Error -> DemoShellText.Error
        }
        Row(
            Modifier.fillMaxWidth().heightIn(min = tokenDp("size.control")).testTag("catalog-$mode")
                .then(if (selected) Modifier.focusRequester(selectedFocus) else Modifier)
                .selectable(selected = selected, role = Role.RadioButton, onClick = { dispatch(ShellEvent.CatalogMode(mode)) })
                .padding(vertical = tokenDp("space.sm")),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
        ) {
            RadioButton(selected = selected, onClick = null)
            Text(label.localized(state.locale), modifier = Modifier.weight(1f))
        }
    }
    Text(DemoShellText.IntegrationsPending.localized(state.locale), style = MaterialTheme.typography.bodySmall)
    Text(DemoShellText.ResetNote.localized(state.locale), style = MaterialTheme.typography.bodySmall)
    TextButton(
        shape = MaterialTheme.shapes.small,
        onClick = { onDismiss(); dispatch(ShellEvent.Reset) },
        modifier = Modifier.testTag("reset"),
    ) {
        Text(DemoShellText.Reset.localized(state.locale))
    }
}
