package com.cantos.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox as MaterialCheckbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip as MaterialFilterChip
import androidx.compose.material3.Icon as MaterialIcon
import androidx.compose.material3.IconButton as MaterialIconButton
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.TextField as MaterialTextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch as MaterialSwitch
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosIcons
import com.cantos.ui.generated.CantosTokens

private fun Modifier.cantosFocus(): Modifier = composed {
    var focused by remember { mutableStateOf(false) }
    val width = tokenDp("size.focusWidth")
    val color = tokenColor("color.focus")
    val shape = RoundedCornerShape(tokenDp("shape.control"))
    onFocusChanged { focused = it.isFocused }
        .then(if (focused) Modifier.border(width, color, shape) else Modifier)
}

private fun Modifier.actionState(state: ButtonState, description: String? = null): Modifier = semantics {
    val descriptions = listOfNotNull(description, state.reason()).joinToString(". ")
    if (descriptions.isNotBlank()) stateDescription = descriptions
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun UiRoot(
    modifier: Modifier = Modifier,
    preferences: UiPreferences = UiPreferences(),
    content: @Composable () -> Unit,
) {
    CantosTheme(preferences) {
        CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides tokenDp("size.control")) {
            RootContent(modifier, content)
        }
    }
}

@Composable
fun Button(
    label: String,
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    variant: ButtonVariant = ButtonVariant.Primary,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        ButtonControl(label, onPress, state = state, variant = variant)
        state.reason()?.let { FieldHelp(it) }
    }
}

/** Control semantics belong on the native interactive node, separate from adjacent help. */
@Composable
private fun ButtonControl(
    label: String,
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    variant: ButtonVariant = ButtonVariant.Primary,
    name: AccessibleName? = null,
    description: String? = null,
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
        Text(label)
    }
}

