package com.cantos.ui

import androidx.compose.foundation.BorderStroke
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
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.Dp

/**
 * Focus decorates the native node without changing its layout or hit area.
 * [restingBorder] is the node's own stroke inside its bounds (for example a field's error outline).
 * It is painted only while the node is unfocused: the focus ring replaces it, so a focused invalid
 * field shows one indicator instead of two concentric strokes.
 */
internal fun Modifier.cantosFocus(shapeRole: String = "shape.control", restingBorder: BorderStroke? = null): Modifier = composed {
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
            } else if (restingBorder != null) {
                val stroke = restingBorder.width.toPx()
                val corner = (radius.toPx() - stroke / 2).coerceAtLeast(0f)
                drawRoundRect(
                    brush = restingBorder.brush,
                    topLeft = Offset(stroke / 2, stroke / 2),
                    size = Size(size.width - stroke, size.height - stroke),
                    cornerRadius = CornerRadius(corner, corner),
                    style = Stroke(stroke),
                )
            }
        }
}

/**
 * One state perimeter, drawn inside the node's own bounds (Fields v3/v4): idle paints nothing, an
 * invalid field paints one `size.border`, focus (or an [active] open menu) paints two, in the
 * error colour while invalid. It never leaves the bounds, so it cannot be clipped, double with the
 * node's fill or move neighbouring layout; the radius is capped by Skia to a stadium.
 * Only the node itself counts as focused: a trailing action inside the shell keeps its own ring.
 * Standard controls keep [cantosFocus]; this is local to the fields and menus that opt in.
 */
internal fun Modifier.cantosInsetPerimeter(radius: Dp, invalid: Boolean = false, active: Boolean = false): Modifier = composed {
    var focused by remember { mutableStateOf(false) }
    val rest = tokenDp("size.border")
    val strong = rest * InsetPerimeterActiveFactor
    val error = tokenColor("color.error")
    val focus = tokenColor("color.focus")
    onFocusChanged { focused = it.isFocused }
        .drawWithContent {
            drawContent()
            val engaged = focused || active
            if (engaged || invalid) {
                val stroke = (if (engaged) strong else rest).toPx()
                val corner = (radius.toPx() - stroke / 2).coerceAtLeast(0f)
                drawRoundRect(
                    color = if (invalid) error else focus,
                    topLeft = Offset(stroke / 2, stroke / 2),
                    size = Size(size.width - stroke, size.height - stroke),
                    cornerRadius = CornerRadius(corner, corner),
                    style = Stroke(stroke),
                )
            }
        }
}

/** Focus and an open menu are twice the resting stroke, as in the Web `--cantos-web-stroke-active`. */
internal const val InsetPerimeterActiveFactor: Int = 2

internal fun Modifier.actionState(state: ButtonState, description: String? = null): Modifier = semantics {
    val descriptions = listOfNotNull(description, state.reason()).filter { it.isNotBlank() }.joinToString(". ")
    if (descriptions.isNotBlank()) stateDescription = descriptions
}
