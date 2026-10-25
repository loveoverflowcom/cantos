package com.cantos.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.Checkbox as MaterialCheckbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuDefaults
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch as MaterialSwitch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.util.lerp
import kotlin.math.abs
import kotlin.math.ulp
import kotlin.math.nextDown
import kotlin.math.nextUp
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription

private fun choiceDescription(state: ButtonState, help: String?, error: String?): String =
    listOfNotNull(help, error, state.reason()).filter { it.isNotBlank() }.joinToString(". ")

private fun Modifier.choiceMessages(state: ButtonState, help: String?, message: String?): Modifier = semantics {
    val description = choiceDescription(state, help, message)
    if (description.isNotBlank()) stateDescription = description
    message?.takeIf { it.isNotBlank() }?.let { error(it) }
}

@Composable
private fun ChoiceMessages(state: ButtonState, help: String?, error: String?) {
    help?.let { FieldHelp(it) }
    error?.let { FieldError(it) }
    state.reason()?.let { FieldHelp(it) }
}

/** One full-label native target. The visual control has no second input/focus handler. */
@Composable
private fun ChoiceLabel(
    label: String,
    modifier: Modifier,
    controlModifier: Modifier,
    state: ButtonState,
    help: String?,
    error: String?,
    interaction: Modifier,
    content: @Composable () -> Unit,
) {
    require(label.isNotBlank())
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        Row(controlModifier.heightIn(min = tokenDp("size.control")).cantosFocus()
            .choiceMessages(state, help, error).semantics { contentDescription = label }
            .then(interaction).padding(horizontal = tokenDp("space.xs")),
            horizontalArrangement = Arrangement.spacedBy(tokenDp("space.sm")),
            verticalAlignment = Alignment.CenterVertically) {
            content()
            Text(label, modifier = Modifier.weight(1f, fill = false),
                color = if (state.canPress()) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (choiceDescription(state, help, error).isNotBlank()) {
            // The ring paints outside the native row. Keep supporting text beyond that paint
            // with the normal message gap, and reserve it before focus so the layout stays still.
            Column(
                modifier = Modifier.padding(top = tokenDp("size.focusOffset") + tokenDp("size.focusWidth")),
                verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs")),
            ) {
                ChoiceMessages(state, help, error)
            }
        }
    }
}

@Composable
fun Checkbox(
    label: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    controlModifier: Modifier = Modifier,
    help: String? = null,
    error: String? = null,
) {
    ChoiceLabel(label, modifier, controlModifier, state, help, error,
        Modifier.toggleable(checked, enabled = state.canPress(), role = Role.Checkbox,
            onValueChange = { if (state.canPress()) onCheckedChange(it) })) {
        MaterialCheckbox(checked, onCheckedChange = null, enabled = state.canPress())
    }
}

@Composable
fun Radio(
    label: String,
    selected: Boolean,
    onSelect: () -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    controlModifier: Modifier = Modifier,
    help: String? = null,
    error: String? = null,
) {
    ChoiceLabel(label, modifier, controlModifier, state, help, error,
        Modifier.selectable(selected, enabled = state.canPress(), role = Role.RadioButton,
            onClick = { if (state.canPress()) onSelect() })) {
        RadioButton(selected, onClick = null, enabled = state.canPress())
    }
}

@Composable
fun Switch(
    label: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    controlModifier: Modifier = Modifier,
    help: String? = null,
    error: String? = null,
) {
    ChoiceLabel(label, modifier, controlModifier, state, help, error,
        Modifier.toggleable(checked, enabled = state.canPress(), role = Role.Switch,
            onValueChange = { if (state.canPress()) onCheckedChange(it) })) {
        MaterialSwitch(checked, onCheckedChange = null, enabled = state.canPress())
    }
}

/** Compatibility index API. Dynamic lists should use the stable-value overload. */
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
    controlModifier: Modifier = Modifier,
    help: String? = null,
    error: String? = null,
) {
    require(options.isNotEmpty() && selected in options.indices)
    Select(label, options.mapIndexed { index, text -> SelectOption(index.toString(), text) },
        selected.toString(), { value -> onSelect(value.toInt()) }, modifier, state,
        expandedDescription, collapsedDescription, placeholder = label, emptyDescription = label,
        controlModifier = controlModifier, help = help, error = error)
}

