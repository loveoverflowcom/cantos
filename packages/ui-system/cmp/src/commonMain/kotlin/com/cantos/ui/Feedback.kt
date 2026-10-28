package com.cantos.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription

private fun StatusTone.role(): String = when (this) {
    StatusTone.Neutral -> "surface"
    StatusTone.Info -> "primary"
    StatusTone.Success -> "success"
    StatusTone.Warning -> "warning"
    StatusTone.Error -> "error"
}

private fun Modifier.announcement(mode: Announcement): Modifier = semantics {
    when (mode) {
        Announcement.Off -> Unit
        Announcement.Polite -> liveRegion = LiveRegionMode.Polite
        Announcement.Assertive -> liveRegion = LiveRegionMode.Assertive
    }
}

/** A bounded reading radius keeps wrapped labels inside the painted surface at large text. */
@Composable
fun Badge(text: String, modifier: Modifier = Modifier, tone: StatusTone = StatusTone.Neutral) {
    androidx.compose.material3.Surface(modifier,
        color = tokenColor("color.${tone.role()}Container"),
        contentColor = when (tone) {
            StatusTone.Neutral -> tokenColor("color.onSurface")
            StatusTone.Info -> tokenColor("color.onPrimaryContainer")
            else -> tokenColor("color.${tone.role()}")
        }, shape = CantosShapes.reading) {
        Text(text, Modifier.padding(horizontal = tokenDp("space.inset"), vertical = tokenDp("space.xs")),
            style = MaterialTheme.typography.labelSmall)
    }
}

/** Hosts choose announcement urgency and own deduplication/throttling. */
@Composable
fun Status(
    text: String,
    modifier: Modifier = Modifier,
    tone: StatusTone = StatusTone.Neutral,
    announce: Boolean = false,
    announcement: Announcement = if (announce) Announcement.Polite else Announcement.Off,
) {
    Row(modifier.announcement(announcement), horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Icon(when (tone) {
            StatusTone.Error, StatusTone.Warning -> "warning_amber"
            StatusTone.Success -> "check"
            StatusTone.Neutral, StatusTone.Info -> "menu_book"
        })
        Badge(text, tone = tone)
    }
}

/**
 * Indeterminate loading with visible localized text. The shared [LoadingIndicator] turns under full
 * motion and stays a stationary arc under reduced motion; the text is the status either way, and the
 * indicator is never a disclosure glyph.
 */
@Composable
fun LoadingState(text: String, modifier: Modifier = Modifier, announcement: Announcement = Announcement.Off) {
    Row(modifier.announcement(announcement).semantics { stateDescription = text },
        horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        LoadingIndicator(size = LoadingSize.Standard)
        Text(text)
    }
}

@Composable
fun EmptyState(title: String, message: String, modifier: Modifier = Modifier, action: (@Composable () -> Unit)? = null) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Icon("menu_book")
        Text(title, style = MaterialTheme.typography.titleLarge)
        Text(message, color = tokenColor("color.onSurfaceVariant"))
        action?.invoke()
    }
}

/** Compatibility retry action delegates to the composable recovery slot. */
@Composable
fun ErrorState(
    title: String,
    message: String,
    retryLabel: String,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
    announcement: Announcement = Announcement.Off,
) {
    ErrorState(title, message, modifier, announcement) {
        Button(retryLabel, onRetry, variant = ButtonVariant.Secondary)
    }
}

@Composable
fun ErrorState(
    title: String,
    message: String,
    modifier: Modifier = Modifier,
    announcement: Announcement = Announcement.Off,
    recovery: (@Composable () -> Unit)? = null,
) {
    Column(modifier.announcement(announcement), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Status(title, tone = StatusTone.Error)
        Text(message)
        recovery?.invoke()
    }
}
