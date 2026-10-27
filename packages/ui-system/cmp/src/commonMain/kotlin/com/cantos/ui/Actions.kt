package com.cantos.ui

import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip as MaterialFilterChip
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.IconButton as MaterialIconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.Density as DisplayDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import com.cantos.ui.generated.CantosIcons

@Composable
fun Button(
    label: String,
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    variant: ButtonVariant = ButtonVariant.Primary,
    controlModifier: Modifier = Modifier,
    interactionSource: MutableInteractionSource? = null,
    icon: String? = null,
) {
    Button(
        name = AccessibleName.of(label),
        onPress = onPress,
        modifier = modifier,
        state = state,
        variant = variant,
        controlModifier = controlModifier,
        interactionSource = interactionSource,
        icon = icon,
    ) {
        Text(label)
    }
}

/** Localized name and a content slot; state and callbacks stay controlled by the caller.
 * modifier owns the whole field; controlModifier owns only the native button/focus node.
 */
@Composable
fun Button(
    name: AccessibleName,
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    variant: ButtonVariant = ButtonVariant.Primary,
    controlModifier: Modifier = Modifier,
    interactionSource: MutableInteractionSource? = null,
    icon: String? = null,
    content: @Composable RowScope.() -> Unit,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        ButtonControl(onPress, controlModifier, state, variant, name = name, interactionSource = interactionSource, icon = icon, content = content)
        state.reason()?.let { FieldHelp(it) }
    }
}

/** Control semantics belong on the native interactive node, separate from adjacent help. */
@Composable
internal fun ButtonControl(
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    variant: ButtonVariant = ButtonVariant.Primary,
    name: AccessibleName? = null,
    description: String? = null,
    interactionSource: MutableInteractionSource? = null,
    minimumHeightRole: String? = null,
    contentPadding: PaddingValues? = null,
    icon: String? = null,
    shape: Shape? = null,
    insetPerimeter: Boolean = false,
    invalid: Boolean = false,
    active: Boolean = false,
    containerColor: Color? = null,
    content: @Composable RowScope.() -> Unit,
) {
    // The closed registry rejects unknown names even when pending hides the glyph.
    icon?.let { CantosIcons.paths(it) }
    val interactions = interactionSource ?: remember { MutableInteractionSource() }
    val auth = variant.isAuth()
    val resolvedShape = shape ?: if (auth) authActionShape(interactions) else CantosShapes.control
    val pair = variant.joinsContainerPair(LocalAuthActionScope.current)
    val minimumHeight = tokenDp(minimumHeightRole ?: when (variant) {
        ButtonVariant.AuthPrimary -> "size.authPrimary"
        ButtonVariant.AuthOutlined -> if (pair) "size.authPrimary" else "size.authSecondary"
        ButtonVariant.AuthSecondary -> "size.authSecondary"
        else -> "size.control"
    })
    val background = containerColor ?: buttonContainerColor(variant)
    val foreground = buttonContentColor(variant)
    val tonal = LocalAuthActionScope.current.isTonal()
    val inset = insetPerimeter || variant.focusIsInset(LocalUiPreferences.current.expression)
    androidx.compose.material3.Button(
        onClick = { if (state.canPress()) onPress() },
        enabled = state.canPress(),
        modifier = modifier.heightIn(min = minimumHeight)
            .then(if (tonal) Modifier.cantosFocusFill(resolvedShape, foreground)
                else if (inset) Modifier.cantosInsetPerimeter(resolvedShape, invalid = invalid, active = active,
                ring = if (variant.ringReadsFromContent()) foreground else null,
                inward = if (variant.ringReadsFromContent()) tokenDp("size.border") * InsetPerimeterActiveFactor else Dp(0f))
                else Modifier.cantosFocus(if (auth) "shape.pill" else "shape.control"))
            .actionState(state, description)
            .semantics { name?.let { contentDescription = it.text } },
        shape = resolvedShape,
        interactionSource = interactions,
        border = if (tonal) null else if (pair) BorderStroke(tokenDp("size.authOutline"), tokenColor("color.onPrimaryContainer"))
            else if (variant == ButtonVariant.Secondary || variant == ButtonVariant.AuthOutlined) BorderStroke(
                tokenDp(if (auth) "size.authOutline" else "size.border"), tokenColor("color.outline"),
            ) else null,
        colors = ButtonDefaults.buttonColors(
            containerColor = background,
            contentColor = foreground,
            disabledContainerColor = tokenColor("color.surfaceContainer"),
            disabledContentColor = tokenColor("color.onSurfaceVariant"),
        ),
        contentPadding = contentPadding ?: if (auth) PaddingValues(horizontal = tokenDp("space.lg"), vertical = tokenDp("space.inset"))
            else PaddingValues(horizontal = tokenDp("space.controlInline")),
    ) {
        if (icon != null || state is ButtonState.Loading) {
            ActionIcon(icon, state)
            Spacer(Modifier.size(tokenDp("space.sm")))
        }
        content()
    }
}

