package com.cantos.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.material3.TextField as MaterialTextField
import androidx.compose.material3.TextFieldColors
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue

/** Blank messages have no visible or semantic invalid state, matching the Web contract. */
internal data class FieldMessages(val help: String?, val error: String?, val reason: String?) {
    val description: String = listOfNotNull(help, error, reason).joinToString(". ")
}

internal fun fieldMessages(state: FieldState, help: String?, error: String?): FieldMessages = FieldMessages(
    help = help?.takeIf { it.isNotBlank() },
    error = error?.takeIf { it.isNotBlank() },
    reason = state.reason(),
)

/** Read-only fields may report selection, but cannot alter caller-owned text or composition. */
internal fun FieldState.acceptsValueChange(previous: TextFieldValue, next: TextFieldValue): Boolean = when (this) {
    FieldState.Editable -> true
    FieldState.ReadOnly -> previous.annotatedString == next.annotatedString && previous.composition == next.composition
    is FieldState.Disabled, is FieldState.Loading -> false
}

private fun KeyboardActions.guardedBy(state: FieldState): KeyboardActions = KeyboardActions(
    onDone = onDone?.let { action -> { if (state.acceptsInput()) action() } },
    onGo = onGo?.let { action -> { if (state.acceptsInput()) action() } },
    onNext = onNext?.let { action -> { if (state.acceptsInput()) action() } },
    onPrevious = onPrevious?.let { action -> { if (state.acceptsInput()) action() } },
    onSearch = onSearch?.let { action -> { if (state.acceptsInput()) action() } },
    onSend = onSend?.let { action -> { if (state.acceptsInput()) action() } },
)

@Composable
private fun Modifier.fieldSemantics(messages: FieldMessages, variant: TextFieldVariant): Modifier = fillMaxWidth()
    .then(if (variant == TextFieldVariant.AuthTonal) Modifier.heightIn(min = tokenDp("size.authField")) else Modifier)
    .cantosFocus(if (variant == TextFieldVariant.AuthTonal) "shape.authField" else "shape.field")
    .then(if (variant == TextFieldVariant.AuthTonal && messages.error != null) Modifier.border(
        BorderStroke(tokenDp("size.authOutline"), tokenColor("color.error")), CantosShapes.authField,
    ) else Modifier)
    .semantics {
        if (messages.description.isNotBlank()) stateDescription = messages.description
        messages.error?.let { error(it) }
    }

@Composable
private fun fieldColors(state: FieldState, variant: TextFieldVariant): TextFieldColors {
    val container = tokenColor(if (state == FieldState.ReadOnly || variant == TextFieldVariant.AuthTonal) "color.surfaceContainer" else "color.surfaceContainerHigh")
    val content = tokenColor("color.onSurface")
    val secondary = tokenColor("color.onSurfaceVariant")
    val error = tokenColor("color.error")
    return TextFieldDefaults.colors(
        focusedTextColor = content,
        unfocusedTextColor = content,
        disabledTextColor = secondary,
        errorTextColor = content,
        focusedContainerColor = container,
        unfocusedContainerColor = container,
        disabledContainerColor = tokenColor("color.surfaceContainer"),
        errorContainerColor = tokenColor("color.errorContainer"),
        cursorColor = tokenColor("color.primary"),
        errorCursorColor = error,
        focusedIndicatorColor = Color.Transparent,
        unfocusedIndicatorColor = Color.Transparent,
        disabledIndicatorColor = Color.Transparent,
        errorIndicatorColor = if (variant == TextFieldVariant.AuthTonal) Color.Transparent else error,
        focusedLabelColor = tokenColor("color.primary"),
        unfocusedLabelColor = secondary,
        disabledLabelColor = secondary,
        errorLabelColor = error,
    )
}

@Composable
private fun fieldSupportingText(messages: FieldMessages): (@Composable () -> Unit)? =
    if (messages.description.isBlank()) null else ({
        Column {
            messages.help?.let { FieldHelp(it) }
            messages.error?.let { FieldError(it) }
            messages.reason?.let { FieldHelp(it) }
        }
    })

