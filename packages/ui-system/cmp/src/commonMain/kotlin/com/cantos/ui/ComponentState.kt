package com.cantos.ui

/** A disabled action cannot exist without a human-readable explanation. */
class DisabledReason private constructor(val text: String) {
    // Reasons are immutable values; rebuilding equivalent caller state must not reset UI state.
    override fun equals(other: Any?): Boolean = other is DisabledReason && text == other.text

    override fun hashCode(): Int = text.hashCode()

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

enum class ButtonVariant { Primary, Tonal, Secondary, Text, Danger, AuthPrimary, AuthSecondary, AuthOutlined }
/**
 * Floating (Fields v3/v4) keeps the label inside one tonal shell with one inset state perimeter and
 * is the only variant whose trailing slot is part of its anatomy. Regular and AuthTonal keep
 * their existing geometry for their current consumers. FloatingSearch is the same anatomy as a
 * stadium with a 72-unit minimum, the shape of a search field that docks its action in the trailing
 * slot; the host still decides what, if anything, a query commits. FloatingTonal (Auth v4.3) is the
 * Floating anatomy with no stroke of any kind: idle, focus and error are tonal fills, a primary
 * label and caret, and an inline icon + message, so no state is carried by a border or halo.
 */
enum class TextFieldVariant { Regular, AuthTonal, Floating, FloatingSearch, FloatingTonal }
enum class StatusTone { Neutral, Info, Success, Warning, Error }
enum class Theme(val tokenName: String) { Light("light"), Dark("dark") }
enum class Density(val tokenName: String) { Compact("compact"), Comfortable("comfortable") }

/**
 * An opt-in remap of semantic roles for one subtree (token v0.7.0). Standard remaps nothing, so
 * Studio and any consumer that names no expression resolve exactly the roles they always did.
 * [tokenName] is the generated `CantosTokens.expressionNames` entry.
 */
enum class CantosExpression(val tokenName: String) { Standard("standard"), Theatre("theatre") }

/**
 * The appearance a person chooses. Only a composition root resolves [System], from a dark-mode
 * fact it was given; components, nested themes and [UiPreferences] see the resolved [Theme] and
 * never read the operating system.
 */
enum class ThemeMode { Light, Dark, System }

/** Pure: [ThemeMode.System] follows the supplied host fact; an explicit mode ignores it. */
fun ThemeMode.resolve(systemIsDark: Boolean): Theme = when (this) {
    ThemeMode.Light -> Theme.Light
    ThemeMode.Dark -> Theme.Dark
    ThemeMode.System -> if (systemIsDark) Theme.Dark else Theme.Light
}

/** Pure cycle transition: Dark → Light → System → Dark. */
fun ThemeMode.cycle(): ThemeMode = when (this) {
    ThemeMode.Dark -> ThemeMode.Light
    ThemeMode.Light -> ThemeMode.System
    ThemeMode.System -> ThemeMode.Dark
}
enum class StackDirection { Vertical, Horizontal, Wrap }
enum class SurfaceTone { Surface, Container, Raised, Reading, PrimaryContainer }
enum class SurfaceVariant { Regular, Auth, Profile, Membership, AccountGroup }
enum class ContainerWidth { Reading, Wide, Fluid }

/** Shared semantic spacing names; values stay in the generated token source. */
enum class Space { None, Xs, Sm, Md, Lg }
enum class Announcement { Off, Polite, Assertive }

/** Stable option identity is distinct from localized display text and caller-owned selection. */
data class SelectOption(
    val value: String,
    val label: String,
    val state: ButtonState = ButtonState.Enabled,
    val icon: String? = null,
) {
    init {
        require(value.isNotBlank()) { "Option values must be nonblank" }
        require(label.isNotBlank()) { "Option labels must be nonblank" }
        icon?.let { com.cantos.ui.generated.CantosIcons.paths(it) }
    }
}
