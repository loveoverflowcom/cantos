package com.cantos.ui

import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon as MaterialIcon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosIcons
import com.cantos.ui.generated.CantosTokens

/** Metadata remains muted on regular surfaces; an explicit paired surface may override it. */
internal val LocalSupportingTextRole = staticCompositionLocalOf { "color.onSurfaceVariant" }

@Composable
fun UiRoot(
    modifier: Modifier = Modifier,
    preferences: UiPreferences = UiPreferences(),
    content: @Composable () -> Unit,
) {
    CantosTheme(preferences) {
        RootContent(modifier, content)
    }
}

@Composable
fun FieldLabel(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier, style = MaterialTheme.typography.labelLarge)
}

@Composable
fun FieldHelp(text: String, modifier: Modifier = Modifier) {
    if (text.isBlank()) return
    Text(text, modifier, style = MaterialTheme.typography.bodySmall, color = tokenColor(LocalSupportingTextRole.current))
}

@Composable
fun FieldError(text: String, modifier: Modifier = Modifier) {
    if (text.isBlank()) return
    Text(text, modifier.semantics { error(text) }, style = MaterialTheme.typography.bodySmall,
        color = tokenColor("color.error"))
}

/** Decorative official Material SVG paths; the enclosing control supplies its name. */
@Composable
fun Icon(name: String, modifier: Modifier = Modifier, tint: Color = LocalContentColor.current) {
    val vector = remember(name, tint) { buildCantosIconVector(name, tint) }
    MaterialIcon(vector, contentDescription = null, modifier = modifier.size(tokenDp("size.icon")), tint = tint)
}

/**
 * Registry glyph as a tint-free vector for a Material `Icon(imageVector, contentDescription)`
 * call site that owns its own size, tint and accessible name. Unknown names fail closed.
 */
@Composable
fun rememberCantosIconVector(name: String): ImageVector = remember(name) { buildCantosIconVector(name) }

/** Pure builder shared by both entry points; paint comes from `tint` (black when tinted later). */
fun buildCantosIconVector(name: String, tint: Color = Color.Black): ImageVector {
    val icon = CantosIcons.icons.firstOrNull { it.name == name }
    require(icon != null) { "Unknown official icon: $name" }
    val bounds = icon.viewBox.split(Regex("\\s+")).map { it.toFloat() }
    return ImageVector.Builder(
        name = name,
        defaultWidth = CantosTokens.number("size.icon").toFloat().dp,
        defaultHeight = CantosTokens.number("size.icon").toFloat().dp,
        viewportWidth = bounds[2], viewportHeight = bounds[3],
        autoMirror = icon.autoMirror,
    ).apply {
        icon.pathData.forEach { pathData ->
            addPath(pathData = PathParser().parsePathString(pathData).toNodes(), fill = SolidColor(tint))
        }
    }.build()
}