@Composable
fun IconButton(
    name: AccessibleName,
    icon: String,
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        MaterialIconButton(
            onClick = { if (state.canPress()) onPress() },
            enabled = state.canPress(),
            modifier = Modifier.size(tokenDp("size.control")).cantosFocus().actionState(state)
                .semantics { contentDescription = name.text },
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
) {
    Column(modifier) {
        MaterialFilterChip(
            selected = selected,
            onClick = { if (state.canPress()) onSelect(!selected) },
            enabled = state.canPress(),
            label = { Text(label) },
            leadingIcon = if (selected) ({ Icon("check") }) else null,
            modifier = Modifier.heightIn(min = tokenDp("size.control")).cantosFocus().actionState(state),
        )
        state.reason()?.let { FieldHelp(it) }
    }
}

/** Native navigation is caller-owned. No HTML anchor/target/download semantics are implied. */
@Composable
fun Link(label: String, href: String, onOpen: (String) -> Unit, modifier: Modifier = Modifier) {
    require(href.startsWith("https://") || href.startsWith("/") || href.startsWith("#")) {
        "Only a relative route, fragment or HTTPS URI is supported"
    }
    TextButton(onClick = { onOpen(href) }, modifier = modifier.cantosFocus()) { Text(label) }
}

@Composable
fun NavItem(label: String, selected: Boolean, onSelect: () -> Unit, modifier: Modifier = Modifier) {
    MaterialFilterChip(
        selected = selected,
        onClick = onSelect,
        label = { Text(label) },
        modifier = modifier.heightIn(min = tokenDp("size.control")).cantosFocus(),
    )
}

@Composable
fun NavTabs(labels: List<String>, selected: Int, onSelect: (Int) -> Unit, modifier: Modifier = Modifier) {
    require(labels.isNotEmpty() && selected in labels.indices)
    TabRow(selectedTabIndex = selected, modifier = modifier) {
        labels.forEachIndexed { index, label ->
            Tab(selected = index == selected, onClick = { onSelect(index) },
                modifier = Modifier.cantosFocus(), text = { Text(label) })
        }
    }
}

@Composable
private fun ControlledTextField(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier,
    state: FieldState,
    help: String?,
    errorText: String?,
    singleLine: Boolean,
    keyboardType: KeyboardType,
) {
    val description = listOfNotNull(help, errorText, state.reason()).joinToString(". ")
    val fieldContainer = tokenColor(if (state == FieldState.ReadOnly) "color.surfaceContainer" else "color.surfaceContainerHigh")
    val content = tokenColor("color.onSurface")
    val secondaryContent = tokenColor("color.onSurfaceVariant")
    val errorColor = tokenColor("color.error")
    MaterialTextField(
        value = value,
        onValueChange = { if (state.acceptsInput()) onValueChange(it) },
        modifier = modifier.fillMaxWidth().cantosFocus().semantics {
            if (description.isNotBlank()) stateDescription = description
            if (errorText != null) error(errorText)
        },
        label = { Text(label) },
        enabled = state.isEnabled(),
        readOnly = state == FieldState.ReadOnly,
        isError = errorText != null,
        singleLine = singleLine,
        minLines = if (singleLine) 1 else 3,
        keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
        shape = RoundedCornerShape(tokenDp("shape.field")),
        colors = TextFieldDefaults.colors(
            focusedTextColor = content,
            unfocusedTextColor = content,
            disabledTextColor = secondaryContent,
            errorTextColor = content,
            focusedContainerColor = fieldContainer,
            unfocusedContainerColor = fieldContainer,
            disabledContainerColor = tokenColor("color.surfaceContainer"),
            errorContainerColor = tokenColor("color.errorContainer"),
            cursorColor = tokenColor("color.primary"),
            errorCursorColor = errorColor,
            focusedIndicatorColor = Color.Transparent,
            unfocusedIndicatorColor = Color.Transparent,
            disabledIndicatorColor = Color.Transparent,
            errorIndicatorColor = errorColor,
            focusedLabelColor = tokenColor("color.primary"),
            unfocusedLabelColor = secondaryContent,
            disabledLabelColor = secondaryContent,
            errorLabelColor = errorColor,
        ),
        supportingText = if (description.isNotBlank()) ({
            Column {
                help?.let { FieldHelp(it) }
                errorText?.let { FieldError(it) }
                state.reason()?.let { FieldHelp(it) }
            }
        }) else null,
    )
}

@Composable
fun TextField(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    state: FieldState = FieldState.Editable,
    help: String? = null,
    error: String? = null,
) {
    ControlledTextField(label, value, onValueChange, modifier, state, help, error, true, KeyboardType.Text)
}

@Composable
fun TextArea(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    state: FieldState = FieldState.Editable,
    help: String? = null,
    error: String? = null,
) {
    ControlledTextField(label, value, onValueChange, modifier, state, help, error, false, KeyboardType.Text)
}

/** Keep incomplete decimal input visible; parsing/validation stays with the caller. */
@Composable
fun NumberField(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    state: FieldState = FieldState.Editable,
    help: String? = null,
    error: String? = null,
) {
    ControlledTextField(label, value, onValueChange, modifier, state, help, error, true, KeyboardType.Decimal)
}

@Composable
fun Select(
    label: String,
    options: List<String>,
    selected: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    expandedDescription: String,
    collapsedDescription: String,
) {
    require(options.isNotEmpty() && selected in options.indices)
    require(expandedDescription.isNotBlank() && collapsedDescription.isNotBlank())
    // Unavailability ends this open interaction; restoring availability never reopens it.
    var expanded by remember(state.canPress()) { mutableStateOf(false) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        FieldLabel(label)
        Box {
            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                ButtonControl(options[selected], onPress = { expanded = true }, state = state,
                    variant = ButtonVariant.Secondary,
                    name = AccessibleName.of("$label: ${options[selected]}"),
                    description = if (expanded && state.canPress()) expandedDescription else collapsedDescription)
                state.reason()?.let { FieldHelp(it) }
            }
            DropdownMenu(expanded = expanded && state.canPress(), onDismissRequest = { expanded = false }) {
                options.forEachIndexed { index, option ->
                    DropdownMenuItem(
                        text = { Text(option) },
                        onClick = { if (state.canPress()) onSelect(index); expanded = false },
                        leadingIcon = if (index == selected) ({ Icon("check") }) else null,
                    )
                }
            }
        }
    }
}

@Composable
private fun ChoiceLabel(
    label: String,
    state: ButtonState,
    modifier: Modifier,
    content: @Composable () -> Unit,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
            content()
            Text(label, modifier = Modifier.padding(top = tokenDp("space.inset")))
        }
        state.reason()?.let { FieldHelp(it) }
    }
}

