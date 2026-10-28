package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellThemeMode
import com.cantos.ui.rememberCantosIconVector
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
 * Theme action of the Theatre top bar: one tonal square on the same 48 dp centerline as the brand
 * and profile. The next theme is the caller's intent; this only presents it from canonical roles.
 */
@Composable
internal fun TopBarThemeButton(
    theme: ShellThemeMode,
    label: String,
    modeName: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    FilledTonalIconButton(
        onClick = onClick,
        modifier = modifier.size(tokenDp("size.control")).semantics {
            contentDescription = label
            stateDescription = modeName
        },
        shape = MaterialTheme.shapes.small,
        colors = IconButtonDefaults.filledTonalIconButtonColors(
            containerColor = tokenColor("color.theatreSurfaceContainerHigh"),
            contentColor = tokenColor("color.theatreOnSurface"),
        ),
    ) {
        when (theme) {
            ShellThemeMode.Light, ShellThemeMode.Dark -> Icon(
                rememberCantosIconVector(if (theme == ShellThemeMode.Dark) "dark_mode" else "light_mode"),
                null,
                Modifier.size(tokenDp("size.icon")),
            )
            ShellThemeMode.System -> Row {
                // Keep System distinct from either explicit mode without adding another icon asset.
                Icon(rememberCantosIconVector("dark_mode"), null, Modifier.size(tokenDp("size.icon") / 2))
                Icon(rememberCantosIconVector("light_mode"), null, Modifier.size(tokenDp("size.icon") / 2))
            }
        }
    }
}

/**
 * The account owner supplies admission only, never a display name. A person glyph stays anonymous;
 * only a confirmed account receives the avatar's pink container. Guests keep the same navigation.
 */
@Composable
internal fun TopBarAccountButton(
    label: String,
    signedIn: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    FilledTonalIconButton(
        onClick = onClick,
        modifier = modifier.size(tokenDp("size.avatarControl")).semantics { contentDescription = label },
        shape = CircleShape,
        colors = IconButtonDefaults.filledTonalIconButtonColors(
            containerColor = tokenColor(if (signedIn) "color.theatrePink" else "color.theatreSurfaceContainerHigh"),
            contentColor = tokenColor(if (signedIn) "color.theatreOnPink" else "color.theatreOnSurface"),
        ),
    ) {
        Icon(rememberCantosIconVector("person"), null, Modifier.size(tokenDp("size.icon")))
    }
}
