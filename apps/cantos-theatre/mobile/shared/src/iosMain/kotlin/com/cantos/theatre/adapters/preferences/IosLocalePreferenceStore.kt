package com.cantos.theatre.adapters.preferences

import com.cantos.theatre.domain.preferences.UiLocale
import com.cantos.theatre.domain.preferences.savedUiLocale
import com.cantos.theatre.ports.LocalePreferenceRead
import com.cantos.theatre.ports.LocalePreferenceStore
import com.cantos.theatre.ports.LocalePreferenceWrite
import platform.Foundation.NSUserDefaults

/** App-owned language, never AppleLanguages or a global Foundation locale mutation. */
class IosLocalePreferenceStore(
    private val defaults: NSUserDefaults = NSUserDefaults.standardUserDefaults,
) : LocalePreferenceStore {
    override fun readLocale(): LocalePreferenceRead {
        val raw = defaults.objectForKey(KEY) ?: return LocalePreferenceRead.Unset
        val locale = (raw as? String)?.let(::savedUiLocale) ?: return LocalePreferenceRead.Corrupt
        return LocalePreferenceRead.Saved(locale)
    }

    override fun writeLocale(locale: UiLocale): LocalePreferenceWrite {
        defaults.setObject(locale.tag, forKey = KEY)
        // UserDefaults persists asynchronously; Apple discourages synchronize().
        return LocalePreferenceWrite.Accepted
    }

    private companion object {
        const val KEY = "com.cantos.theatre.ui-locale.v1"
    }
}
