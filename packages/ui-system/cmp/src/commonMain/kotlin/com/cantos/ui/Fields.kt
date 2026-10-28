package com.cantos.ui

import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.ui.draw.clip
import androidx.compose.runtime.getValue
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField as MaterialTextField
import androidx.compose.material3.TextFieldColors
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.cantos.ui.generated.CantosTokens

// Renderer-local geometry from the Theatre v4 design (board 5), in logical dp rather than Web CSS
// pixels. Both are minimums: the label, value and text scale grow the shell, nothing is clipped.
internal val FloatingFieldMinHeight: Dp = 68.dp
internal val FloatingFieldRadius: Dp = 16.dp

/** The two shell variants share one anatomy; only the search field changes shape and minimum. */
internal fun TextFieldVariant.isFloating(): Boolean =
    this == TextFieldVariant.Floating || this == TextFieldVariant.FloatingSearch || this == TextFieldVariant.FloatingTonal

/** AuthTonal puts its label outside the shell and the docked search has none: its label is the prompt and the accessible name. */
internal fun TextFieldVariant.hasFieldLabel(): Boolean = this != TextFieldVariant.AuthTonal && this != TextFieldVariant.FloatingSearch

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
private fun Modifier.fieldSemantics(messages: FieldMessages, variant: TextFieldVariant, label: String, docked: Boolean = false): Modifier = fillMaxWidth()
    .then(when (variant) {
        TextFieldVariant.AuthTonal -> Modifier.heightIn(min = tokenDp("size.authField"))
        TextFieldVariant.Floating, TextFieldVariant.FloatingTonal -> Modifier.heightIn(min = FloatingFieldMinHeight)
        TextFieldVariant.FloatingSearch -> Modifier.heightIn(min = tokenDp("size.searchHeader"))
        TextFieldVariant.Regular -> Modifier
    })
    .then(when (variant) {
        // One inset perimeter owned by this field; no other control's focus geometry changes.
        TextFieldVariant.Floating -> Modifier.cantosInsetPerimeter(FloatingFieldRadius, invalid = messages.error != null)
        // Auth v4.3: the tonal fill, label and caret carry focus and error; nothing is stroked.
        TextFieldVariant.FloatingTonal -> Modifier
        // The docked search header: resting radius 36 (half of the 72 minimum), hover 1 / focus 2 inset.
        TextFieldVariant.FloatingSearch -> if (docked) Modifier else Modifier.cantosInsetPerimeter(RoundedCornerShape(tokenDp("shape.searchRest")), invalid = messages.error != null, hoverStroke = true)
        // The auth shell is painted, so it gets the same Fields v4 perimeter on its own shape.
        TextFieldVariant.AuthTonal -> Modifier.cantosInsetPerimeter(CantosShapes.authField, invalid = messages.error != null)
        // Regular is the Standard expression's field (its error is an underline indicator); no Theatre
        // surface uses it, so it keeps the outset ring.
        TextFieldVariant.Regular -> Modifier.cantosFocus("shape.field")
    })
    .semantics {
        if (variant == TextFieldVariant.AuthTonal || variant == TextFieldVariant.FloatingSearch) contentDescription = label
        if (messages.description.isNotBlank()) stateDescription = messages.description
        messages.error?.let { error(it) }
    }

@Composable
private fun fieldColors(state: FieldState, variant: TextFieldVariant, docked: Boolean = false): TextFieldColors {
    val tonal = tokenColor("color.surfaceContainer")
    // A non-editable shell is the same tone, paler: state is never carried by the fill alone.
    val quiet = lerp(tonal, tokenColor("color.surface"), FloatingQuietFillFraction)
    val floating = variant.isFloating()
    val tonalRecipe = variant == TextFieldVariant.FloatingTonal
    val high = tokenColor("color.surfaceContainerHigh")
    val container = when {
        docked -> Color.Transparent
        // Web Auth parity: an editable tonal shell rests on the high container, a locked one on the container.
        tonalRecipe -> if (state == FieldState.Editable) high else tonal
        floating -> if (state == FieldState.Editable) tonal else quiet
        state == FieldState.ReadOnly || variant == TextFieldVariant.AuthTonal -> tonal
        else -> tokenColor("color.surfaceContainerHigh")
    }
    val content = tokenColor("color.onSurface")
    val secondary = tokenColor("color.onSurfaceVariant")
    val error = tokenColor("color.error")
    return TextFieldDefaults.colors(
        focusedTextColor = content,
        unfocusedTextColor = content,
        disabledTextColor = secondary,
        errorTextColor = content,
        // Focus is the same primary state layer the Web recipe mixes over the high container.
        focusedContainerColor = if (tonalRecipe && state == FieldState.Editable) {
            lerp(high, tokenColor("color.primary"), CantosTokens.number("state.focusOpacity", LocalUiPreferences.current.density.tokenName).toFloat())
        } else container,
        unfocusedContainerColor = container,
        disabledContainerColor = if (floating && !tonalRecipe) quiet else tonal,
        errorContainerColor = if (floating && !tonalRecipe) container else tokenColor("color.errorContainer"),
        cursorColor = tokenColor("color.primary"),
        errorCursorColor = error,
        focusedIndicatorColor = Color.Transparent,
        unfocusedIndicatorColor = Color.Transparent,
        disabledIndicatorColor = Color.Transparent,
        errorIndicatorColor = if (variant == TextFieldVariant.Regular) error else Color.Transparent,
        focusedLabelColor = if (floating && !tonalRecipe) secondary else tokenColor("color.primary"),
        unfocusedLabelColor = secondary,
        disabledLabelColor = secondary,
        errorLabelColor = error,
        focusedPlaceholderColor = secondary,
        unfocusedPlaceholderColor = secondary,
        focusedTrailingIconColor = secondary,
        unfocusedTrailingIconColor = secondary,
        disabledTrailingIconColor = secondary,
        errorTrailingIconColor = secondary,
    )
}

