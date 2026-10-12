package com.cantos.theatre.application.preferences

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

enum class PreferenceTheme { Light, Dark }

enum class PreferenceLocale { Vietnamese, English }

data class PreferencesState(
    val theme: PreferenceTheme = PreferenceTheme.Light,
    val locale: PreferenceLocale = PreferenceLocale.Vietnamese,
)

sealed interface PreferencesEvent {
    data class ThemeChanged(val theme: PreferenceTheme) : PreferencesEvent
    data class LocaleChanged(val locale: PreferenceLocale) : PreferencesEvent
    data object Reset : PreferencesEvent
}

fun reducePreferences(state: PreferencesState, event: PreferencesEvent): PreferencesState = when (event) {
    is PreferencesEvent.ThemeChanged -> state.copy(theme = event.theme)
    is PreferencesEvent.LocaleChanged -> state.copy(locale = event.locale)
    PreferencesEvent.Reset -> PreferencesState()
}

/** Memory-only theme/locale authority. Active speed and timer belong to playback. */
class PreferencesController(initialState: PreferencesState = PreferencesState()) {
    private val mutableState = MutableStateFlow(initialState)
    val state: StateFlow<PreferencesState> = mutableState.asStateFlow()

    fun dispatch(event: PreferencesEvent) {
        mutableState.update { reducePreferences(it, event) }
    }

    fun setTheme(theme: PreferenceTheme) = dispatch(PreferencesEvent.ThemeChanged(theme))
    fun setLocale(locale: PreferenceLocale) = dispatch(PreferencesEvent.LocaleChanged(locale))
    fun reset() = dispatch(PreferencesEvent.Reset)
}
