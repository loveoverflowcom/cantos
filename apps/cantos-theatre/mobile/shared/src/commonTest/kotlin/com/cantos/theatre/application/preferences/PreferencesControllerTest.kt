package com.cantos.theatre.application.preferences

import com.cantos.theatre.domain.preferences.UiLocale
import com.cantos.theatre.domain.preferences.savedUiLocale
import com.cantos.theatre.ports.LocalePreferenceRead
import com.cantos.theatre.ports.LocalePreferenceStore
import com.cantos.theatre.ports.LocalePreferenceWrite
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

    @Test
    fun saved_locale_wins_over_host_seed_without_changing_the_host_theme() {
        val store = TestLocaleStore(LocalePreferenceRead.Saved(PreferenceLocale.English))
        val controller = PreferencesController(PreferencesState(PreferenceTheme.Dark), store)
        assertEquals(PreferencesState(PreferenceTheme.Dark, PreferenceLocale.English), controller.state.value)
        assertEquals(LocalePreferenceRead.Saved(PreferenceLocale.English), controller.initialLocaleRead)
        assertEquals(emptyList(), store.writes)
    }

    @Test
    fun missing_corrupt_or_unavailable_preferences_use_the_explicit_seed() {
        val seed = PreferencesState(PreferenceTheme.Dark, PreferenceLocale.English)
        listOf(LocalePreferenceRead.Unset, LocalePreferenceRead.Corrupt, LocalePreferenceRead.Unavailable).forEach { read ->
            val store = TestLocaleStore(read)
            val controller = PreferencesController(seed, store)
            assertEquals(seed, controller.state.value, "Fallback for $read")
            assertEquals(read, controller.initialLocaleRead)
            assertEquals(emptyList(), store.writes, "Restore never overwrites an unavailable/corrupt preference")
        }
        assertEquals(PreferenceLocale.Vietnamese, PreferencesController().state.value.locale)
    }

    @Test
    fun accepted_language_change_restores_from_the_store_and_theme_stays_memory_only() {
        val store = TestLocaleStore()
        val controller = PreferencesController(localePreferenceStore = store)
        assertEquals(PreferencesUpdate.Applied, controller.setTheme(PreferenceTheme.Dark))
        assertEquals(emptyList(), store.writes)
        assertEquals(PreferencesUpdate.Applied, controller.setLocale(PreferenceLocale.English))
        assertEquals(listOf(PreferenceLocale.English), store.writes)
        assertEquals(PreferencesState(PreferenceTheme.Dark, PreferenceLocale.English), controller.state.value)
        assertEquals(PreferencesState(PreferenceTheme.Light, PreferenceLocale.English), PreferencesController(localePreferenceStore = store).state.value)
    }

    @Test
    fun pre_acceptance_store_refusal_keeps_the_entire_published_state_and_can_be_retried() {
        val initial = PreferencesState(PreferenceTheme.Dark, PreferenceLocale.Vietnamese)
        val store = TestLocaleStore().also { it.writeResult = LocalePreferenceWrite.Unavailable }
        lateinit var controller: PreferencesController
        store.onWrite = { assertEquals(initial, controller.state.value, "Publish follows the storage result") }
        controller = PreferencesController(initial, store)
        assertEquals(PreferencesUpdate.LocaleStoreUnavailable, controller.setLocale(PreferenceLocale.English))
        assertEquals(PreferencesUpdate.LocaleStoreUnavailable, controller.localeUpdate.value)
        assertEquals(initial, controller.state.value)
        store.writeResult = LocalePreferenceWrite.Accepted
        assertEquals(PreferencesUpdate.Applied, controller.setLocale(PreferenceLocale.English))
        assertEquals(PreferencesUpdate.Applied, controller.localeUpdate.value)
        assertEquals(initial.copy(locale = PreferenceLocale.English), controller.state.value)
    }

    @Test
    fun preference_reset_requests_the_default_locale_and_refusal_keeps_controller_state() {
        val initial = PreferencesState(PreferenceTheme.Dark, PreferenceLocale.English)
        val store = TestLocaleStore().also { it.writeResult = LocalePreferenceWrite.Unavailable }
        val controller = PreferencesController(initial, store)
        assertEquals(PreferencesUpdate.LocaleStoreUnavailable, controller.reset())
        assertEquals(initial, controller.state.value)
        store.writeResult = LocalePreferenceWrite.Accepted
        assertEquals(PreferencesUpdate.Applied, controller.reset())
        assertEquals(PreferencesState(), controller.state.value)
        assertEquals(PreferencesState(), PreferencesController(localePreferenceStore = store).state.value)
    }

    @Test
    fun saved_locale_tags_are_explicit_and_never_negotiate_the_os_language() {
        assertEquals(UiLocale.Vietnamese, savedUiLocale("vi-VN"))
        assertEquals(UiLocale.English, savedUiLocale("en"))
        listOf("", "vi", "en-US", "en-GB", "VI-vn", "fr", "en ", "null").forEach { tag ->
            assertEquals(null, savedUiLocale(tag), "Unreviewed saved locale $tag")
        }
    }
}

private class TestLocaleStore(
    private var readResult: LocalePreferenceRead = LocalePreferenceRead.Unset,
) : LocalePreferenceStore {
    val writes = mutableListOf<UiLocale>()
    var writeResult = LocalePreferenceWrite.Accepted
    var onWrite: () -> Unit = {}
    override fun readLocale(): LocalePreferenceRead = readResult
    override fun writeLocale(locale: UiLocale): LocalePreferenceWrite {
        onWrite()
        writes += locale
        if (writeResult == LocalePreferenceWrite.Accepted) readResult = LocalePreferenceRead.Saved(locale)
        return writeResult
    }
}
