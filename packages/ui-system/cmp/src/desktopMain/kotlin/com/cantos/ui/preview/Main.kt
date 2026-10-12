package com.cantos.ui.preview

import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import com.cantos.ui.generated.CantosTokens

fun main() = application {
    val medium = CantosTokens.number("breakpoint.medium").toFloat().dp
    val expanded = CantosTokens.number("breakpoint.expanded").toFloat().dp
    val gap = CantosTokens.number("space.lg").toFloat().dp
    // Desktop window geometry only. All semantic visual values still resolve shared tokens.
    val window = rememberWindowState(width = expanded + medium / 2, height = expanded)
    Window(onCloseRequest = ::exitApplication, state = window, title = "Cantos UI System · CMP") {
        PreviewApp { preset ->
            val width = when (preset) {
                "narrow" -> medium / 2 + gap * 3
                "medium" -> medium
                else -> expanded + medium / 2
            }
            window.size = DpSize(width, window.size.height)
        }
    }
}
