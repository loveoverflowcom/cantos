package com.cantos.theatre.application.preferences

import com.cantos.theatre.domain.preferences.UiLocale
import com.cantos.theatre.ports.LocalePreferenceRead
import com.cantos.theatre.ports.LocalePreferenceStore
import com.cantos.theatre.ports.LocalePreferenceWrite
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

enum class PreferenceTheme { Light, Dark, System }

typealias PreferenceLocale = UiLocale

data class PreferencesState(
    val theme: PreferenceTheme = PreferenceTheme.Light,
    val locale: PreferenceLocale = PreferenceLocale.Default,
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

enum class PreferencesUpdate { Applied, LocaleStoreUnavailable }

/** Saved locale > explicit host/preview seed > Vietnamese default. Theme stays memory-only.
 * A null store explicitly selects a session-only owner; the core never constructs a mock store.
 * Callers confine this owner to the same application dispatcher as the other app authorities.
 */
class PreferencesController(
    initialState: PreferencesState = PreferencesState(),
    private val localePreferenceStore: LocalePreferenceStore? = null,
) {
    val initialLocaleRead: LocalePreferenceRead = localePreferenceStore?.readLocale() ?: LocalePreferenceRead.Unset
    private val mutableState = MutableStateFlow(
        when (val restored = initialLocaleRead) {
            is LocalePreferenceRead.Saved -> initialState.copy(locale = restored.locale)
            LocalePreferenceRead.Unset, LocalePreferenceRead.Corrupt, LocalePreferenceRead.Unavailable -> initialState
        },
    )
    val state: StateFlow<PreferencesState> = mutableState.asStateFlow()
    private val mutableLocaleUpdate = MutableStateFlow(PreferencesUpdate.Applied)
    val localeUpdate: StateFlow<PreferencesUpdate> = mutableLocaleUpdate.asStateFlow()

    fun dispatch(event: PreferencesEvent): PreferencesUpdate {
        val locale = when (event) {
            is PreferencesEvent.LocaleChanged -> event.locale
            PreferencesEvent.Reset -> PreferencesState().locale
            is PreferencesEvent.ThemeChanged -> null
        }
        // Never perform storage in StateFlow.update: its pure transformation may be retried.
        if (locale != null && localePreferenceStore?.writeLocale(locale) == LocalePreferenceWrite.Unavailable) {
            mutableLocaleUpdate.value = PreferencesUpdate.LocaleStoreUnavailable
            return PreferencesUpdate.LocaleStoreUnavailable
        }
        mutableState.update { reducePreferences(it, event) }
        if (locale != null) mutableLocaleUpdate.value = PreferencesUpdate.Applied
        return PreferencesUpdate.Applied
    }

    fun setTheme(theme: PreferenceTheme) = dispatch(PreferencesEvent.ThemeChanged(theme))
    fun setLocale(locale: PreferenceLocale) = dispatch(PreferencesEvent.LocaleChanged(locale))
    fun reset() = dispatch(PreferencesEvent.Reset)
}
