package com.cantos.theatre.ports

import com.cantos.theatre.domain.preferences.UiLocale

sealed interface LocalePreferenceRead {
    data object Unset : LocalePreferenceRead
    data class Saved(val locale: UiLocale) : LocalePreferenceRead
    data object Corrupt : LocalePreferenceRead
    data object Unavailable : LocalePreferenceRead
}

/** Accepted by the platform settings API; asynchronous disk durability is not acknowledged. */
enum class LocalePreferenceWrite { Accepted, Unavailable }

/** One nonsecret UI preference. Domain cache, credentials and source content never enter it.
 * Reads may use the platform cache. Unavailable is a refusal before API acceptance, not a
 * promise to observe later disk failure or to roll back an already accepted native write.
 */
interface LocalePreferenceStore {
    fun readLocale(): LocalePreferenceRead
    fun writeLocale(locale: UiLocale): LocalePreferenceWrite
}
