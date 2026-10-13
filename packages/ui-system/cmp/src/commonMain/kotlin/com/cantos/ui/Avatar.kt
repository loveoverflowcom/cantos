package com.cantos.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/** Initials are rendered locally; there is no generated image or network request. */
enum class AvatarInitialMode { Single, Pair }

enum class AvatarSize { Small, Medium, Large }

/**
 * A stable index into semantic theme pairs, not a status color.
 * Indices and FNV-1a hashing must match the Leptos Avatar implementation.
 */
enum class AvatarTone { Soft, Strong, Elevated, Quiet }

/**
 * Single initial matches a Google-style account avatar. Pair takes the first and last
 * meaningful words, following VOTHub's initials fallback. Keep Vietnamese diacritics.
 */
fun avatarInitials(displayName: String, mode: AvatarInitialMode = AvatarInitialMode.Single): String {
    val letters = displayName.trim().split(Regex("\\s+")).mapNotNull { word ->
        word.firstOrNull { it.isLetterOrDigit() }?.uppercase()?.firstOrNull()?.toString()
    }
    if (letters.isEmpty()) return "?"
    return if (mode == AvatarInitialMode.Pair && letters.size > 1) {
        letters.first() + letters.last()
    } else {
        letters.first()
    }
}

/** FNV-1a over UTF-8; never use platform hashCode or randomized hash maps for avatar colors. */
fun avatarTone(seed: String): AvatarTone {
    var hash = 0x811c9dc5.toInt()
    seed.trim().encodeToByteArray().forEach { byte ->
        hash = (hash xor (byte.toInt() and 0xff)) * 0x01000193
    }
    return AvatarTone.entries[(hash.toUInt() % AvatarTone.entries.size.toUInt()).toInt()]
}

/**
 * Pure renderer of a caller-owned decoded image or text fallback.
 *
 * Image fetching, cancellation, caching and decoding belong to the host adapter. Pass null
 * when an image is unavailable or fails; this primitive remains offline-safe.
 *
 * Decorative by default: adjacent text supplies the user's name. Supply a localized
 * AccessibleName only when the avatar itself must convey that name.
 *
 * Initially internal until the matching Leptos component and both preview catalogues land.
 */
@Composable
internal fun Avatar(
    displayName: String,
    modifier: Modifier = Modifier,
    stableId: String? = null,
    image: ImageBitmap? = null,
    size: AvatarSize = AvatarSize.Medium,
    initials: AvatarInitialMode = AvatarInitialMode.Single,
    accessibleName: AccessibleName? = null,
) {
    val tone = avatarTone(stableId?.trim()?.takeIf { it.isNotEmpty() } ?: displayName)
    val (background, foreground) = when (tone) {
        AvatarTone.Soft -> tokenColor("color.primaryContainer") to tokenColor("color.onPrimaryContainer")
        AvatarTone.Strong -> tokenColor("color.primary") to tokenColor("color.onPrimary")
        AvatarTone.Elevated -> tokenColor("color.surfaceContainerHigh") to tokenColor("color.onSurface")
        AvatarTone.Quiet -> tokenColor("color.surfaceContainer") to tokenColor("color.onSurface")
    }
    val diameter = when (size) {
        AvatarSize.Small -> 32.dp
        AvatarSize.Medium -> tokenDp("size.control")
        AvatarSize.Large -> 64.dp
    }
    Box(
        modifier = modifier.size(diameter).clip(CircleShape).background(background)
            .clearAndSetSemantics {
                if (accessibleName != null) contentDescription = accessibleName.text
            },
        contentAlignment = Alignment.Center,
    ) {
        if (image != null) {
            Image(image, contentDescription = null, modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop)
        } else {
            Text(
                text = avatarInitials(displayName, initials),
                color = foreground,
                style = when (size) {
                    AvatarSize.Small -> MaterialTheme.typography.labelSmall
                    AvatarSize.Medium -> MaterialTheme.typography.labelLarge
                    AvatarSize.Large -> MaterialTheme.typography.titleLarge
                },
                maxLines = 1,
                overflow = TextOverflow.Clip,
                textAlign = TextAlign.Center,
            )
        }
    }
}
