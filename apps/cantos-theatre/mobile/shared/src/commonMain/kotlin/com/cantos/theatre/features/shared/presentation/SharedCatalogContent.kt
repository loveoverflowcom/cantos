package com.cantos.theatre.features.shared.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import com.cantos.theatre.application.catalog.SharedCatalogPhase
import com.cantos.theatre.application.catalog.SharedCatalogState
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellCopy
import com.cantos.ui.Button
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.DisabledReason
import com.cantos.ui.Surface
import com.cantos.ui.SurfaceVariant
import com.cantos.ui.tokenDp

/** Native composition over S07 facts. No artwork fetch, player, public cache or ACL decision. */
@Composable
fun SharedCatalogContent(
    state: SharedCatalogState,
    copy: ShellCopy,
    onOpen: (PublicWorkId) -> Unit,
    onSignIn: () -> Unit,
    onRefresh: () -> Unit,
    onMore: () -> Unit,
    modifier: Modifier = Modifier,
    detail: Boolean = false,
) {
    Surface(modifier = modifier.fillMaxWidth().testTag("shared-catalog"), variant = SurfaceVariant.AccountGroup) {
        Column(Modifier.padding(tokenDp("space.inset")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            Text(copy.text(CopyKey.SharedTitle), style = MaterialTheme.typography.titleLarge,
                modifier = Modifier.semantics { heading() })
            when (state.phase) {
                SharedCatalogPhase.Idle, SharedCatalogPhase.Loading -> Text(copy.text(CopyKey.SharedLoading))
                SharedCatalogPhase.SignInRequired -> Button(copy.text(CopyKey.SharedLogin), onSignIn,
                    controlModifier = Modifier.testTag("shared-sign-in"))
                SharedCatalogPhase.Denied -> {
                    Text(copy.text(CopyKey.SharedGeneric), style = MaterialTheme.typography.titleMedium)
                    Text(copy.text(CopyKey.SharedDenied))
                }
                SharedCatalogPhase.Restart -> Text(copy.text(CopyKey.SharedRestart))
                SharedCatalogPhase.Unavailable, SharedCatalogPhase.Protocol -> Text(copy.text(CopyKey.SharedUnavailable))
                SharedCatalogPhase.Ready -> {
                    if (detail) {
                        state.work?.let { work ->
                            Text(work.title, style = MaterialTheme.typography.headlineSmall,
                                modifier = Modifier.semantics { heading() })
                            if (work.description.isNotBlank()) Text(work.description)
                            Text(copy.text(CopyKey.SharedBadge), style = MaterialTheme.typography.labelLarge)
                        }
                        state.units.forEach { unit -> key(unit.id) {
                            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                                Text(unit.title, style = MaterialTheme.typography.titleMedium)
                                if (unit.description.isNotBlank()) Text(unit.description)
                                // A metadata playback field is not a qualified native transport capability.
                                Button(copy.text(CopyKey.Play), {}, state = ButtonState.Disabled(DisabledReason.of(
                                    copy.text(if (unit.publicationId == null) CopyKey.EpisodeNoAudio else CopyKey.SharedNativeUnavailable))),
                                    controlModifier = Modifier.testTag("shared-play-${unit.id.value}"))
                            }
                        } }
                        Text(copy.text(CopyKey.SharedRestoreUnavailable), style = MaterialTheme.typography.bodySmall)
                    } else {
                        if (state.works.isEmpty()) Text(copy.text(CopyKey.SharedEmpty))
                        state.works.forEach { work -> key(work.id) {
                            Button(work.title, { onOpen(work.id) }, variant = ButtonVariant.Tonal,
                                modifier = Modifier.fillMaxWidth(), controlModifier = Modifier.testTag("shared-work-${work.id.value}"))
                        } }
                    }
                    if (state.cursor != null) Button(copy.text(CopyKey.LoadMore), onMore, variant = ButtonVariant.Text,
                        controlModifier = Modifier.testTag("shared-more"))
                }
            }
            if (state.phase != SharedCatalogPhase.Loading && state.phase != SharedCatalogPhase.SignInRequired) {
                Button(copy.text(CopyKey.SharedRefresh), onRefresh, variant = ButtonVariant.Text,
                    controlModifier = Modifier.testTag("shared-refresh"))
            }
        }
    }
}
