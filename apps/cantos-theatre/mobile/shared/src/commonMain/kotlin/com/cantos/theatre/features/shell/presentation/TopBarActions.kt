package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.layout.size
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.ui.generated.CantosTokens
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
internal fun TopBarThemeButton(theme: ShellTheme, label: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    FilledTonalIconButton(
        onClick = onClick,
        modifier = modifier.size(tokenDp("size.control")),
        shape = MaterialTheme.shapes.small,
        colors = IconButtonDefaults.filledTonalIconButtonColors(
            containerColor = tokenColor("color.theatreSurfaceContainerHigh"),
            contentColor = tokenColor("color.theatreOnSurface"),
        ),
    ) {
        Icon(
            rememberCantosIconVector(if (theme == ShellTheme.Dark) "light_mode" else "dark_mode"),
            label,
            Modifier.size(tokenDp("size.icon")),
        )
    }
}

/**
 * Account action of the Theatre top bar: a 48 dp circular text avatar. The letter is the generic
 * Cantos fallback; the app holds no display name here, so none is invented and the account state
 * keeps its owner. The glyph is decorative: the localized [label] is the single accessible name.
 */
@Composable
internal fun TopBarAccountButton(label: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    FilledIconButton(
        onClick = onClick,
        modifier = modifier.size(tokenDp("size.avatarControl")).semantics { contentDescription = label },
        colors = IconButtonDefaults.filledIconButtonColors(
            containerColor = tokenColor("color.theatrePink"),
            contentColor = tokenColor("color.theatreOnPink"),
        ),
    ) {
        Text(
            GENERIC_AVATAR_INITIAL,
            Modifier.clearAndSetSemantics { },
            style = MaterialTheme.typography.titleMedium.copy(
                fontWeight = FontWeight(CantosTokens.number("type.theatreControl.weight").toInt()),
            ),
        )
    }
}

private const val GENERIC_AVATAR_INITIAL = "C"
