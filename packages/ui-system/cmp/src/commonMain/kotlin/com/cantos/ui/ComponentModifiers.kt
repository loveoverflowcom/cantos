package com.cantos.ui

import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.Dp

/**
 * Focus decorates the native node without changing its layout or hit area. The ring paints
 * `size.focusOffset` outside the bounds, so it belongs to controls with no painted surface of their
 * own (a choice row, a slider, a transparent link or tab) and to the Standard expression; a painted
 * Theatre shell uses [cantosInsetPerimeter] instead and never leaves its bounds.
 */
internal fun Modifier.cantosFocus(shapeRole: String = "shape.control"): Modifier = composed {
    var focused by remember { mutableStateOf(false) }
    val width = tokenDp("size.focusWidth")
    val offset = tokenDp("size.focusOffset")
    val radius = tokenDp(shapeRole)
    val color = tokenColor("color.focus")
    onFocusChanged { focused = it.isFocused }
        .drawWithContent {
            drawContent()
            if (focused) {
                val outset = offset.toPx() + width.toPx() / 2
                val corner = radius.toPx() + outset
                drawRoundRect(
                    color = color,
                    topLeft = Offset(-outset, -outset),
                    size = Size(size.width + 2 * outset, size.height + 2 * outset),
                    cornerRadius = CornerRadius(corner, corner),
                    style = Stroke(width.toPx()),
                )
            }
        }
}

/**
 * One state perimeter, drawn inside the node's own bounds (Fields v3/v4): idle paints nothing, an
 * invalid field paints one `size.border`, focus (or an [active] open menu) paints two, in the
 * error colour while invalid and otherwise in [ring]. It never leaves the bounds, so it cannot be
 * clipped, double with the node's fill or move neighbouring layout. The corner follows [shape] as
 * the node is measured, so an animated shape (the pressed auth capsule) keeps the ring on its edge.
 * [hoverStroke] adds the idle-hover cue of the docked search: one `size.border` in `color.outline`
 * while a pointer is over an unfocused, valid node (touch has no hover and paints nothing).
 * Only the node itself counts as focused: a trailing action inside the shell keeps its own ring.
 * [ring] defaults to `color.focus`; a fill that is itself that hue passes its content colour. That ring
 * would merge with the page at the node's edge, so it also passes [inward] to sit that far inside the fill.
 */
internal fun Modifier.cantosInsetPerimeter(
    shape: Shape,
    invalid: Boolean = false,
    active: Boolean = false,
    ring: Color? = null,
    inward: Dp = Dp(0f),
    hoverStroke: Boolean = false,
): Modifier = composed {
    var focused by remember { mutableStateOf(false) }
    val pointer = remember { MutableInteractionSource() }
    val hovered by pointer.collectIsHoveredAsState()
    val rest = tokenDp("size.border")
    val strong = rest * InsetPerimeterActiveFactor
    val error = tokenColor("color.error")
    val focus = ring ?: tokenColor("color.focus")
    val hover = tokenColor("color.outline")
    onFocusChanged { focused = it.isFocused }
        .then(if (hoverStroke) Modifier.hoverable(pointer) else Modifier)
        .drawWithContent {
            drawContent()
            val engaged = focused || active
            val hinted = hoverStroke && hovered && !engaged && !invalid
            if (engaged || invalid || hinted) {
                val stroke = (if (engaged) strong else rest).toPx()
                val radius = outlineCornerRadius(shape.createOutline(size, layoutDirection, this))
                val box = insetStrokeBox(size.width, size.height, stroke, radius, inward.toPx())
                drawRoundRect(
                    color = if (invalid) error else if (hinted) hover else focus,
                    topLeft = Offset(box.left, box.top),
                    size = Size(box.width, box.height),
                    cornerRadius = CornerRadius(box.corner, box.corner),
                    style = Stroke(stroke),
                )
            }
        }
}

/** A fixed radius: the field and menu shells whose shape is a plain rounded rectangle. */
internal fun Modifier.cantosInsetPerimeter(radius: Dp, invalid: Boolean = false, active: Boolean = false): Modifier =
    cantosInsetPerimeter(RoundedCornerShape(radius), invalid, active)

/** Where an inset stroke of [stroke] px is centred so its outer edge is [inward] px inside the node's edge. */
internal data class InsetStrokeBox(val left: Float, val top: Float, val width: Float, val height: Float, val corner: Float)

/**
 * Pure geometry: the stroke's centre line sits [inward] plus half a stroke inside the bounds, and its
 * corner is the node's [radius] (capped to a stadium) less that offset, so the outer edge follows the shape.
 */
internal fun insetStrokeBox(width: Float, height: Float, stroke: Float, radius: Float, inward: Float = 0f): InsetStrokeBox {
    val capped = radius.coerceIn(0f, minOf(width, height) / 2f)
    val offset = inward + stroke / 2
    return InsetStrokeBox(
        left = offset,
        top = offset,
        width = (width - 2 * offset).coerceAtLeast(0f),
        height = (height - 2 * offset).coerceAtLeast(0f),
        corner = (capped - offset).coerceAtLeast(0f),
    )
}

/**
 * The largest corner radius of a measured outline. Every Cantos control shape is a uniformly rounded
 * rectangle; a rectangle has none, and a free-form path falls back to square corners on its bounds.
 */
internal fun outlineCornerRadius(outline: Outline): Float = when (outline) {
    is Outline.Rounded -> with(outline.roundRect) {
        maxOf(topLeftCornerRadius.x, topRightCornerRadius.x, bottomLeftCornerRadius.x, bottomRightCornerRadius.x)
    }
    is Outline.Rectangle, is Outline.Generic -> 0f
}

/** Focus and an open menu are twice the resting stroke, as in the Web `--cantos-web-stroke-active`. */
internal const val InsetPerimeterActiveFactor: Int = 2

internal fun Modifier.actionState(state: ButtonState, description: String? = null): Modifier = semantics {
    val descriptions = listOfNotNull(description, state.reason()).filter { it.isNotBlank() }.joinToString(". ")
    if (descriptions.isNotBlank()) stateDescription = descriptions
}