/** Auth help remains readable outside the functional boundary and focus paint. */
@Composable
private fun FieldFrame(
    modifier: Modifier,
    variant: TextFieldVariant,
    messages: FieldMessages,
    content: @Composable (Modifier) -> Unit,
) {
    if (variant == TextFieldVariant.Regular) {
        content(modifier)
    } else {
        val supportGap = tokenDp("size.focusOffset") + tokenDp("size.focusWidth") + tokenDp("space.xs")
        Column(modifier, verticalArrangement = Arrangement.spacedBy(supportGap)) {
            content(Modifier)
            fieldSupportingText(messages)?.invoke()
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
    error: String?,
    singleLine: Boolean,
    keyboardOptions: KeyboardOptions,
    keyboardActions: KeyboardActions,
    minLines: Int = 1,
    maxLines: Int = Int.MAX_VALUE,
    variant: TextFieldVariant = TextFieldVariant.Regular,
    controlModifier: Modifier = Modifier,
) {
    require(minLines > 0 && maxLines >= minLines) { "Expected positive ordered line limits" }
    val messages = fieldMessages(state, help, error)
    FieldFrame(modifier, variant, messages) { frameModifier ->
        MaterialTextField(
            value = value,
            onValueChange = { if (state.acceptsInput()) onValueChange(it) },
            modifier = frameModifier.then(controlModifier).fieldSemantics(messages, variant),
            label = { Text(label) },
            enabled = state.isEnabled(),
            readOnly = state == FieldState.ReadOnly,
            isError = messages.error != null,
            singleLine = singleLine,
            minLines = minLines,
            maxLines = maxLines,
            keyboardOptions = keyboardOptions,
            keyboardActions = keyboardActions.guardedBy(state),
            shape = if (variant == TextFieldVariant.AuthTonal) CantosShapes.authField else CantosShapes.reading,
            colors = fieldColors(state, variant),
            supportingText = if (variant == TextFieldVariant.AuthTonal) null else fieldSupportingText(messages),
        )
    }
}

@Composable
private fun ControlledTextField(
    label: String,
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    modifier: Modifier,
    state: FieldState,
    help: String?,
    error: String?,
    singleLine: Boolean,
    keyboardOptions: KeyboardOptions,
    keyboardActions: KeyboardActions,
    minLines: Int = 1,
    maxLines: Int = Int.MAX_VALUE,
    variant: TextFieldVariant = TextFieldVariant.Regular,
    controlModifier: Modifier = Modifier,
) {
    require(minLines > 0 && maxLines >= minLines) { "Expected positive ordered line limits" }
    val messages = fieldMessages(state, help, error)
    FieldFrame(modifier, variant, messages) { frameModifier ->
        MaterialTextField(
            value = value,
            onValueChange = { if (state.acceptsValueChange(value, it)) onValueChange(it) },
            modifier = frameModifier.then(controlModifier).fieldSemantics(messages, variant),
            label = { Text(label) },
            enabled = state.isEnabled(),
            readOnly = state == FieldState.ReadOnly,
            isError = messages.error != null,
            singleLine = singleLine,
            minLines = minLines,
            maxLines = maxLines,
            keyboardOptions = keyboardOptions,
            keyboardActions = keyboardActions.guardedBy(state),
            shape = if (variant == TextFieldVariant.AuthTonal) CantosShapes.authField else CantosShapes.reading,
            colors = fieldColors(state, variant),
            supportingText = if (variant == TextFieldVariant.AuthTonal) null else fieldSupportingText(messages),
        )
    }
}

/** Controlled raw text; the native field owns cursor/selection internally. */
@Composable
fun TextField(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    state: FieldState = FieldState.Editable,
    help: String? = null,
    error: String? = null,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    variant: TextFieldVariant = TextFieldVariant.Regular,
    controlModifier: Modifier = Modifier,
) {
    ControlledTextField(
        label = label,
        value = value,
        onValueChange = onValueChange,
        modifier = modifier,
        state = state,
        help = help,
        error = error,
        singleLine = true,
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        variant = variant,
        controlModifier = controlModifier,
    )
}

/** Caller owns raw text, selection and IME composition; return the received value unchanged while composing. */
@Composable
fun TextField(
    label: String,
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
    state: FieldState = FieldState.Editable,
    help: String? = null,
    error: String? = null,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    variant: TextFieldVariant = TextFieldVariant.Regular,
    controlModifier: Modifier = Modifier,
) {
    ControlledTextField(
        label = label,
        value = value,
        onValueChange = onValueChange,
        modifier = modifier,
        state = state,
        help = help,
        error = error,
        singleLine = true,
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        variant = variant,
        controlModifier = controlModifier,
    )
}

/** Controlled raw text; the native field owns cursor/selection internally. */
@Composable
fun TextArea(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    state: FieldState = FieldState.Editable,
    help: String? = null,
    error: String? = null,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    minLines: Int = 3,
    maxLines: Int = Int.MAX_VALUE,
) {
    ControlledTextField(
        label = label,
        value = value,
        onValueChange = onValueChange,
        modifier = modifier,
        state = state,
        help = help,
        error = error,
        singleLine = false,
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        minLines = minLines,
        maxLines = maxLines,
    )
}

/** Caller owns raw text, selection and IME composition; return the received value unchanged while composing. */
@Composable
fun TextArea(
    label: String,
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
    state: FieldState = FieldState.Editable,
    help: String? = null,
    error: String? = null,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    minLines: Int = 3,
    maxLines: Int = Int.MAX_VALUE,
) {
    ControlledTextField(
        label = label,
        value = value,
        onValueChange = onValueChange,
        modifier = modifier,
        state = state,
        help = help,
        error = error,
        singleLine = false,
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        minLines = minLines,
        maxLines = maxLines,
    )
}

/** Controlled raw text; the native field owns cursor/selection internally. */
/** Decimal keyboard hint only; incomplete values, zero and localized separators are never parsed here. */
@Composable
fun NumberField(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    state: FieldState = FieldState.Editable,
    help: String? = null,
    error: String? = null,
    keyboardOptions: KeyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
    keyboardActions: KeyboardActions = KeyboardActions.Default,
) {
    ControlledTextField(
        label = label,
        value = value,
        onValueChange = onValueChange,
        modifier = modifier,
        state = state,
        help = help,
        error = error,
        singleLine = true,
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
    )
}

/** Caller owns raw text, selection and IME composition; return the received value unchanged while composing. */
/** Decimal keyboard hint only; incomplete values, zero and localized separators are never parsed here. */
@Composable
fun NumberField(
    label: String,
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
    state: FieldState = FieldState.Editable,
    help: String? = null,
    error: String? = null,
    keyboardOptions: KeyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
    keyboardActions: KeyboardActions = KeyboardActions.Default,
) {
    ControlledTextField(
        label = label,
        value = value,
        onValueChange = onValueChange,
        modifier = modifier,
        state = state,
        help = help,
        error = error,
        singleLine = true,
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
    )
}
