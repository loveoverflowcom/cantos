package com.cantos.theatre.app

import com.cantos.theatre.adapters.mock.MockLocalePreferenceStore
import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.application.preferences.PreferencesUpdate
import com.cantos.theatre.ports.LocalePreferenceRead
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals

@OptIn(ExperimentalCoroutinesApi::class)
class LocalePreferenceSessionTest {
    @Test
    fun injected_locale_store_restores_in_a_new_mock_mount_without_persisting_domain_state() = runTest {
        val store = MockLocalePreferenceStore()
        val first = mockAppContainer(this, localePreferenceStore = store)
        runCurrent()
        assertEquals(PreferencesUpdate.Applied, first.preferences.setLocale(PreferenceLocale.English))
        first.close()
        val reopened = mockAppContainer(this, localePreferenceStore = store)
        runCurrent()
        assertEquals(PreferenceLocale.English, reopened.preferences.state.value.locale)
        assertEquals(LocalePreferenceRead.Saved(PreferenceLocale.English), reopened.preferences.initialLocaleRead)
        reopened.close()
        // This is an injected-memory mount test, never native reopen or OS process-death evidence.
    }

    @Test
    fun locale_changes_and_pre_acceptance_refusals_preserve_other_application_authorities() = runTest {
        val store = MockLocalePreferenceStore()
        val app = mockAppContainer(this, localePreferenceStore = store)
        runCurrent()
        app.bookmark()
        runCurrent()
        val playback = app.playback.state.value
        val listening = app.listening.state.value
        val navigation = app.navigation.value
        val account = app.accountState.value
        assertEquals(PreferencesUpdate.Applied, app.preferences.setLocale(PreferenceLocale.English))
        store.writesAvailable = false
        assertEquals(PreferencesUpdate.LocaleStoreUnavailable, app.preferences.setLocale(PreferenceLocale.Vietnamese))
        assertEquals(PreferenceLocale.English, app.preferences.state.value.locale)
        assertEquals(playback, app.playback.state.value)
        assertEquals(listening, app.listening.state.value)
        assertEquals(navigation, app.navigation.value)
        assertEquals(account, app.accountState.value)
        app.close()
    }
}
