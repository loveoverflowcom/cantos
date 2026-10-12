package com.cantos.theatre.application.preferences

import kotlin.test.Test
import kotlin.test.assertEquals

class PreferencesControllerTest {
    @Test
    fun preferences_have_a_pure_reducer_and_one_memory_authority() {
        val initial = PreferencesState()
        val dark = initial.copy(theme = PreferenceTheme.Dark)
        assertEquals(dark, reducePreferences(initial, PreferencesEvent.ThemeChanged(PreferenceTheme.Dark)))
        val controller = PreferencesController()
        controller.setTheme(PreferenceTheme.Dark)
        controller.setLocale(PreferenceLocale.English)
        assertEquals(PreferencesState(PreferenceTheme.Dark, PreferenceLocale.English), controller.state.value)
        controller.reset()
        assertEquals(initial, controller.state.value)
    }
}
