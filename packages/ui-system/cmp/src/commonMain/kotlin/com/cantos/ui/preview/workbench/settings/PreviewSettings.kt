package com.cantos.ui.preview.workbench

import com.cantos.ui.Density
import com.cantos.ui.ThemeMode
import com.cantos.ui.UiPreferences
import com.cantos.ui.resolve

/** The preview content language, injected by the host; it never changes the operating system. */
enum class PreviewLocale { ViVn, En }

/**
 * Diagnostic layout constraints in dp, independent of the window, density and device identity.
 * Bounds keep an imported recipe inside the workbench's supported capture envelope.
 */
data class ConstraintFrame(val widthDp: Int = 390, val heightDp: Int = 844) {
    init {
        require(widthDp in WIDTH_RANGE && heightDp in HEIGHT_RANGE) { "Unsupported preview constraint frame" }
    }

    companion object {
        val WIDTH_RANGE: IntRange = 240..1600
        val HEIGHT_RANGE: IntRange = 240..2400
    }
}

/** Renderer values only: no port, controller, platform query or live adapter belongs here. */
data class PreviewSettings(
    val themeMode: ThemeMode = ThemeMode.Light,
    val density: Density = Density.Compact,
    val locale: PreviewLocale = PreviewLocale.ViVn,
    val fontScale: Float = 1f,
    val constraints: ConstraintFrame = ConstraintFrame(),
    val reducedMotion: Boolean = false,
) {
    init {
        require(fontScale in FONT_SCALES) { "Unsupported preview font scale" }
    }

    fun preferences(systemIsDark: Boolean): UiPreferences = UiPreferences(
        theme = themeMode.resolve(systemIsDark),
        density = density,
        reducedMotion = reducedMotion,
        textScale = fontScale,
    )

    companion object {
        val FONT_SCALES: List<Float> get() = listOf(1f, 1.3f, 2f)
    }
}