/**
 * A painted action keeps its focus perimeter inside its own surface: every auth action (their only
 * consumer is Theatre) and every action under the Theatre expression. The Standard expression keeps
 * the outset ring.
 */
internal fun ButtonVariant.focusIsInset(expression: CantosExpression): Boolean =
    isAuth() || expression == CantosExpression.Theatre

/**
 * A primary fill is the focus hue itself (`color.primary` against `color.focus` is about 1.1:1 in
 * both themes), so an inset ring in `color.focus` would vanish on it; it reads from the content colour.
 * That colour is also the page's, so such a ring sits one active stroke inside the fill, never on its edge.
 */
internal fun ButtonVariant.ringReadsFromContent(): Boolean =
    this == ButtonVariant.Primary || this == ButtonVariant.AuthPrimary

/**
 * Under the Theatre expression `primaryContainer` is the ink of selection (chips, the current
 * destination), so a tonal action reads from the tonal surface roles: pale lilac with ink text.
 */
@Composable
private fun theatreTonal(variant: ButtonVariant): Boolean =
    LocalUiPreferences.current.expression == CantosExpression.Theatre &&
        (variant == ButtonVariant.Tonal || variant == ButtonVariant.AuthSecondary)

@Composable
internal fun buttonContainerColor(variant: ButtonVariant): Color = when (variant) {
    ButtonVariant.AuthPrimary -> tokenColor(
        if (variant.joinsContainerPair(LocalAuthActionScope.current)) "color.onPrimaryContainer" else "color.primary")
    ButtonVariant.Primary -> tokenColor("color.primary")
    ButtonVariant.Tonal, ButtonVariant.AuthSecondary ->
        tokenColor(if (theatreTonal(variant)) "color.surfaceContainerHigh" else "color.primaryContainer")
    ButtonVariant.Danger -> tokenColor("color.errorContainer")
    // The outlined auth action has no outline in the tonal recipe: it is the quiet tonal container.
    ButtonVariant.AuthOutlined ->
        if (LocalAuthActionScope.current.isTonal()) tokenColor("color.surfaceContainerHigh") else Color.Transparent
    ButtonVariant.Secondary, ButtonVariant.Text -> Color.Transparent
}

@Composable
internal fun buttonContentColor(variant: ButtonVariant): Color = when (variant) {
    ButtonVariant.AuthPrimary -> tokenColor(
        if (variant.joinsContainerPair(LocalAuthActionScope.current)) "color.primaryContainer" else "color.onPrimary")
    ButtonVariant.Primary -> tokenColor("color.onPrimary")
    ButtonVariant.AuthOutlined -> tokenColor(
        if (variant.joinsContainerPair(LocalAuthActionScope.current)) "color.onPrimaryContainer" else "color.primary")
    ButtonVariant.Tonal, ButtonVariant.AuthSecondary ->
        tokenColor(if (theatreTonal(variant)) "color.onSurface" else "color.onPrimaryContainer")
    ButtonVariant.Danger -> tokenColor("color.error")
    ButtonVariant.Secondary, ButtonVariant.Text -> tokenColor("color.primary")
}

/** The 23 October anatomy centers the glyph and label together, without an icon bubble. */
@Composable
private fun ActionIcon(icon: String?, state: ButtonState) {
    ActionGlyph(icon, state, androidx.compose.material3.LocalContentColor.current)
}

/** The pending glyph of an action; the action's localized state owns the pending announcement. */
@Composable
private fun ActionGlyph(icon: String?, state: ButtonState, tint: Color) {
    if (state is ButtonState.Loading) {
        LoadingIndicator(tint = tint)
    } else if (icon != null) {
        Icon(icon, tint = tint)
    }
}

