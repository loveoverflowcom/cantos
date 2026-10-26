package com.cantos.theatre.features.auth.presentation

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.DisabledReason
import com.cantos.ui.LocalEditorialFontFamily
import com.cantos.ui.tokenDp
import kotlinx.coroutines.launch

/** The sheet follows the Account frame's width so a tablet keeps a compact, centered confirmation. */
private val SheetMaxWidth = 430.dp
private const val STACK_ABOVE_FONT_SCALE = 1.3f
private val STACK_BELOW_WIDTH = 300.dp

/**
 * Modal confirmation for signing out (design v4.1, page 3). Opening, Go back, system Back, the scrim
 * and a drag all close it without an intent; only [onConfirm] reaches the auth owner, and [pending]
 * keeps that action from being sent twice. [backOverlay] hands Back to the sheet so it closes before
 * the Account route. The caller owns whether the sheet exists and what dismissal resets.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun LogoutSheet(
    copy: AuthCopy,
    backOverlay: @Composable (onBack: () -> Unit) -> Unit,
    pending: Boolean,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = rememberCoroutineScope()
    // Animate out first; leaving composition mid-hide cancels the job and must not dismiss.
    val close: () -> Unit = {
        scope.launch { sheetState.hide() }.invokeOnCompletion { if (!sheetState.isVisible) onDismiss() }
    }
    backOverlay(close)
    val title = copy.text(AuthText.LogoutTitle)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        modifier = Modifier.testTag("logout-sheet"),
        sheetState = sheetState,
        sheetMaxWidth = SheetMaxWidth,
        shape = RoundedCornerShape(topStart = tokenDp("shape.accountGroup"), topEnd = tokenDp("shape.accountGroup")),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        contentColor = MaterialTheme.colorScheme.onSurface,
        scrimColor = MaterialTheme.colorScheme.scrim.copy(alpha = 0.6f),
        // The default handle carries an English description; dragging is a convenience, never the only way out.
        dragHandle = { SheetHandle() },
    ) {
        Column(
            Modifier.fillMaxWidth().verticalScroll(rememberScrollState())
                .padding(start = tokenDp("space.lg"), end = tokenDp("space.lg"), bottom = tokenDp("space.lg"))
                .semantics { paneTitle = title },
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.md")),
        ) {
            Text(title, Modifier.semantics { heading() }.testTag("logout-sheet-title"),
                style = MaterialTheme.typography.headlineSmall.copy(fontFamily = LocalEditorialFontFamily.current))
            Text(copy.text(AuthText.LogoutBody), Modifier.testTag("logout-sheet-body"),
                style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
            SheetActions(copy, pending, onConfirm, close)
        }
    }
}

@Composable
private fun SheetActions(copy: AuthCopy, pending: Boolean, onConfirm: () -> Unit, onCancel: () -> Unit) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        // Larger text or a very narrow screen stacks full-width actions instead of clipping a label.
        val stacked = LocalDensity.current.fontScale > STACK_ABOVE_FONT_SCALE || maxWidth < STACK_BELOW_WIDTH
        val cancel: @Composable (Modifier) -> Unit = { modifier ->
            Button(
                copy.text(AuthText.LogoutCancel), onCancel, modifier,
                variant = ButtonVariant.AuthSecondary,
                controlModifier = Modifier.fillMaxWidth().heightIn(min = SheetActionHeight).testTag("logout-cancel"),
            )
        }
        val confirm: @Composable (Modifier) -> Unit = { modifier ->
            Button(
                copy.text(if (pending) AuthText.LogoutPending else AuthText.Logout), onConfirm,
                // The polite region announces progress; a disabled control cannot be activated again.
                modifier.semantics { if (pending) liveRegion = LiveRegionMode.Polite },
                state = if (pending) ButtonState.Loading(DisabledReason.of(copy.text(AuthText.LogoutPendingReason))) else ButtonState.Enabled,
                variant = ButtonVariant.AuthPrimary,
                icon = authActionIcon(AuthIntent.Logout),
                controlModifier = Modifier.fillMaxWidth().heightIn(min = SheetActionHeight).testTag("logout-confirm"),
            )
        }
        if (stacked) Column(verticalArrangement = Arrangement.spacedBy(SheetActionGap)) {
            cancel(Modifier.fillMaxWidth())
            confirm(Modifier.fillMaxWidth())
        } else Row(horizontalArrangement = Arrangement.spacedBy(SheetActionGap), verticalAlignment = Alignment.Top) {
            cancel(Modifier.weight(1f))
            confirm(Modifier.weight(1f))
        }
    }
}

private val SheetActionHeight = 56.dp
private val SheetActionGap = 12.dp

@Composable
private fun SheetHandle() {
    Box(
        Modifier.padding(vertical = tokenDp("space.md")).size(width = 32.dp, height = 4.dp).clip(CircleShape)
            .background(MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f))
            .clearAndSetSemantics { },
    )
}