/** How far a read-only, disabled or loading Floating shell moves from the tonal fill toward the surface. */
private const val FloatingQuietFillFraction: Float = 0.5f

@Composable
private fun fieldSupportingText(messages: FieldMessages, errorIcon: Boolean = false): (@Composable () -> Unit)? =
    if (messages.description.isBlank()) null else ({
        Column {
            messages.help?.let { FieldHelp(it) }
            messages.error?.let { message ->
                // Colour is never the only signal: the tonal recipe pairs the message with a glyph.
                if (errorIcon) Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.xs"))) {
                    Icon("warning_amber", tint = tokenColor("color.error"))
                    FieldError(message)
                } else FieldError(message)
            }
            messages.reason?.let { FieldHelp(it) }
        }
    })

/** Search v4.3's action inset is derived from the canonical 72 header and 52 action. */
private fun fieldTrailing(variant: TextFieldVariant, trailing: (@Composable () -> Unit)?): (@Composable () -> Unit)? =
    trailing?.let { content -> {
        if (variant == TextFieldVariant.FloatingSearch) {
            Box(Modifier.padding(end = (tokenDp("size.searchHeader") - tokenDp("size.searchAction")) / 2)) { content() }
        } else content()
    } }

/**
 * Auth and floating fields keep help outside the painted boundary, so the perimeter and fill wrap
 * only the control. AuthTonal also puts its label outside; Floating keeps it inside the shell.
 */
@Composable
private fun FieldFrame(
    modifier: Modifier,
    variant: TextFieldVariant,
    messages: FieldMessages,
    label: String,
    dockExpanded: Boolean? = null,
    dockContent: (@Composable () -> Unit)? = null,
    content: @Composable (Modifier) -> Unit,
) {
    if (variant == TextFieldVariant.FloatingSearch && dockExpanded != null && dockContent != null) {
        val preference = LocalUiPreferences.current.motion
        val spec = motionFor(MotionIntent.Navigation, MotionProperty.Spatial, preference)
        val radius by animateFloatAsState(
            if (dockExpanded) tokenDp("shape.searchOpen").value else tokenDp("shape.searchRest").value,
            spec.toAnimationSpec(), label = "search-radius",
        )
        val shape = RoundedCornerShape(radius.dp)
        Column(modifier.background(tokenColor("color.surfaceContainer"), shape)
            .cantosInsetPerimeter(shape, invalid = messages.error != null, active = dockExpanded, hoverStroke = true)
            .clip(shape).animateContentSize(animationSpec = when (spec) {
                MotionSpec.Instant -> snap()
                is MotionSpec.Timed -> tween(spec.durationMillis, easing = CubicBezierEasing(spec.easing.x1, spec.easing.y1, spec.easing.x2, spec.easing.y2))
            })) {
            content(Modifier)
            // Removal is synchronous: a closed/closing body never keeps focusable children.
            if (dockExpanded) dockContent()
        }
        fieldSupportingText(messages)?.invoke()
    } else if (variant == TextFieldVariant.Regular) {
        content(modifier)
    } else {
        val supportGap = if (variant.isFloating()) tokenDp("space.xs")
            else tokenDp("size.focusOffset") + tokenDp("size.focusWidth") + tokenDp("space.xs")
        Column(modifier, verticalArrangement = Arrangement.spacedBy(supportGap)) {
            if (variant == TextFieldVariant.AuthTonal) Text(label, style = MaterialTheme.typography.labelLarge)
            content(Modifier)
            fieldSupportingText(messages, errorIcon = variant == TextFieldVariant.FloatingTonal)?.invoke()
        }
    }
}

