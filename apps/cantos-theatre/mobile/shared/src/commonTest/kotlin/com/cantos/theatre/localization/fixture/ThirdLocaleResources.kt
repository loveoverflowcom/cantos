// Generated text-only test projection; not release resources or native formatter evidence.
package com.cantos.theatre.localization.fixture

import com.cantos.theatre.localization.fixture.UiLocale

enum class ProbeKey {
    Language,
    Unavailable,
    ;

    fun text(locale: UiLocale): String = when (this) {
        Language -> when (locale) {
            UiLocale.Vietnamese -> "Ngôn ngữ"
            UiLocale.English -> "Language"
            UiLocale.Japanese -> "言語"
        }
        Unavailable -> when (locale) {
            UiLocale.Vietnamese -> "Không thể phát nội dung này."
            UiLocale.English -> "This content cannot be played."
            UiLocale.Japanese -> "このコンテンツは再生できません。"
        }
    }
}

sealed interface ProbeMessage {
    fun text(locale: UiLocale): String

    data class Bookmark(val time: String, val title: String) : ProbeMessage {
        override fun text(locale: UiLocale): String = when (locale) {
            UiLocale.Vietnamese -> "" + "Nghe " + title + " tại " + time
            UiLocale.English -> "" + "Listen to " + title + " at " + time
            UiLocale.Japanese -> "" + time + "に" + title + "を聴く"
        }
    }

}
