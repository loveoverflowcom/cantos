package com.cantos.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip as MaterialFilterChip
import androidx.compose.material3.IconButton as MaterialIconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.clearAndSetSemantics
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
    content: @Composable RowScope.() -> Unit,
) {
    // The closed registry rejects unknown names even when pending hides the glyph.
    icon?.let { CantosIcons.paths(it) }
    val interactions = interactionSource ?: remember { MutableInteractionSource() }
    val auth = variant.isAuth()
    val shape = if (auth) authActionShape(interactions) else CantosShapes.control
    val minimumHeight = tokenDp(minimumHeightRole ?: when (variant) {
        ButtonVariant.AuthPrimary -> "size.authPrimary"
        ButtonVariant.AuthSecondary, ButtonVariant.AuthOutlined -> "size.authSecondary"
        else -> "size.control"
    })
    val background = when (variant) {
        ButtonVariant.Primary, ButtonVariant.AuthPrimary -> tokenColor("color.primary")
        ButtonVariant.Tonal, ButtonVariant.AuthSecondary -> tokenColor("color.primaryContainer")
        ButtonVariant.Danger -> tokenColor("color.errorContainer")
        ButtonVariant.Secondary, ButtonVariant.Text, ButtonVariant.AuthOutlined -> Color.Transparent
    }
    val foreground = when (variant) {
        ButtonVariant.Primary, ButtonVariant.AuthPrimary -> tokenColor("color.onPrimary")
        ButtonVariant.Tonal, ButtonVariant.AuthSecondary -> tokenColor("color.onPrimaryContainer")
        ButtonVariant.Danger -> tokenColor("color.error")
        ButtonVariant.Secondary, ButtonVariant.Text, ButtonVariant.AuthOutlined -> tokenColor("color.primary")
    }
    androidx.compose.material3.Button(
        onClick = { if (state.canPress()) onPress() },
        enabled = state.canPress(),
        modifier = modifier.heightIn(min = minimumHeight).cantosFocus(if (auth) "shape.pill" else "shape.control").actionState(state, description)
            .semantics { name?.let { contentDescription = it.text } },
        shape = shape,
        interactionSource = interactions,
        border = if (variant == ButtonVariant.Secondary || variant == ButtonVariant.AuthOutlined) BorderStroke(
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
            ActionIcon(icon, state, variant)
            Spacer(Modifier.size(tokenDp("space.sm")))
        }
        content()
    }
}

/** Leading icons are decorative; the native action keeps the sole localized name.
 * Only AuthPrimary uses the contrast-paired circle, including its blocked tonal state.
 */
@Composable
private fun ActionIcon(icon: String?, state: ButtonState, variant: ButtonVariant) {
    if (variant == ButtonVariant.AuthPrimary) {
        val circleRole = if (state.canPress()) "color.onPrimary" else "color.onSurfaceVariant"
        val glyphRole = if (state.canPress()) "color.primary" else "color.surfaceContainer"
        Box(
            Modifier.size(tokenDp("size.authIcon")).background(tokenColor(circleRole), CircleShape),
            contentAlignment = Alignment.Center,
        ) { ActionGlyph(icon, state, tokenColor(glyphRole)) }
    } else {
        ActionGlyph(icon, state, androidx.compose.material3.LocalContentColor.current)
    }
}

/** Stationary indeterminate ring; the action's localized state owns the pending announcement.
 * The fixed arc is decorative, never a completion percentage or a disclosure/check glyph.
 */
@Composable
private fun ActionGlyph(icon: String?, state: ButtonState, tint: Color) {
    if (state is ButtonState.Loading) {
        val stroke = tokenDp("size.focusWidth")
        Canvas(Modifier.size(tokenDp("size.icon")).clearAndSetSemantics { }) {
            val width = stroke.toPx()
            drawArc(tint, -90f, 270f, false,
                topLeft = androidx.compose.ui.geometry.Offset(width / 2, width / 2),
                size = androidx.compose.ui.geometry.Size(size.width - width, size.height - width),
                style = Stroke(width))
        }
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

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun IconButton(
    name: AccessibleName,
    icon: String,
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    controlModifier: Modifier = Modifier,
) {
    CantosIcons.paths(icon)
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        TooltipBox(
            positionProvider = TooltipDefaults.rememberPlainTooltipPositionProvider(),
            tooltip = { PlainTooltip { Text(name.text) } },
            state = rememberTooltipState(),
        ) {
        MaterialIconButton(
            onClick = { if (state.canPress()) onPress() },
            enabled = state.canPress(),
            modifier = controlModifier.size(tokenDp("size.control")).cantosFocus().actionState(state)
                .semantics { contentDescription = name.text },
            colors = IconButtonDefaults.iconButtonColors(
                contentColor = tokenColor("color.onSurfaceVariant"),
                disabledContentColor = tokenColor("color.onSurfaceVariant"),
            ),
        ) { ActionGlyph(icon, state, androidx.compose.material3.LocalContentColor.current) }
        }
        state.reason()?.let { FieldHelp(it) }
    }
}

@Composable
fun FilterChip(
    label: String,
    selected: Boolean,
    onSelect: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    controlModifier: Modifier = Modifier,
) {
    Column(modifier) {
        MaterialFilterChip(
            selected = selected,
            onClick = { if (state.canPress()) onSelect(!selected) },
            enabled = state.canPress(),
            label = { Text(label) },
            leadingIcon = if (selected) ({ Icon("check") }) else null,
            modifier = controlModifier.heightIn(min = tokenDp("size.control")).cantosFocus().actionState(state),
        )
        state.reason()?.let { FieldHelp(it) }
    }
}