@Composable
fun Checkbox(
    label: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
) {
    ChoiceLabel(label, state, modifier) {
        MaterialCheckbox(checked, onCheckedChange = { if (state.canPress()) onCheckedChange(it) },
            enabled = state.canPress(), modifier = Modifier.cantosFocus().actionState(state)
                .semantics { contentDescription = label })
    }
}

@Composable
fun Radio(
    label: String,
    selected: Boolean,
    onSelect: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
) {
    ChoiceLabel(label, state, modifier) {
        RadioButton(selected, onClick = { if (state.canPress()) onSelect() },
            enabled = state.canPress(), modifier = Modifier.cantosFocus().actionState(state)
                .semantics { contentDescription = label })
    }
}

@Composable
fun Switch(
    label: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
) {
    ChoiceLabel(label, state, modifier) {
        MaterialSwitch(checked, onCheckedChange = { if (state.canPress()) onCheckedChange(it) },
            enabled = state.canPress(), modifier = Modifier.cantosFocus().actionState(state)
                .semantics { contentDescription = label })
    }
}

@Composable
fun RangeField(
    label: String,
    value: Float,
    onValueChange: (Float) -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    range: ClosedFloatingPointRange<Float> = 0f..100f,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        FieldLabel("$label: ${value.toInt()}")
        Slider(value, onValueChange = { if (state.canPress()) onValueChange(it) }, valueRange = range,
            enabled = state.canPress(), modifier = Modifier.fillMaxWidth().cantosFocus().actionState(state)
                .semantics { contentDescription = label })
        state.reason()?.let { FieldHelp(it) }
    }
}

@Composable
fun FieldLabel(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier, style = MaterialTheme.typography.labelLarge)
}

@Composable
fun FieldHelp(text: String, modifier: Modifier = Modifier) {
    if (text.isBlank()) return
    Text(text, modifier, style = MaterialTheme.typography.bodySmall, color = tokenColor("color.onSurfaceVariant"))
}

@Composable
fun FieldError(text: String, modifier: Modifier = Modifier) {
    if (text.isBlank()) return
    Text(text, modifier.semantics { error(text) }, style = MaterialTheme.typography.bodySmall,
        color = tokenColor("color.error"))
}

private fun StatusTone.role(): String = when (this) {
    StatusTone.Neutral -> "surface"
    StatusTone.Info -> "primary"
    StatusTone.Success -> "success"
    StatusTone.Warning -> "warning"
    StatusTone.Error -> "error"
}

@Composable
fun Badge(text: String, modifier: Modifier = Modifier, tone: StatusTone = StatusTone.Neutral) {
    androidx.compose.material3.Surface(
        modifier,
        color = tokenColor("color.${tone.role()}Container"),
        contentColor = when (tone) {
            StatusTone.Neutral -> tokenColor("color.onSurface")
            StatusTone.Info -> tokenColor("color.onPrimaryContainer")
            else -> tokenColor("color.${tone.role()}")
        },
        shape = RoundedCornerShape(tokenDp("shape.pill")),
    ) {
        Text(text, Modifier.padding(horizontal = tokenDp("space.inset"), vertical = tokenDp("space.xs")),
            style = MaterialTheme.typography.labelSmall)
    }
}

@Composable
fun Status(text: String, modifier: Modifier = Modifier, tone: StatusTone = StatusTone.Neutral, announce: Boolean = false) {
    Row(modifier.semantics { if (announce) liveRegion = LiveRegionMode.Polite },
        horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Icon(when (tone) {
            StatusTone.Error, StatusTone.Warning -> "warning_amber"
            StatusTone.Success -> "check"
            StatusTone.Neutral, StatusTone.Info -> "menu_book"
        })
        Badge(text, tone = tone)
    }
}

@Composable
fun LoadingState(text: String, modifier: Modifier = Modifier) {
    Row(modifier.semantics { liveRegion = LiveRegionMode.Polite; stateDescription = text },
        horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        // This sample deliberately has no perpetually animated spinner: reduced-motion parity is honest.
        Icon("expand_more")
        Text(text)
    }
}

@Composable
fun EmptyState(title: String, message: String, modifier: Modifier = Modifier, action: (@Composable () -> Unit)? = null) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Icon("menu_book")
        Text(title, style = MaterialTheme.typography.titleLarge)
        Text(message, color = tokenColor("color.onSurfaceVariant"))
        action?.invoke()
    }
}

