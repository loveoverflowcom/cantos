package com.cantos.theatre.adapters.mock

import com.cantos.theatre.domain.preferences.UiLocale
import com.cantos.theatre.ports.LocalePreferenceRead
import com.cantos.theatre.ports.LocalePreferenceStore
import com.cantos.theatre.ports.LocalePreferenceWrite

/** Injectable diagnostic memory, excluded with :preview-sandbox from production release. */
class MockLocalePreferenceStore(
    initial: LocalePreferenceRead = LocalePreferenceRead.Unset,
) : LocalePreferenceStore {
    var readResult: LocalePreferenceRead = initial
    var writesAvailable: Boolean = true
    override fun readLocale(): LocalePreferenceRead = readResult
    override fun writeLocale(locale: UiLocale): LocalePreferenceWrite {
        if (!writesAvailable) return LocalePreferenceWrite.Unavailable
        readResult = LocalePreferenceRead.Saved(locale)
        return LocalePreferenceWrite.Accepted
    }
}
