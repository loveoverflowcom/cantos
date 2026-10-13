// Generated from Cantos UI tokens v0.2.2 sha256:6cc6c44890b5c41b73dad22e927c0962b95164f8421bf94f5645578ece6f8a1d
// by python3 scripts/ui_tokens.py. Do not edit.
package com.cantos.ui.generated

data class TokenValue(
    val name: String, val kind: String,
    val light: String, val dark: String,
    val compact: String?, val comfortable: String?, val aliasOf: String?
)

object CantosTokens {
    const val version: String = "0.2.2"
    val entries: List<TokenValue> = listOf(
        TokenValue("breakpoint.expanded", "dimension", "840", "840", null, null, null),
        TokenValue("breakpoint.medium", "dimension", "600", "600", null, null, null),
        TokenValue("color.accent", "color", "#886638", "#d9b57d", null, null, null),
        TokenValue("color.background", "color", "#f7f3eb", "#211d25", null, null, null),
        TokenValue("color.error", "color", "#a33436", "#ffb3a9", null, null, null),
        TokenValue("color.errorContainer", "color", "#fce3dd", "#4d2627", null, null, null),
        TokenValue("color.focus", "color", "#744b98", "#e0b8fc", null, null, null),
        TokenValue("color.onPlayerSurface", "color", "#f7eef9", "#f7eef9", null, null, null),
        TokenValue("color.onPrimary", "color", "#ffffff", "#352044", null, null, null),
        TokenValue("color.onPrimaryContainer", "color", "#332340", "#f0defe", null, null, null),
        TokenValue("color.onSurface", "color", "#292331", "#f1e9f2", null, null, null),
        TokenValue("color.onSurfaceVariant", "color", "#6c616f", "#c4b8ca", null, null, null),
        TokenValue("color.outline", "color", "#837587", "#9d8ba4", null, null, null),
        TokenValue("color.outlineVariant", "color", "#dcd4dd", "#514658", null, null, null),
        TokenValue("color.playerAccent", "color", "#dec6f0", "#dec6f0", null, null, null),
        TokenValue("color.playerSurface", "color", "#34283f", "#34283f", null, null, null),
        TokenValue("color.primary", "color", "#675080", "#d9bdf1", null, null, null),
        TokenValue("color.primaryContainer", "color", "#e6d9f1", "#513b63", null, null, null),
        TokenValue("color.success", "color", "#375e4b", "#b7d6ba", null, null, null),
        TokenValue("color.successContainer", "color", "#dfeadf", "#293f33", null, null, null),
        TokenValue("color.surface", "color", "#fffcf7", "#2c2631", null, null, null),
        TokenValue("color.surfaceContainer", "color", "#eee8f1", "#372f3e", null, null, null),
        TokenValue("color.surfaceContainerHigh", "color", "#e7dfea", "#463a4e", null, null, null),
        TokenValue("color.warning", "color", "#7a521c", "#f5d397", null, null, null),
        TokenValue("color.warningContainer", "color", "#f6e7ca", "#483b26", null, null, null),
        TokenValue("elevation.base", "shadow", "{\"blur\":0,\"color\":\"#000000\",\"opacity\":0,\"spread\":0,\"x\":0,\"y\":0}", "{\"blur\":0,\"color\":\"#000000\",\"opacity\":0,\"spread\":0,\"x\":0,\"y\":0}", null, null, null),
        TokenValue("elevation.overlay", "shadow", "{\"blur\":24,\"color\":\"#000000\",\"opacity\":0.16,\"spread\":0,\"x\":0,\"y\":8}", "{\"blur\":24,\"color\":\"#000000\",\"opacity\":0.16,\"spread\":0,\"x\":0,\"y\":8}", null, null, null),
        TokenValue("motion.effects.easing", "cubicBezier", "[0.2,0,0,1]", "[0.2,0,0,1]", null, null, null),
        TokenValue("motion.emphasis.duration", "duration", "260", "260", null, null, null),
        TokenValue("motion.feedback.duration", "duration", "140", "140", null, null, null),
        TokenValue("motion.navigation.duration", "duration", "260", "260", null, null, null),
        TokenValue("shape.container", "dimension", "16", "16", null, null, null),
        TokenValue("shape.control", "dimension", "8", "8", null, null, null),
        TokenValue("shape.detail", "dimension", "4", "4", null, null, null),
        TokenValue("shape.field", "dimension", "12", "12", null, null, "shape.reading"),
        TokenValue("shape.overlay", "dimension", "20", "20", null, null, null),
        TokenValue("shape.pill", "dimension", "999", "999", null, null, null),
        TokenValue("shape.reading", "dimension", "12", "12", null, null, null),
        TokenValue("shape.thumbnail", "dimension", "10", "10", null, null, null),
        TokenValue("size.border", "dimension", "1", "1", null, null, null),
        TokenValue("size.control", "dimension", "44", "44", "44", "48", null),
        TokenValue("size.focusOffset", "dimension", "4", "4", null, null, null),
        TokenValue("size.focusWidth", "dimension", "3", "3", null, null, null),
        TokenValue("size.icon", "dimension", "24", "24", null, null, null),
        TokenValue("size.iconCompact", "dimension", "20", "20", null, null, null),
        TokenValue("space.controlInline", "dimension", "12", "12", "12", "16", null),
        TokenValue("space.inset", "dimension", "12", "12", null, null, null),
        TokenValue("space.lg", "dimension", "24", "24", null, null, null),
        TokenValue("space.md", "dimension", "16", "16", null, null, null),
        TokenValue("space.section", "dimension", "20", "20", null, null, null),
        TokenValue("space.sm", "dimension", "8", "8", null, null, null),
        TokenValue("space.xl", "dimension", "32", "32", null, null, null),
        TokenValue("space.xs", "dimension", "4", "4", null, null, null),
        TokenValue("space.xxl", "dimension", "36", "36", null, null, null),
        TokenValue("state.disabledOpacity", "number", "0.46", "0.46", null, null, null),
        TokenValue("state.focusOpacity", "number", "0.12", "0.12", null, null, null),
        TokenValue("state.hoverOpacity", "number", "0.08", "0.08", null, null, null),
        TokenValue("state.pressedOpacity", "number", "0.12", "0.12", null, null, null),
        TokenValue("type.body.lineHeight", "number", "1.55", "1.55", null, null, null),
        TokenValue("type.body.size", "dimension", "14", "14", null, null, null),
        TokenValue("type.body.weight", "fontWeight", "400", "400", null, null, null),
        TokenValue("type.editor.lineHeight", "number", "1.9", "1.9", null, null, null),
        TokenValue("type.editor.size", "dimension", "14", "14", null, null, null),
        TokenValue("type.editor.weight", "fontWeight", "400", "400", null, null, null),
        TokenValue("type.family.editorial", "fontFamily", "\"Noto Serif\", Georgia, serif", "\"Noto Serif\", Georgia, serif", null, null, null),
        TokenValue("type.family.ui", "fontFamily", "\"Noto Sans\", system-ui, sans-serif", "\"Noto Sans\", system-ui, sans-serif", null, null, null),
        TokenValue("type.headline.lineHeight", "number", "1.4", "1.4", null, null, null),
        TokenValue("type.headline.size", "dimension", "28", "28", null, null, null),
        TokenValue("type.headline.weight", "fontWeight", "500", "500", null, null, null),
        TokenValue("type.label.lineHeight", "number", "1.5", "1.5", null, null, null),
        TokenValue("type.label.size", "dimension", "14", "14", null, null, null),
        TokenValue("type.label.weight", "fontWeight", "600", "600", null, null, null),
        TokenValue("type.metadata.lineHeight", "number", "1.5", "1.5", null, null, null),
        TokenValue("type.metadata.size", "dimension", "12", "12", null, null, null),
        TokenValue("type.metadata.weight", "fontWeight", "400", "400", null, null, null),
        TokenValue("type.title.lineHeight", "number", "1.4", "1.4", null, null, null),
        TokenValue("type.title.size", "dimension", "20", "20", null, null, null),
        TokenValue("type.title.weight", "fontWeight", "600", "600", null, null, null),
    )
    val names: List<String> = entries.map { it.name }
    private val byName = entries.associateBy { it.name }
    private val reducedRoles = setOf("motion.emphasis.duration", "motion.feedback.duration", "motion.navigation.duration")

    fun raw(name: String, theme: String = "light", density: String = "compact"): String {
        require(theme == "light" || theme == "dark") { "Unknown theme: $theme" }
        require(density == "compact" || density == "comfortable") { "Unknown density: $density" }
        val token = requireNotNull(byName[name]) { "Unknown token: $name" }
        val override = if (density == "comfortable") token.comfortable else token.compact
        return override ?: if (theme == "dark") token.dark else token.light
    }

    fun color(name: String, theme: String): String {
        require(byName[name]?.kind == "color") { "Not a color token: $name" }
        return raw(name, theme)
    }

    fun number(name: String, density: String = "compact", reducedMotion: Boolean = false): Double {
        require(byName[name]?.kind in setOf("dimension", "number", "fontWeight", "duration")) {
            "Not a numeric token: $name"
        }
        return if (reducedMotion && name in reducedRoles) 0.0 else raw(name, density = density).toDouble()
    }

    fun text(name: String): String = raw(name)
}
