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
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow

/**
 * Single is the Google/YouTube account default; Pair takes the first and last named words,
 * so a Vietnamese name keeps its given name ("Nguyễn Thị Ánh" → "NÁ").
 */
enum class AvatarInitials { Single, Pair }

enum class AvatarSize(internal val token: String) {
    Small("size.avatarSmall"),
    Medium("size.avatarMedium"),
    /** A 48-unit header control: the same hit area as an icon action, never smaller. */
    Control("size.avatarControl"),
    Large("size.avatarLarge"),
}

/**
 * Stable identity hue, never a status. Order is part of the shared contract: the index is
 * FNV-1a(seed) mod 6 on both renderers (see packages/ui-system/shared/README.md#avatar).
 */
enum class AvatarTone { Lavender, Gold, Sage, Rose, Sky, Clay }

/**
 * Text fallback for a person or creator. Returns "" when no word starts with a letter or
 * digit; the avatar then shows its plain tone, as Google does, instead of a "?" that reads as
 * an error. Combining marks after the initial are kept, so decomposed Vietnamese stays intact.
 */
fun avatarInitials(displayName: String, mode: AvatarInitials = AvatarInitials.Single): String {
    val letters = words(displayName).mapNotNull(::wordInitial)
    return when {
        letters.isEmpty() -> ""
        mode == AvatarInitials.Pair && letters.size > 1 -> letters.first() + letters.last()
        else -> letters.first()
    }
}

/**
 * Colour seed: a caller-owned stable ID (account or creator ID) when present, so a rename
 * keeps the colour; otherwise the display name with whitespace runs collapsed.
 */
fun avatarSeed(displayName: String, stableId: String? = null): String =
    stableId?.trim()?.takeIf { it.isNotEmpty() } ?: words(displayName).joinToString(" ")

/** FNV-1a over UTF-8; never platform hashCode or randomized hashing. */
fun avatarTone(seed: String): AvatarTone {
    var hash = 0x811c9dc5.toInt()
    seed.encodeToByteArray().forEach { byte ->
        hash = (hash xor (byte.toInt() and 0xff)) * 0x01000193
    }
    return AvatarTone.entries[(hash.toUInt() % AvatarTone.entries.size.toUInt()).toInt()]
}

private fun words(text: String): List<String> = buildList {
    val word = StringBuilder()
    for (char in text) {
        if (char.isWhitespace()) {
            if (word.isNotEmpty()) add(word.toString())
            word.clear()
        } else {
            word.append(char)
        }
    }
    if (word.isNotEmpty()) add(word.toString())
}

// Supplementary-plane characters (emoji, CJK extension B) are skipped on both renderers:
// Kotlin common has no code-point classification, and parity matters more than coverage.
private fun startsInitial(char: Char): Boolean =
    !char.isSurrogate() && (char.isLetter() || char in '0'..'9')

private fun isCombiningMark(char: Char): Boolean = when (char.code) {
    in 0x0300..0x036F, in 0x1AB0..0x1AFF, in 0x1DC0..0x1DFF, in 0x20D0..0x20FF,
    in 0xFE20..0xFE2F -> true
    else -> false
}

private fun wordInitial(word: String): String? {
    val start = word.indexOfFirst(::startsInitial)
    if (start < 0) return null
    val marks = word.drop(start + 1).takeWhile(::isCombiningMark)
    // Uppercasing may expand one character ("ß" → "SS"); one avatar slot keeps the first.
    return word[start].uppercase().first() + marks
}

/** Initials fill a fixed circle, so they scale with the diameter rather than font scale. */
private const val SINGLE_GLYPH_RATIO = 0.44f
private const val PAIR_GLYPH_RATIO = 0.36f

/**
 * Pure renderer of a caller-owned decoded image or the text fallback.
 *
 * Fetching, cancellation, caching, size limits and decoding belong to the host adapter; pass
 * null while loading or after a failure and the initials show without a blank frame. The tone
 * stays underneath an image, so transparent artwork never sits on an undefined background.
 *
 * Decorative by default: adjacent text supplies the name. Pass a localized [accessibleName]
 * only when the avatar stands alone and must convey who it is. [accent] pins the Theatre accent
 * pair instead of the identity hue; initials and semantics are unchanged.
 *
 * Registered in both preview catalogues; apps adopt it only after both renderers qualify.
 */
@Composable
fun Avatar(
    displayName: String,
    modifier: Modifier = Modifier,
    stableId: String? = null,
    image: ImageBitmap? = null,
    size: AvatarSize = AvatarSize.Medium,
    initials: AvatarInitials = AvatarInitials.Single,
    accessibleName: AccessibleName? = null,
    accent: Boolean = false,
) {
    val tone = avatarTone(avatarSeed(displayName, stableId))
    // The accent pins the Theatre pink pair; the identity hue stays the default and is never inferred.
    val fill = if (accent) "color.theatrePink" else "color.avatar${tone.name}"
    val ink = if (accent) "color.theatreOnPink" else "color.onAvatar${tone.name}"
    val diameter = tokenDp(size.token)
    val ratio = if (initials == AvatarInitials.Pair) PAIR_GLYPH_RATIO else SINGLE_GLYPH_RATIO
    // toSp divides by fontScale, keeping the glyph inside the circle at large text sizes.
    val glyph = with(LocalDensity.current) { (diameter * ratio).toSp() }
    Box(
        modifier = modifier.size(diameter).clip(CircleShape)
            .background(tokenColor(fill))
            .clearAndSetSemantics {
                if (accessibleName != null) {
                    contentDescription = accessibleName.text
                    role = Role.Image
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        if (image != null) {
            Image(image, contentDescription = null, modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop)
        } else {
            Text(
                text = avatarInitials(displayName, initials),
                color = tokenColor(ink),
                style = MaterialTheme.typography.labelLarge.copy(fontSize = glyph, lineHeight = glyph),
                maxLines = 1,
                softWrap = false,
                overflow = TextOverflow.Clip,
                textAlign = TextAlign.Center,
            )
        }
    }
}
