package com.cantos.ui

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
