package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.PreviewLocalePreferenceStore
import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.application.preferences.PreferencesUpdate
import com.cantos.theatre.ports.LocalePreferenceRead
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest

@OptIn(ExperimentalCoroutinesApi::class)
class PreviewLocalePreferenceTest {
    @Test
    fun injected_locale_store_restores_in_a_new_mount_without_persisting_domain_state() = runTest {
        val store = PreviewLocalePreferenceStore()
        val first = PreviewHarness(backgroundScope, localeStore = store)
        runCurrent()
        assertEquals(PreferencesUpdate.Applied, first.owners.preferences.setLocale(PreferenceLocale.English))
        first.close()
        val reopened = PreviewHarness(backgroundScope, localeStore = store)
        runCurrent()
        assertEquals(PreferenceLocale.English, reopened.owners.preferences.state.value.locale)
        assertEquals(LocalePreferenceRead.Saved(PreferenceLocale.English), reopened.owners.preferences.initialLocaleRead)
        reopened.close()
        // This is an injected-memory mount test, never native reopen or OS process-death evidence.
    }

    @Test
    fun locale_changes_and_pre_acceptance_refusals_preserve_other_application_authorities() = runTest {
        val store = PreviewLocalePreferenceStore()
        val h = PreviewHarness(backgroundScope, localeStore = store)
        runCurrent()
        val playback = h.owners.runtime.listening.playback.state.value
        val library = h.shell.library.value
        val navigation = h.shell.navigation.value
        val account = h.account.state.value
        assertEquals(PreferencesUpdate.Applied, h.owners.preferences.setLocale(PreferenceLocale.English))
        store.writesAvailable = false
        assertEquals(PreferencesUpdate.LocaleStoreUnavailable, h.owners.preferences.setLocale(PreferenceLocale.Vietnamese))
        assertEquals(PreferenceLocale.English, h.owners.preferences.state.value.locale)
        assertEquals(playback, h.owners.runtime.listening.playback.state.value)
        assertEquals(library, h.shell.library.value)
        assertEquals(navigation, h.shell.navigation.value)
        assertEquals(account, h.account.state.value)
        h.close()
    }
}