/** Contextual Back only dispatches the host intent. It never navigates or signs out itself.
 * iconOnly keeps a full localized name; a visible label reflows at large text sizes.
 */
@Composable
fun BackButton(
    name: AccessibleName,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    iconOnly: Boolean = true,
    controlModifier: Modifier = Modifier,
    interactionSource: MutableInteractionSource? = null,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        ButtonControl(
            onPress = onBack,
            modifier = controlModifier.widthIn(min = tokenDp("size.backControl")),
            state = state,
            variant = ButtonVariant.AuthSecondary,
            name = name,
            interactionSource = interactionSource,
            minimumHeightRole = "size.backControl",
            contentPadding = PaddingValues(
                horizontal = tokenDp(if (iconOnly) "space.inset" else "space.lg"),
                vertical = tokenDp("space.inset"),
            ),
        ) {
            Icon("arrow_back")
            if (!iconOnly) {
                Spacer(Modifier.size(tokenDp("space.sm")))
                Text(name.text)
            }
        }
        state.reason()?.let { FieldHelp(it) }
    }
}

/**
 * Icon-only native action; [name] is its localized accessible name and tooltip. [variant] Text keeps
 * the plain muted glyph every existing caller has; Tonal/Primary fill a 48-unit container, so a
 * Theatre theme button or a docked search action is this control, never a clickable container.
 * [circular] makes the container a circle (a docked action) instead of the reading-radius square.
 */
@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun IconButton(
    name: AccessibleName,
    icon: String,
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    controlModifier: Modifier = Modifier,
    variant: ButtonVariant = ButtonVariant.Text,
    circular: Boolean = false,
) {
    CantosIcons.paths(icon)
    val filled = variant != ButtonVariant.Text
    val shape = if (circular) CantosShapes.full else CantosShapes.reading
    val tooltipState = rememberTooltipState()
    val coroutineScope = rememberCoroutineScope()
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        TooltipBox(
            positionProvider = TooltipDefaults.rememberPlainTooltipPositionProvider(),
            tooltip = { PlainTooltip { Text(name.text) } },
            state = tooltipState,
        ) {
        // The plain glyph button is Material's circle whatever [circular] says; a filled one is [shape].
        val tonalFocus = LocalAuthActionScope.current.isTonal()
        val focus = if (tonalFocus) {
            Modifier.cantosFocusFill(if (filled) shape else CantosShapes.full, buttonContentColor(variant))
        } else if (variant.focusIsInset(LocalUiPreferences.current.expression)) {
            Modifier.cantosInsetPerimeter(if (filled) shape else CantosShapes.full,
                ring = if (variant.ringReadsFromContent()) buttonContentColor(variant) else null,
                inward = if (variant.ringReadsFromContent()) tokenDp("size.border") * InsetPerimeterActiveFactor else Dp(0f))
        } else Modifier.cantosFocus(if (circular) "shape.pill" else "shape.control")
        val doubleTapGesture = if (state.canPress()) {
            Modifier.pointerInput(name.text) {
                detectTapGestures(
                    onDoubleTap = { coroutineScope.launch { tooltipState.show() } },
                    onTap = { onPress() },
                )
            }
        } else Modifier
        val iconModifier = controlModifier.size(tokenDp("size.control")).then(focus).actionState(state)
            .semantics {
                contentDescription = name.text
                customActions = listOf(CustomAccessibilityAction(name.text) {
                    coroutineScope.launch { tooltipState.show() }
                    true
                })
            }
            .then(doubleTapGesture)
        if (filled) {
            FilledIconButton(
                onClick = { if (state.canPress()) onPress() },
                enabled = state.canPress(),
                shape = shape,
                modifier = iconModifier,
                colors = IconButtonDefaults.filledIconButtonColors(
                    containerColor = buttonContainerColor(variant),
                    contentColor = buttonContentColor(variant),
                    disabledContainerColor = tokenColor("color.surfaceContainer"),
                    disabledContentColor = tokenColor("color.onSurfaceVariant"),
                ),
            ) { ActionGlyph(icon, state, androidx.compose.material3.LocalContentColor.current) }
        } else {
            MaterialIconButton(
                onClick = { if (state.canPress()) onPress() },
                enabled = state.canPress(),
                modifier = iconModifier,
                colors = IconButtonDefaults.iconButtonColors(
                    contentColor = tokenColor("color.onSurfaceVariant"),
                    disabledContentColor = tokenColor("color.onSurfaceVariant"),
                ),
            ) { ActionGlyph(icon, state, androidx.compose.material3.LocalContentColor.current) }
        }
        }
        state.reason()?.let { FieldHelp(it) }
    }
}

