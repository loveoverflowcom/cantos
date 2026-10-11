package com.cantos.ui

import com.cantos.ui.generated.PreviewCatalog
import com.cantos.ui.generated.PreviewComponent

/** A disabled action cannot exist without a human-readable explanation. */
class DisabledReason private constructor(val text: String) {
    companion object {
        fun of(text: String): DisabledReason {
            require(text.isNotBlank()) { "A disabled reason must not be blank" }
            return DisabledReason(text)
        }
    }
}

class AccessibleName private constructor(val text: String) {
    companion object {
        fun of(text: String): AccessibleName {
            require(text.isNotBlank()) { "An accessible name must not be blank" }
            return AccessibleName(text)
        }
    }
}

sealed interface ButtonState {
    data object Enabled : ButtonState
    data class Disabled(val reason: DisabledReason) : ButtonState
    data class Loading(val reason: DisabledReason) : ButtonState
}

fun ButtonState.canPress(): Boolean = this == ButtonState.Enabled

fun ButtonState.reason(): String? = when (this) {
    ButtonState.Enabled -> null
    is ButtonState.Disabled -> reason.text
    is ButtonState.Loading -> reason.text
}

sealed interface FieldState {
    data object Editable : FieldState
    data object ReadOnly : FieldState
    data class Disabled(val reason: DisabledReason) : FieldState
    data class Loading(val reason: DisabledReason) : FieldState
}

fun FieldState.acceptsInput(): Boolean = this == FieldState.Editable

fun FieldState.isEnabled(): Boolean = this == FieldState.Editable || this == FieldState.ReadOnly

fun FieldState.reason(): String? = when (this) {
    FieldState.Editable, FieldState.ReadOnly -> null
    is FieldState.Disabled -> reason.text
    is FieldState.Loading -> reason.text
}

enum class ButtonVariant { Primary, Tonal, Secondary, Text, Danger }
enum class StatusTone { Neutral, Info, Success, Warning, Error }
enum class Theme(val tokenName: String) { Light("light"), Dark("dark") }
enum class Density(val tokenName: String) { Compact("compact"), Comfortable("comfortable") }
enum class StackDirection { Vertical, Horizontal, Wrap }
enum class SurfaceTone { Surface, Container, Raised }
enum class ContainerWidth { Reading, Wide, Fluid }

/** Explicit inventory: a registry addition needs a real example before it can pass parity. */
enum class PreviewExample(val id: String) {
    UiRoot("ui-root"),
    Button("button"),
    IconButton("icon-button"),
    FilterChip("filter-chip"),
    Link("link"),
    NavItem("nav-item"),
    NavTabs("nav-tabs"),
    TextField("text-field"),
    TextArea("text-area"),
    NumberField("number-field"),
    Select("select"),
    Checkbox("checkbox"),
    Radio("radio"),
    Switch("switch"),
    RangeField("range-field"),
    FieldLabel("field-label"),
    FieldHelp("field-help"),
    FieldError("field-error"),
    Badge("badge"),
    Status("status"),
    LoadingState("loading-state"),
    EmptyState("empty-state"),
    ErrorState("error-state"),
    Surface("surface"),
    Stack("stack"),
    Grid("grid"),
    Container("container"),
    Dialog("dialog"),
    Icon("icon");

    companion object {
        fun fromId(id: String): PreviewExample? = entries.firstOrNull { it.id == id }
    }
}

fun filterComponents(query: String, category: String?): List<PreviewComponent> {
    val normalized = query.trim().lowercase()
    return PreviewCatalog.components.filter { component ->
        (category == null || component.category == category) &&
            (normalized.isEmpty() || component.name.lowercase().contains(normalized) ||
                component.id.contains(normalized) || component.category.contains(normalized))
    }
}

/** Read-only HTML is not a supported native choice-control mode. */
fun nativeChoiceState(state: FieldState): ButtonState = when (state) {
    FieldState.Editable -> ButtonState.Enabled
    FieldState.ReadOnly -> ButtonState.Disabled(DisabledReason.of("Read-only is not a native choice mode"))
    is FieldState.Disabled -> ButtonState.Disabled(state.reason)
    is FieldState.Loading -> ButtonState.Loading(state.reason)
}
