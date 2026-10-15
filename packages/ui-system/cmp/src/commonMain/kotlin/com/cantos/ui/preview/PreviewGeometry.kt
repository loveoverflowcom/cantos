package com.cantos.ui.preview

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens
import kotlin.math.roundToInt

private fun breakpoint(name: String): Float = CantosTokens.number(name).toFloat()

/**
 * Preview-local geometry. Every value is dp, derived from a canonical breakpoint, scoped to this
 * diagnostic gallery and not a design token: shared components never read it. A value that a shared
 * component needs belongs in the token source instead.
 */
internal object PreviewGeometry {
    /** List pane beside the detail pane in an expanded window: a third of the medium breakpoint. */
    val masterPane: Dp = (breakpoint("breakpoint.medium") / 3).dp

    /** Name column beside a type or font token sample: the same third, so the two panes align. */
    val tokenSampleColumn: Dp = masterPane

    /** Height of a nested-theme sample root: half the medium breakpoint holds a heading, text and one action. */
    val sampleRoot: Dp = (breakpoint("breakpoint.medium") / 2).dp

    /** Height of the root nested inside [sampleRoot]: a quarter of the medium breakpoint holds one line. */
    val nestedSampleRoot: Dp = (breakpoint("breakpoint.medium") / 4).dp

    /** Width cap that makes Grid and Container states reflow as if the window were half the medium breakpoint. */
    val narrowSampleMaxWidth: Dp = sampleRoot

    /** Longest bar drawn for a space or size token sample: three steps of the xl space. A longer value keeps its exact text and is marked clipped. */
    val tokenBarBound: Dp = (CantosTokens.number("space.xl") * 3).toFloat().dp

    /** Height of the inventory's two panes when the whole page scrolls: the medium breakpoint, tall enough for several rows and a detail. */
    val twoPaneBody: Dp = breakpoint("breakpoint.medium").dp
}

/**
 * The gallery page scrolls as a whole in a narrow window and whenever text is enlarged, because its header
 * and controls then outgrow the window. The scale that matters is the effective one (host font scale times
 * the preview multiplier), so a host with large text behaves like the 2x chip. Only an expanded window at
 * normal text size keeps a fixed page whose panes scroll on their own.
 */
internal fun scrollsWholePage(expanded: Boolean, effectiveFontScale: Float): Boolean = !expanded || effectiveFontScale > 1f

/**
 * Width constraints, in dp, for evidence runs. 320 and 390 are the small and typical phone widths the
 * design v3 issue names; 600 and 840 are the canonical medium and expanded breakpoints; Wide is the
 * desktop window's default width. They are layout constraints, not device models, pixels or tokens.
 */
enum class ViewportPreset(val widthDp: Float) {
    Small(320f),
    Typical(390f),
    Medium(breakpoint("breakpoint.medium")),
    Expanded(breakpoint("breakpoint.expanded")),
    Wide(breakpoint("breakpoint.expanded") + breakpoint("breakpoint.medium") / 2);

    val label: String get() = "${widthDp.roundToInt()} dp"
}