/**
 * Pure: the corner radius of a Theatre chip between its resting radius and the capsule of its
 * shortest side, at [fraction] 0 (unselected) to 1 (selected). Out-of-range input is clamped.
 */
internal fun chipRadius(shortestSidePx: Float, restingPx: Float, fraction: Float): Float {
    val capsule = shortestSidePx / 2f
    val resting = restingPx.coerceAtMost(capsule)
    return resting + (capsule - resting) * fraction.coerceIn(0f, 1f)
}

private class ChipShape(private val fraction: Float, private val restingRadius: Dp) : Shape {
    override fun createOutline(size: Size, layoutDirection: LayoutDirection, density: DisplayDensity): Outline {
        val radius = chipRadius(size.minDimension, with(density) { restingRadius.toPx() }, fraction)
        return Outline.Rounded(RoundRect(Rect(0f, 0f, size.width, size.height), CornerRadius(radius)))
    }
}

/**
 * Standard keeps the Material filter chip. The Theatre expression reads selection by check, tone and
 * shape: unselected is a tonal reading-radius chip centered on its label, selected an ink capsule
 * that gains the check. The chip keeps its left edge, top and 48-unit height, so the press point
 * stays inside it; only its width grows. The shape change uses the interruptible feedback motion
 * that reduced motion removes.
 */
@Composable
fun FilterChip(
    label: String,
    selected: Boolean,
    onSelect: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    controlModifier: Modifier = Modifier,
) {
    val theatre = LocalUiPreferences.current.expression == CantosExpression.Theatre
    Column(modifier) {
        if (theatre) {
            val spec = rememberMotionSpec(MotionIntent.Feedback, MotionProperty.Spatial)
            val fraction by animateFloatAsState(if (selected) 1f else 0f, spec.toAnimationSpec(), label = "chip-shape")
            val resting = tokenDp("shape.reading")
            val chipShape = remember(fraction, resting) { ChipShape(fraction, resting) }
            MaterialFilterChip(
                selected = selected,
                onClick = { if (state.canPress()) onSelect(!selected) },
                enabled = state.canPress(),
                label = { Text(label) },
                leadingIcon = if (selected) ({ Icon("check") }) else null,
                shape = chipShape,
                colors = FilterChipDefaults.filterChipColors(
                    containerColor = tokenColor("color.surfaceContainer"),
                    labelColor = tokenColor("color.onSurface"),
                    iconColor = tokenColor("color.onPrimaryContainer"),
                    selectedContainerColor = tokenColor("color.primaryContainer"),
                    selectedLabelColor = tokenColor("color.onPrimaryContainer"),
                    selectedLeadingIconColor = tokenColor("color.onPrimaryContainer"),
                    disabledContainerColor = tokenColor("color.surfaceContainer"),
                    disabledLabelColor = tokenColor("color.onSurfaceVariant"),
                    disabledLeadingIconColor = tokenColor("color.onSurfaceVariant"),
                    disabledSelectedContainerColor = tokenColor("color.surfaceContainer"),
                ),
                border = null,
                // The selected ink container is the label's inverse, so its ring reads from that.
                modifier = controlModifier.heightIn(min = tokenDp("size.control"))
                    .cantosInsetPerimeter(chipShape, ring = if (selected) tokenColor("color.onPrimaryContainer") else null,
                        inward = if (selected) tokenDp("size.border") * InsetPerimeterActiveFactor else Dp(0f))
                    .actionState(state),
            )
        } else {
            MaterialFilterChip(
                selected = selected,
                onClick = { if (state.canPress()) onSelect(!selected) },
                enabled = state.canPress(),
                label = { Text(label) },
                leadingIcon = if (selected) ({ Icon("check") }) else null,
                modifier = controlModifier.heightIn(min = tokenDp("size.control")).cantosFocus().actionState(state),
            )
        }
        state.reason()?.let { FieldHelp(it) }
    }
}
