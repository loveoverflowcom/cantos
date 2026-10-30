package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import com.cantos.theatre.application.errors.ErrorAction
import com.cantos.theatre.application.errors.ErrorKind
import com.cantos.theatre.application.errors.ErrorMessageKey
import com.cantos.theatre.application.errors.ErrorNotice
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.ui.CantosShapes
import com.cantos.ui.tokenDp
import com.cantos.ui.rememberCantosIconVector

/**
 * The one standard modal error window for Cantos CMP (#298). Renders only the closed [ErrorNotice]
 * vocabulary through [ShellCopy] — never raw exception, SQL or provider text — and guarantees a
 * [ErrorAction.Close] path alongside any [ErrorAction.Retry] or [ErrorAction.SignIn] primary action.
 */
@Composable
fun ErrorDialog(
    notice: ErrorNotice,
    copy: ShellCopy,
    onAction: (ErrorAction) -> Unit,
    onDismiss: () -> Unit,
    returnFocus: FocusRequester? = null,
) {
    val primaryFocus = remember(notice.key) { FocusRequester() }
    // Desktop dialogs have a separate window. Bound the content to the measured app viewport,
    // just as on native, so large copy scrolls instead of pushing actions beyond a narrow host.
    val viewport = LocalLoadingViewport.current
    val maximumHeight = with(LocalDensity.current) { viewport.height.toDp() }
    val viewportConstraint = if (maximumHeight.value > 0f) Modifier.heightIn(max = maximumHeight) else Modifier
    val expired = notice.kind == ErrorKind.SessionExpired
    val primary = notice.primary
    val closeOnly = primary == ErrorAction.Close
    fun dismissAndRestore() {
        onDismiss()
        returnFocus?.let { runCatching { it.requestFocus() } }
    }
    fun chooseAndRestore(action: ErrorAction) {
        onAction(action)
        if (action == ErrorAction.Close) {
            returnFocus?.let { runCatching { it.requestFocus() } }
        }
    }
    LaunchedEffect(notice.key) {
        withFrameNanos { }
        runCatching { primaryFocus.requestFocus() }
    }
    AlertDialog(
        shape = CantosShapes.overlay,
        containerColor = MaterialTheme.colorScheme.surface,
        modifier = viewportConstraint
            .onPreviewKeyEvent {
                if (it.key == Key.Escape && it.type == KeyEventType.KeyUp) {
                    dismissAndRestore()
                    true
                } else {
                    false
                }
            }
            .testTag("error-dialog"),
        onDismissRequest = ::dismissAndRestore,
        icon = if (expired) ({ Icon(rememberCantosIconVector("timer"), null, tint = MaterialTheme.colorScheme.primary) }) else null,
        title = {
            Text(
                copy.text(if (expired) CopyKey.SessionExpiredTitle else CopyKey.ErrorDialogTitle),
                style = MaterialTheme.typography.titleLarge,
                modifier = Modifier.semantics { heading() }.testTag("error-dialog-title"),
            )
        },
        text = {
            Text(
                copy.text(notice.messageKey.toCopyKey()),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.verticalScroll(rememberScrollState()).testTag("error-dialog-body"),
            )
        },
        confirmButton = {
            if (closeOnly) {
                TextButton(
                    shape = MaterialTheme.shapes.small,
                    onClick = { chooseAndRestore(ErrorAction.Close) },
                    modifier = Modifier
                        .heightIn(min = tokenDp("size.control"))
                        .focusRequester(primaryFocus)
                        .testTag("error-dialog-close"),
                ) {
                    Text(copy.text(if (expired) CopyKey.SessionExpiredLater else CopyKey.CloseError))
                }
            } else {
                Button(
                    shape = MaterialTheme.shapes.medium,
                    onClick = { chooseAndRestore(primary) },
                    modifier = Modifier
                        .heightIn(min = tokenDp("size.control"))
                        .focusRequester(primaryFocus)
                        .testTag("error-dialog-primary"),
                ) {
                    Text(copy.text(primary.toCopyKey()))
                }
            }
        },
        dismissButton = if (closeOnly) {
            null
        } else {
            {
                TextButton(
                    shape = MaterialTheme.shapes.small,
                    onClick = { chooseAndRestore(ErrorAction.Close) },
                    modifier = Modifier
                        .heightIn(min = tokenDp("size.control"))
                        .testTag("error-dialog-close"),
                ) {
                    Text(copy.text(if (expired) CopyKey.SessionExpiredLater else CopyKey.CloseError))
                }
            }
        },
    )
}

internal fun ErrorMessageKey.toCopyKey(): CopyKey = when (this) {
    ErrorMessageKey.SessionExpired -> CopyKey.SessionExpiredBody
    ErrorMessageKey.Network -> CopyKey.ErrorNetwork
    ErrorMessageKey.Timeout -> CopyKey.ErrorTimeout
    ErrorMessageKey.Unauthenticated -> CopyKey.ErrorUnauthenticated
    ErrorMessageKey.Forbidden -> CopyKey.ErrorForbidden
    ErrorMessageKey.ContentUnavailable -> CopyKey.ErrorContentUnavailable
    ErrorMessageKey.Protocol -> CopyKey.ErrorProtocol
    ErrorMessageKey.Configuration -> CopyKey.ErrorConfiguration
    ErrorMessageKey.Storage -> CopyKey.ErrorStorage
    ErrorMessageKey.ServiceUnavailable -> CopyKey.ErrorServiceUnavailable
}

internal fun ErrorAction.toCopyKey(): CopyKey = when (this) {
    ErrorAction.Retry -> CopyKey.Retry
    ErrorAction.SignIn -> CopyKey.SignIn
    ErrorAction.Close -> CopyKey.CloseError
}
