package com.cantos.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Density as DisplayDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.shape.RoundedCornerShape
import com.cantos.ui.generated.CantosTokens

/** Renderer preferences only. No production app or domain state belongs here. */
data class UiPreferences(
    val theme: Theme = Theme.Light,
    val density: Density = Density.Compact,
    val reducedMotion: Boolean = false,
    val textScale: Float = 1f,
)

val LocalUiPreferences = staticCompositionLocalOf { UiPreferences() }

@Composable
fun tokenDp(name: String): Dp = CantosTokens.number(
    name,
    density = LocalUiPreferences.current.density.tokenName,
).toFloat().dp

@Composable
fun tokenColor(name: String): Color = parseTokenColor(
    CantosTokens.color(name, LocalUiPreferences.current.theme.tokenName),
)

fun parseTokenColor(hex: String): Color {
    require(hex.matches(Regex("#[0-9a-fA-F]{6}"))) { "Expected a six-digit shared color token" }
    return Color(hex.drop(1).toInt(16) or (255 shl 24))
}

internal fun typeStyle(role: String, editorial: Boolean = false): TextStyle {
    val size = CantosTokens.number("type.$role.size").toFloat()
    val family = CantosTokens.text(if (editorial) "type.family.editorial" else "type.family.ui")
    val fallback = when {
        family.contains("Noto Sans") -> FontFamily.SansSerif
        family.contains("Noto Serif") -> FontFamily.Serif
        else -> error("No reviewed platform mapping for shared font family: $family")
    }
    return TextStyle(
        // Shared contract names Noto families; no unlicensed embedded font is redistributed.
        // Platform fallback is explicit until a reviewed Noto font-resource slice is adopted.
        fontFamily = fallback,
        fontSize = size.sp,
        lineHeight = (size * CantosTokens.number("type.$role.lineHeight").toFloat()).sp,
        fontWeight = FontWeight(CantosTokens.number("type.$role.weight").toInt()),
    )
}

@Composable
fun CantosTheme(preferences: UiPreferences, content: @Composable () -> Unit) {
    fun color(name: String): Color = parseTokenColor(CantosTokens.color(name, preferences.theme.tokenName))
    val primary = color("color.primary")
    val onPrimary = color("color.onPrimary")
    val primaryContainer = color("color.primaryContainer")
    val onPrimaryContainer = color("color.onPrimaryContainer")
    val surface = color("color.surface")
    val onSurface = color("color.onSurface")
    val container = color("color.surfaceContainer")
    val highContainer = color("color.surfaceContainerHigh")
    val scheme = if (preferences.theme == Theme.Dark) darkColorScheme() else lightColorScheme()
    val mapped = scheme.copy(
        primary = primary,
        onPrimary = onPrimary,
        primaryContainer = primaryContainer,
        onPrimaryContainer = onPrimaryContainer,
        inversePrimary = primaryContainer,
        secondary = primary,
        onSecondary = onPrimary,
        secondaryContainer = container,
        onSecondaryContainer = onSurface,
        tertiary = color("color.accent"),
        onTertiary = surface,
        tertiaryContainer = highContainer,
        onTertiaryContainer = onSurface,
        background = color("color.background"),
        onBackground = onSurface,
        surface = surface,
        onSurface = onSurface,
        surfaceVariant = container,
        onSurfaceVariant = color("color.onSurfaceVariant"),
        surfaceTint = primary,
        inverseSurface = onSurface,
        inverseOnSurface = surface,
        error = color("color.error"),
        onError = color("color.errorContainer"),
        errorContainer = color("color.errorContainer"),
        onErrorContainer = color("color.error"),
        outline = color("color.outline"),
        outlineVariant = color("color.outlineVariant"),
        scrim = onSurface,
        surfaceBright = surface,
        surfaceDim = container,
        surfaceContainer = container,
        surfaceContainerHigh = highContainer,
        surfaceContainerHighest = highContainer,
        surfaceContainerLow = surface,
        surfaceContainerLowest = color("color.background"),
    )
    val body = typeStyle("body")
    val title = typeStyle("title")
    val headline = typeStyle("headline")
    val label = typeStyle("label")
    val metadata = typeStyle("metadata")
    val currentDensity = LocalDensity.current
    CompositionLocalProvider(
        LocalUiPreferences provides preferences,
        LocalDensity provides DisplayDensity(currentDensity.density, currentDensity.fontScale * preferences.textScale),
    ) {
        MaterialTheme(
            colorScheme = mapped,
            typography = Typography(
                displayLarge = headline, displayMedium = headline, displaySmall = headline,
                headlineLarge = headline, headlineMedium = headline, headlineSmall = headline,
                titleLarge = title, titleMedium = title, titleSmall = title,
                bodyLarge = body, bodyMedium = body, bodySmall = metadata,
                labelLarge = label, labelMedium = label, labelSmall = metadata,
            ),
            shapes = Shapes(
                extraSmall = RoundedCornerShape(tokenDp("shape.reading")),
                small = RoundedCornerShape(tokenDp("shape.reading")),
                medium = RoundedCornerShape(tokenDp("shape.container")),
                large = RoundedCornerShape(tokenDp("shape.control")),
                extraLarge = RoundedCornerShape(tokenDp("shape.overlay")),
            ),
            content = content,
        )
    }
}

@Composable
internal fun RootContent(modifier: Modifier, content: @Composable () -> Unit) {
    androidx.compose.material3.Surface(
        modifier = modifier.fillMaxSize(),
        color = tokenColor("color.background"),
        contentColor = tokenColor("color.onSurface"),
    ) {
        Box(Modifier.fillMaxSize()) { content() }
    }
}