/** Missing/empty selection never invents a value or dispatches a callback. */
@Composable
fun Select(
    label: String,
    options: List<SelectOption>,
    value: String?,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    expandedDescription: String,
    collapsedDescription: String,
    placeholder: String,
    emptyDescription: String,
    controlModifier: Modifier = Modifier,
    help: String? = null,
    error: String? = null,
) {
    require(label.isNotBlank() && placeholder.isNotBlank() && emptyDescription.isNotBlank())
    require(expandedDescription.isNotBlank() && collapsedDescription.isNotBlank())
    require(options.map { it.value }.distinct().size == options.size) { "Option values must be unique" }
    val effective = if (options.isEmpty() && state.canPress()) {
        ButtonState.Disabled(DisabledReason.of(emptyDescription))
    } else state
    val selectedOption = options.firstOrNull { it.value == value }
    val hasIcons = options.any { it.icon != null }
    val text = selectedOption?.label ?: placeholder
    // Any external input change interrupts the menu. Availability recovery cannot reopen it.
    var expanded by remember(effective.canPress(), options.toList(), value) { mutableStateOf(false) }
    var anchorWidth by remember { mutableStateOf(0) }
    val menuMinWidth = with(LocalDensity.current) { anchorWidth.toDp() }
    val description = listOf(if (expanded) expandedDescription else collapsedDescription,
        choiceDescription(ButtonState.Enabled, help, error), selectedOption?.state?.reason()).filterNotNull()
        .filter { it.isNotBlank() }.joinToString(". ")
    // Theatre: one tonal capsule, its value + chevron one centered group, and a single inset cue
    // (2 while open or focused) instead of the outset ring. Standard keeps the button anatomy.
    val theatre = LocalUiPreferences.current.expression == CantosExpression.Theatre
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        FieldLabel(label)
        Box {
            ButtonControl(onPress = { expanded = true }, state = effective,
                shape = if (theatre) CantosShapes.full else null, insetPerimeter = theatre,
                invalid = theatre && !error.isNullOrBlank(),
                active = theatre && expanded && effective.canPress(),
                containerColor = if (theatre) tokenColor("color.surfaceContainer") else null,
                modifier = controlModifier.onSizeChanged { anchorWidth = it.width }
                    .semantics { error?.takeIf { it.isNotBlank() }?.let { error(it) } },
                variant = ButtonVariant.Tonal, name = AccessibleName.of(listOf(label, text).joinToString(": ")),
                description = description) {
                if (hasIcons) {
                    Box(Modifier.padding(end = tokenDp("space.sm")).size(tokenDp("size.icon"))) {
                        selectedOption?.icon?.let { Icon(it) }
                    }
                }
                Text(text, Modifier.weight(1f, fill = !theatre))
                // A trailing slot keeps long labels away from the decorative disclosure.
                // Loading already has the action's pending cue; it must not look expandable.
                Box(Modifier.padding(start = tokenDp("space.sm")).size(tokenDp("size.icon"))) {
                    if (effective !is ButtonState.Loading) Icon("expand_more")
                }
            }
            DropdownMenu(expanded = expanded && effective.canPress(), onDismissRequest = { expanded = false },
                modifier = Modifier.widthIn(min = menuMinWidth), shape = CantosShapes.overlay,
                containerColor = MaterialTheme.colorScheme.surface) {
                options.forEach { option ->
                    val isSelected = option.value == value
                    val selectedColor = MaterialTheme.colorScheme.onPrimaryContainer
                    DropdownMenuItem(text = {
                        Column {
                            Text(option.label)
                            option.state.reason()?.let { FieldHelp(it) }
                        }
                    }, enabled = option.state.canPress(),
                        colors = MenuDefaults.itemColors(
                            textColor = if (isSelected) selectedColor else MaterialTheme.colorScheme.onSurface,
                            leadingIconColor = if (isSelected) selectedColor else MaterialTheme.colorScheme.onSurfaceVariant,
                            trailingIconColor = if (isSelected) selectedColor else MaterialTheme.colorScheme.onSurfaceVariant,
                        ),
                        modifier = Modifier.background(
                            if (isSelected) MaterialTheme.colorScheme.primaryContainer else Color.Transparent,
                            CantosShapes.reading,
                        ).cantosFocus().actionState(option.state).semantics { selected = isSelected },
                        onClick = {
                            if (effective.canPress() && option.state.canPress()) onValueChange(option.value)
                            expanded = false
                        }, leadingIcon = if (hasIcons) {
                            { Box(Modifier.size(tokenDp("size.icon"))) { option.icon?.let { Icon(it) } } }
                        } else null, trailingIcon = {
                            Box(Modifier.size(tokenDp("size.icon"))) {
                                if (option.value == value) Icon("check")
                            }
                        })
                }
            }
        }
        ChoiceMessages(effective, help, error)
    }
}

