package com.cantos.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilterChip as MaterialFilterChip
import androidx.compose.material3.IconButton as MaterialIconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics

@Composable
fun Button(
    label: String,
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    variant: ButtonVariant = ButtonVariant.Primary,
    controlModifier: Modifier = Modifier,
) {
    Button(
        name = AccessibleName.of(label),
        onPress = onPress,
        modifier = modifier,
        state = state,
        variant = variant,
        controlModifier = controlModifier,
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
    content: @Composable RowScope.() -> Unit,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        ButtonControl(onPress, controlModifier, state, variant, name = name, content = content)
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
    content: @Composable RowScope.() -> Unit,
) {
    val background = when (variant) {
        ButtonVariant.Primary -> tokenColor("color.primary")
        ButtonVariant.Tonal -> tokenColor("color.primaryContainer")
        ButtonVariant.Danger -> tokenColor("color.errorContainer")
        ButtonVariant.Secondary, ButtonVariant.Text -> Color.Transparent
    }
    val foreground = when (variant) {
        ButtonVariant.Primary -> tokenColor("color.onPrimary")
        ButtonVariant.Tonal -> tokenColor("color.onPrimaryContainer")
        ButtonVariant.Danger -> tokenColor("color.error")
        ButtonVariant.Secondary, ButtonVariant.Text -> tokenColor("color.primary")
    }
    androidx.compose.material3.Button(
        onClick = { if (state.canPress()) onPress() },
        enabled = state.canPress(),
        modifier = modifier.heightIn(min = tokenDp("size.control")).cantosFocus().actionState(state, description)
            .semantics { name?.let { contentDescription = it.text } },
        shape = RoundedCornerShape(tokenDp("shape.control")),
        border = if (variant == ButtonVariant.Secondary) BorderStroke(
            tokenDp("size.border"), tokenColor("color.outline"),
        ) else null,
        colors = ButtonDefaults.buttonColors(
            containerColor = background,
            contentColor = foreground,
            disabledContainerColor = tokenColor("color.surfaceContainer"),
            disabledContentColor = tokenColor("color.onSurfaceVariant"),
        ),
        contentPadding = PaddingValues(horizontal = tokenDp("space.controlInline")),
    ) {
        if (state is ButtonState.Loading) {
            Icon("expand_more")
            Spacer(Modifier.size(tokenDp("space.sm")))
        }
        content()
    }
}

@Composable
fun IconButton(
    name: AccessibleName,
    icon: String,
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    controlModifier: Modifier = Modifier,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        MaterialIconButton(
            onClick = { if (state.canPress()) onPress() },
            enabled = state.canPress(),
            modifier = controlModifier.size(tokenDp("size.control")).cantosFocus().actionState(state)
                .semantics { contentDescription = name.text },
            colors = IconButtonDefaults.iconButtonColors(
                contentColor = tokenColor("color.onSurfaceVariant"),
                disabledContentColor = tokenColor("color.onSurfaceVariant"),
            ),
        ) { Icon(icon) }
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
