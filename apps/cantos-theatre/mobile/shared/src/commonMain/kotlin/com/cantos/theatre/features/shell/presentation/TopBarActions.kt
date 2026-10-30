package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.size
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellThemeMode
import com.cantos.ui.CantosShapes
import com.cantos.ui.ThemeMode
import com.cantos.ui.rememberCantosIconVector
import com.cantos.ui.themeModeIconName
import com.cantos.ui.tokenColor
import com.cantos.ui.tokenDp

/**
 * Top-level destinations carry the official brand mark before the wordmark, with Back first when
 * the stack has one; every drill-down route keeps Back (Close for the player) and the title only.
 */
internal fun topBarShowsBrandMark(route: ShellRoute): Boolean = when (route) {
    ShellRoute.Discovery, ShellRoute.Search, ShellRoute.Library, ShellRoute.History -> true
    else -> false
}

/**
 * Theme action of the Theatre top bar: one tonal reading-radius control on the same 48 dp
 * centerline as the brand and profile, painting the shared registry glyph of the current mode
 * ([themeModeIconName]). The next theme is the caller's intent; this only presents it.
 */
@Composable
internal fun TopBarThemeButton(
    theme: ShellThemeMode,
    label: String,
    modeName: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val mode = when (theme) {
        ShellThemeMode.Light -> ThemeMode.Light
        ShellThemeMode.Dark -> ThemeMode.Dark
        ShellThemeMode.System -> ThemeMode.System
    }
    FilledTonalIconButton(
        onClick = onClick,
        modifier = modifier.size(tokenDp("size.control")).semantics {
            contentDescription = label
            stateDescription = modeName
        },
        shape = CantosShapes.reading,
        colors = IconButtonDefaults.filledTonalIconButtonColors(
            containerColor = tokenColor("color.theatreSurfaceContainerHigh"),
            contentColor = tokenColor("color.theatreOnSurface"),
        ),
    ) {
        Icon(rememberCantosIconVector(themeModeIconName(mode)), null, Modifier.size(tokenDp("size.icon")))
    }
}

/**
 * Navigation uses the same generated presentation as Me. Missing presentation keeps the
 * existing anonymous affordance; a raw actor or credential is never a display name.
 */
@Composable
internal fun TopBarAccountButton(
    label: String,
    signedIn: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    presentation: com.cantos.theatre.domain.identity.AccountPresentation? = null,
) {
    FilledTonalIconButton(
        onClick = onClick,
        modifier = modifier.size(tokenDp("size.avatarControl")).semantics { contentDescription = presentation?.let { "$label: ${it.alias}" } ?: label },
        shape = CantosShapes.full,
        colors = IconButtonDefaults.filledTonalIconButtonColors(
            containerColor = tokenColor(if (signedIn) "color.theatrePink" else "color.theatreSurfaceContainerHigh"),
            contentColor = tokenColor(if (signedIn) "color.theatreOnPink" else "color.theatreOnSurface"),
        ),
    ) {
        if (presentation != null) com.cantos.ui.Avatar(
            displayName = presentation.alias, stableId = presentation.avatarSeed,
            size = com.cantos.ui.AvatarSize.Control, initials = com.cantos.ui.AvatarInitials.Pair,
            tone = com.cantos.ui.AvatarTone.Lavender,
        ) else Icon(rememberCantosIconVector("person"), null, Modifier.size(tokenDp("size.icon")))
    }
}