/** Native steps count interior ticks, not interval size. Callers supply localized value text. */
@Composable
fun RangeField(
    label: String,
    value: Float,
    onValueChange: (Float) -> Unit,
    modifier: Modifier = Modifier,
    state: ButtonState = ButtonState.Enabled,
    range: ClosedFloatingPointRange<Float> = 0f..100f,
    steps: Int = 0,
    valueText: String = value.toString(),
    onValueChangeFinished: () -> Unit = {},
    controlModifier: Modifier = Modifier,
    help: String? = null,
    error: String? = null,
) {
    validateRange(value, range, steps)
    require(label.isNotBlank() && valueText.isNotBlank())
    var keyboardChanged by remember(state.canPress()) { mutableStateOf(false) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
        FieldLabel("$label: $valueText")
        Slider(value, onValueChange = { request ->
            if (state.canPress()) rangeChangeValue(request, range, steps)?.let(onValueChange)
        }, valueRange = range,
            steps = steps, onValueChangeFinished = { if (state.canPress()) onValueChangeFinished() },
            enabled = state.canPress(), modifier = controlModifier.fillMaxWidth().cantosFocus()
                .onFocusChanged { if (!it.isFocused) keyboardChanged = false }
                .onPreviewKeyEvent { event ->
                    val next = rangeKeyValue(value, range, steps, event.key)
                    if (!state.canPress() || next == null) false
                    else {
                        if (event.type == KeyEventType.KeyDown && next != rangeCurrentValue(value, range, steps)) {
                            onValueChange(next)
                            keyboardChanged = true
                        } else if (event.type == KeyEventType.KeyUp && keyboardChanged) {
                            keyboardChanged = false
                            onValueChangeFinished()
                        }
                        true
                    }
                }
                .choiceMessages(state, help, error).semantics {
                    contentDescription = label
                    stateDescription = listOf(valueText, choiceDescription(state, help, error))
                        .filter { it.isNotBlank() }.joinToString(". ")
                    // Pinned Material3 can emit off-tick negative values or NaN through
                    // SetProgress. Keep every native request closed over our value contract.
                    setProgress { request ->
                        val next = rangeChangeValue(request, range, steps)
                        if (!state.canPress() || next == null || next == rangeCurrentValue(value, range, steps)) false
                        else {
                            onValueChange(next)
                            onValueChangeFinished()
                            true
                        }
                    }
                })
        ChoiceMessages(state, help, error)
    }
}

internal fun validateRange(value: Float, range: ClosedFloatingPointRange<Float>, steps: Int) {
    require(range.start.isFinite() && range.endInclusive.isFinite() && range.start < range.endInclusive &&
        (range.endInclusive - range.start).isFinite()) { "Range endpoints and span must be finite and increasing" }
    require(value.isFinite() && value in range) { "Controlled value must be finite and inside its range" }
    // Material allocates and renders every tick. Bound work instead of accepting a UI-sized OOM.
    require(steps in 0..1000) { "Native range supports zero to 1000 interior ticks" }
    if (steps > 0) {
        val ticks = rangeTicks(range, steps)
        require(ticks.all { it.isFinite() }) { "Every stepped tick must be finite" }
        require(ticks.distinct().size == ticks.size) { "Every stepped tick must have a distinct Float value" }
        require(ticks.any { abs(value - it) <= maxOf(value.ulp, it.ulp) }) {
            "Controlled stepped value must match a native Float tick within one ULP"
        }
    }
}

/** Bounded Float projections keep validation and keyboard navigation closed over their outputs. */
internal fun rangeTicks(range: ClosedFloatingPointRange<Float>, steps: Int): List<Float> =
    (0..steps + 1).map { index ->
        lerp(range.start, range.endInclusive, index.toFloat() / (steps + 1))
    }

/** Project requests only; never rewrite the caller's controlled value. Ties choose the higher tick. */
internal fun rangeChangeValue(request: Float, range: ClosedFloatingPointRange<Float>, steps: Int): Float? {
    if (!request.isFinite()) return null
    val bounded = request.coerceIn(range)
    if (steps == 0) return bounded
    return rangeTicks(range, steps).asReversed().minByOrNull { abs(it.toDouble() - bounded.toDouble()) }
}

/** Material's current thumb snaps lower on a tie; a caller decimal may be one ULP away. */
private fun rangeCurrentValue(value: Float, range: ClosedFloatingPointRange<Float>, steps: Int): Float =
    if (steps == 0) value else requireNotNull(rangeTicks(range, steps).minByOrNull { abs(it.toDouble() - value.toDouble()) })

/** Pinned Material3 has progress semantics but no key handler. Preserve native focus/drag. */
internal fun rangeKeyValue(value: Float, range: ClosedFloatingPointRange<Float>, steps: Int, key: Key): Float? {
    val delta = when (key) {
        Key.DirectionRight, Key.DirectionUp -> 1
        Key.DirectionLeft, Key.DirectionDown -> -1
        Key.PageUp -> 10
        Key.PageDown -> -10
        Key.MoveHome -> return range.start
        Key.MoveEnd -> return range.endInclusive
        else -> return null
    }
    if (steps > 0) {
        val ticks = rangeTicks(range, steps)
        val current = requireNotNull(ticks.indices.minByOrNull { abs(ticks[it].toDouble() - value.toDouble()) })
        return ticks[(current + delta).coerceIn(ticks.indices)]
    }
    val span = range.endInclusive.toDouble() - range.start.toDouble()
    val next = (value.toDouble() + span / 100 * delta)
        .coerceIn(range.start.toDouble(), range.endInclusive.toDouble()).toFloat()
    // On narrow/large-offset Float ranges, one percent can round to the current
    // value. Make directional input advance by the smallest representable value.
    return when {
        next != value -> next
        delta > 0 && value < range.endInclusive -> value.nextUp().coerceAtMost(range.endInclusive)
        delta < 0 && value > range.start -> value.nextDown().coerceAtLeast(range.start)
        else -> value
    }
}
