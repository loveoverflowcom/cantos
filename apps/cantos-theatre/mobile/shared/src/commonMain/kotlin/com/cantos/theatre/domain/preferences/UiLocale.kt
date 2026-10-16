package com.cantos.theatre.domain.preferences

/** Application UI language only; source titles and dialogue keep their original language. */
enum class UiLocale(val tag: String) {
    Vietnamese("vi-VN"),
    English("en"),
}

/** Only canonical persisted values are accepted; corrupt/old values never become a new locale. */
fun savedUiLocale(tag: String): UiLocale? = when (tag) {
    "vi-VN" -> UiLocale.Vietnamese
    "en" -> UiLocale.English
    else -> null
}
