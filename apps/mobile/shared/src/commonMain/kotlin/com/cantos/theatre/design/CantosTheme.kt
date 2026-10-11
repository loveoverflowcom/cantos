package com.cantos.theatre.design

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
fun CantosTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val colors = if (dark) {
        with(GeneratedTokens.Dark) {
            darkColorScheme(
                primary = primary, onPrimary = onPrimary,
                primaryContainer = primaryContainer, onPrimaryContainer = onPrimaryContainer,
                background = surface, onBackground = onSurface,
                surface = surfaceRaised, onSurface = onSurface,
                surfaceVariant = surfaceContainer, onSurfaceVariant = onSurfaceVariant,
                surfaceContainer = surfaceContainer, surfaceContainerHigh = surfaceContainerHigh,
                outline = outline, outlineVariant = outlineVariant,
                error = error, errorContainer = errorContainer,
            )
        }
    } else {
        with(GeneratedTokens.Light) {
            lightColorScheme(
                primary = primary, onPrimary = onPrimary,
                primaryContainer = primaryContainer, onPrimaryContainer = onPrimaryContainer,
                background = surface, onBackground = onSurface,
                surface = surfaceRaised, onSurface = onSurface,
                surfaceVariant = surfaceContainer, onSurfaceVariant = onSurfaceVariant,
                surfaceContainer = surfaceContainer, surfaceContainerHigh = surfaceContainerHigh,
                outline = outline, outlineVariant = outlineVariant,
                error = error, errorContainer = errorContainer,
            )
        }
    }
    MaterialTheme(
        colorScheme = colors,
        shapes = Shapes(
            small = RoundedCornerShape(GeneratedTokens.shape_field.dp),
            medium = RoundedCornerShape(GeneratedTokens.shape_container.dp),
            large = RoundedCornerShape(GeneratedTokens.shape_control.dp),
        ),
        typography = Typography(
            headlineMedium = TextStyle(fontFamily = FontFamily.Serif, fontSize = GeneratedTokens.type_title_size.sp),
            titleLarge = TextStyle(fontFamily = FontFamily.Serif, fontSize = GeneratedTokens.type_title_size.sp),
            bodyLarge = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = GeneratedTokens.type_body_size.sp),
            labelSmall = TextStyle(fontFamily = FontFamily.SansSerif, fontSize = GeneratedTokens.type_metadata_size.sp),
        ),
        content = content,
    )
}
