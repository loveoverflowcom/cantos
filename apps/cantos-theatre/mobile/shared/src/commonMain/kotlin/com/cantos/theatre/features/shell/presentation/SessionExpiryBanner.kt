package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.ui.AccessibleName
import com.cantos.ui.Button
import com.cantos.ui.ButtonVariant
import com.cantos.ui.IconButton
import com.cantos.ui.Space
import com.cantos.ui.Surface
import com.cantos.ui.SurfaceTone
import com.cantos.ui.tokenDp

/** A scrollable inline composition of admitted controls; it never takes focus or owns a session. */
@Composable
internal fun SessionExpiryBanner(copy: ShellCopy, onSignIn: () -> Unit, onDismiss: () -> Unit) {
    Surface(Modifier.fillMaxWidth().testTag("session-expiry-banner").semantics { liveRegion = LiveRegionMode.Polite },
        tone = SurfaceTone.Container, inset = Space.Md) {
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
                Text(copy.text(CopyKey.SessionExpiredBanner), style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f).semantics { heading() })
                IconButton(AccessibleName.of(copy.text(CopyKey.SessionExpiredDismiss)), "close", onDismiss,
                    controlModifier = Modifier.testTag("session-expiry-dismiss"))
            }
            Text(copy.text(CopyKey.SessionExpiredBannerBody), style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
            Button(copy.text(CopyKey.SignIn), onSignIn, variant = ButtonVariant.Text,
                controlModifier = Modifier.testTag("session-expiry-sign-in"))
        }
    }
}
