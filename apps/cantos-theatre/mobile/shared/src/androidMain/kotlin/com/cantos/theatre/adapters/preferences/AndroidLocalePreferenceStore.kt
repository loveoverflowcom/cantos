package com.cantos.theatre.adapters.preferences

import android.content.Context
import com.cantos.theatre.domain.preferences.UiLocale
import com.cantos.theatre.domain.preferences.savedUiLocale
import com.cantos.theatre.ports.LocalePreferenceRead
import com.cantos.theatre.ports.LocalePreferenceStore
import com.cantos.theatre.ports.LocalePreferenceWrite

/** A private nonsecret preference, separate from identity and the domain cache. */
class AndroidLocalePreferenceStore(context: Context) : LocalePreferenceStore {
    private val applicationContext = context.applicationContext

    override fun readLocale(): LocalePreferenceRead = try {
        val preferences = applicationContext.getSharedPreferences(FILE, Context.MODE_PRIVATE)
        if (!preferences.contains(KEY)) LocalePreferenceRead.Unset
        else preferences.getString(KEY, null)?.let(::savedUiLocale)?.let(LocalePreferenceRead::Saved)
            ?: LocalePreferenceRead.Corrupt
    } catch (_: ClassCastException) {
        LocalePreferenceRead.Corrupt
    } catch (_: SecurityException) {
        LocalePreferenceRead.Unavailable
    } catch (_: IllegalStateException) {
        LocalePreferenceRead.Unavailable
    }

    override fun writeLocale(locale: UiLocale): LocalePreferenceWrite = try {
        // apply updates the platform cache and schedules disk persistence without a failure
        // callback. Acceptance never means a forced disk flush or process-death durability.
        applicationContext.getSharedPreferences(FILE, Context.MODE_PRIVATE)
            .edit().putString(KEY, locale.tag).apply()
        LocalePreferenceWrite.Accepted
    } catch (_: SecurityException) {
        LocalePreferenceWrite.Unavailable
    } catch (_: IllegalStateException) {
        LocalePreferenceWrite.Unavailable
    }

    private companion object {
        const val FILE = "cantos-ui-preferences-v1"
        const val KEY = "ui-locale"
    }
}
