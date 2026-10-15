package com.cantos.ui.preview

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import com.cantos.ui.generated.CantosTokens

fun main() = application {
    // Desktop window geometry (dp) only: the default width is the Wide preset and the height reuses the
    // expanded breakpoint value. All semantic visual values still resolve shared tokens.
    val windowHeight = CantosTokens.number("breakpoint.expanded").toFloat().dp
    val window = rememberWindowState(width = ViewportPreset.Wide.widthDp.dp, height = windowHeight)
    Window(onCloseRequest = ::exitApplication, state = window, title = "Cantos UI System · CMP") {
        // This host is the composition root: it owns the operating-system query and injects the fact.
        // It also owns font delivery: the licensed Noto bundle, injected so the gallery matches the design's families.
        PreviewApp(
            systemIsDark = isSystemInDarkTheme(),
            onWidthPreset = { preset -> window.size = DpSize(preset.widthDp.dp, window.size.height) },
            uiFontFamily = PreviewFonts.ui,
            editorialFontFamily = PreviewFonts.editorial,
        )
    }
}
