package com.cantos.ui

/**
 * Registry glyph of one theme mode for the topbar theme action. System is its own split-disc sun
 * glyph, never two smaller icons, so the three modes share one stroke family and one 24-unit
 * viewbox. Control height is `size.control`/`size.avatarControl` (48) and the shape is
 * [CantosShapes.reading] for the theme action and [CantosShapes.full] for the account avatar;
 * a consumer composes them with the Web recipe `cui-topbar-actions` as its parity oracle.
 */
fun themeModeIconName(mode: ThemeMode): String = when (mode) {
    ThemeMode.Dark -> "dark_mode"
    ThemeMode.Light -> "light_mode"
    ThemeMode.System -> "theme_system"
}