@Composable
private fun fieldShape(variant: TextFieldVariant): androidx.compose.ui.graphics.Shape = when (variant) {
    TextFieldVariant.AuthTonal -> CantosShapes.authField
    TextFieldVariant.Floating -> RoundedCornerShape(FloatingFieldRadius)
    // Auth's softer corner is the token the auth shell already owns; Search and Floating keep 16.
    TextFieldVariant.FloatingTonal -> CantosShapes.authField
    TextFieldVariant.FloatingSearch -> RoundedCornerShape(tokenDp("shape.searchRest"))
    TextFieldVariant.Regular -> CantosShapes.reading
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
    visualTransformation: VisualTransformation = VisualTransformation.None,
    trailing: (@Composable () -> Unit)? = null,
    dockExpanded: Boolean? = null,
    dockContent: (@Composable () -> Unit)? = null,
) {
    require(minLines > 0 && maxLines >= minLines) { "Expected positive ordered line limits" }
    val messages = fieldMessages(state, help, error)
    FieldFrame(modifier, variant, messages, label, dockExpanded, dockContent) { frameModifier ->
        MaterialTextField(
            value = value,
            onValueChange = { if (state.acceptsInput()) onValueChange(it) },
            modifier = frameModifier.then(controlModifier).fieldSemantics(messages, variant, label, dockExpanded != null),
            label = if (variant.hasFieldLabel()) ({ Text(label) }) else null,
            placeholder = if (variant == TextFieldVariant.FloatingSearch) ({ Text(label) }) else null,
            trailingIcon = fieldTrailing(variant, trailing),
            enabled = state.isEnabled(),
            readOnly = state == FieldState.ReadOnly,
            isError = messages.error != null,
            singleLine = singleLine,
            minLines = minLines,
            maxLines = maxLines,
            keyboardOptions = keyboardOptions,
            keyboardActions = keyboardActions.guardedBy(state),
            visualTransformation = visualTransformation,
            shape = fieldShape(variant),
            colors = fieldColors(state, variant, dockExpanded != null),
            supportingText = if (variant == TextFieldVariant.Regular) fieldSupportingText(messages) else null,
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
    visualTransformation: VisualTransformation = VisualTransformation.None,
    trailing: (@Composable () -> Unit)? = null,
) {
    require(minLines > 0 && maxLines >= minLines) { "Expected positive ordered line limits" }
    val messages = fieldMessages(state, help, error)
    FieldFrame(modifier, variant, messages, label) { frameModifier ->
        MaterialTextField(
            value = value,
            onValueChange = { if (state.acceptsValueChange(value, it)) onValueChange(it) },
            modifier = frameModifier.then(controlModifier).fieldSemantics(messages, variant, label),
            label = if (variant.hasFieldLabel()) ({ Text(label) }) else null,
            placeholder = if (variant == TextFieldVariant.FloatingSearch) ({ Text(label) }) else null,
            trailingIcon = fieldTrailing(variant, trailing),
            enabled = state.isEnabled(),
            readOnly = state == FieldState.ReadOnly,
            isError = messages.error != null,
            singleLine = singleLine,
            minLines = minLines,
            maxLines = maxLines,
            keyboardOptions = keyboardOptions,
            keyboardActions = keyboardActions.guardedBy(state),
            visualTransformation = visualTransformation,
            shape = fieldShape(variant),
            colors = fieldColors(state, variant),
            supportingText = if (variant == TextFieldVariant.Regular) fieldSupportingText(messages) else null,
        )
    }
}

/**
 * Controlled raw text; the native field owns cursor/selection internally.
 *
 * [trailing] is one optional action inside the shell (a password eye, a docked search submit): pass
 * a real control with a localized name, such as [IconButton]. The field reserves its width, so text
 * never runs under it, and owns no action, secret or visibility state. A trailing action keeps its
 * own focus ring; the field's perimeter follows only the text node.
 */
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
    visualTransformation: VisualTransformation = VisualTransformation.None,
    trailing: (@Composable () -> Unit)? = null,
    // Optional body of the canonical search shell; the caller owns state, clamp and requests.
    dockExpanded: Boolean? = null,
    dockContent: (@Composable () -> Unit)? = null,
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
        visualTransformation = visualTransformation,
        trailing = trailing,
        dockExpanded = dockExpanded,
        dockContent = dockContent,
    )
}

/**
 * Caller owns raw text, selection and IME composition; return the received value unchanged while composing.
 * [trailing] has the same contract as the string overload and never changes composition or selection.
 */
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
    visualTransformation: VisualTransformation = VisualTransformation.None,
    trailing: (@Composable () -> Unit)? = null,
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
        visualTransformation = visualTransformation,
        trailing = trailing,
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