@Composable
fun ErrorState(title: String, message: String, retryLabel: String, onRetry: () -> Unit, modifier: Modifier = Modifier) {
    Column(modifier.semantics { liveRegion = LiveRegionMode.Polite },
        verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Status(title, tone = StatusTone.Error)
        Text(message)
        Button(retryLabel, onRetry, variant = ButtonVariant.Secondary)
    }
}

@Composable
fun Surface(modifier: Modifier = Modifier, tone: SurfaceTone = SurfaceTone.Surface, content: @Composable () -> Unit) {
    val role = when (tone) {
        SurfaceTone.Surface -> "color.surface"
        SurfaceTone.Container -> "color.surfaceContainer"
        SurfaceTone.Raised -> "color.surfaceContainerHigh"
    }
    androidx.compose.material3.Surface(modifier, color = tokenColor(role), contentColor = tokenColor("color.onSurface"),
        shape = RoundedCornerShape(tokenDp("shape.container"))) {
        Box(Modifier.padding(tokenDp("space.md"))) { content() }
    }
}

@Composable
fun Stack(modifier: Modifier = Modifier, direction: StackDirection = StackDirection.Vertical, content: @Composable () -> Unit) {
    when (direction) {
        StackDirection.Vertical -> Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) { content() }
        StackDirection.Horizontal -> Row(modifier, horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) { content() }
        StackDirection.Wrap -> FlowRow(modifier, horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
            verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) { content() }
    }
}

/** Reflow child groups rather than shrinking text or native controls. */
@Composable
fun Grid(items: List<@Composable () -> Unit>, modifier: Modifier = Modifier, requestedColumns: Int = 2) {
    require(requestedColumns in 1..3)
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val capacity = when {
            maxWidth >= tokenDp("breakpoint.expanded") -> 3
            maxWidth >= tokenDp("breakpoint.medium") -> 2
            else -> 1
        }
        val columns = minOf(capacity, requestedColumns)
        Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            items.chunked(columns).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                    row.forEach { child -> Box(Modifier.weight(1f)) { child() } }
                    repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
}

@Composable
fun Container(modifier: Modifier = Modifier, width: ContainerWidth = ContainerWidth.Reading, content: @Composable () -> Unit) {
    val limit = when (width) {
        ContainerWidth.Reading -> Modifier.widthIn(max = tokenDp("breakpoint.expanded"))
        ContainerWidth.Wide -> Modifier.widthIn(max = tokenDp("breakpoint.expanded") + tokenDp("breakpoint.medium") / 2)
        ContainerWidth.Fluid -> Modifier
    }
    Box(modifier.then(limit).fillMaxWidth().padding(tokenDp("space.md"))) { content() }
}

/** Built-in native popup owns modality; the preview caller demonstrates focus restoration. */
@Composable
fun Dialog(
    open: Boolean,
    title: String,
    closeLabel: String,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    if (open) {
        AlertDialog(
            onDismissRequest = onDismiss,
            modifier = modifier,
            title = { Text(title) },
            text = content,
            confirmButton = { Button(closeLabel, onDismiss, variant = ButtonVariant.Text) },
            containerColor = tokenColor("color.surface"),
            shape = RoundedCornerShape(tokenDp("shape.overlay")),
        )
    }
}

/** Decorative official Material SVG paths; the enclosing control supplies its name. */
@Composable
fun Icon(name: String, modifier: Modifier = Modifier) {
    val icon = CantosIcons.icons.firstOrNull { it.name == name }
    require(icon != null) { "Unknown official icon: $name" }
    val tint = tokenColor("color.onSurfaceVariant")
    val vector = remember(name, tint) {
        val bounds = icon.viewBox.split(Regex("\\s+")).map { it.toFloat() }
        ImageVector.Builder(
            name = name,
            defaultWidth = CantosTokens.number("size.icon").toFloat().dp,
            defaultHeight = CantosTokens.number("size.icon").toFloat().dp,
            viewportWidth = bounds[2], viewportHeight = bounds[3],
        ).apply {
            icon.pathData.forEach { pathData ->
                addPath(pathData = PathParser().parsePathString(pathData).toNodes(), fill = SolidColor(tint))
            }
        }.build()
    }
    MaterialIcon(vector, contentDescription = null, modifier = modifier.size(tokenDp("size.icon")), tint = tint)
}
