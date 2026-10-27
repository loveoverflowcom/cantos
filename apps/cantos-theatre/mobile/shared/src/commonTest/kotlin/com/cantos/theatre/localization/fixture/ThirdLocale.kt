// Generated text-only test projection; not release resources or native formatter evidence.
package com.cantos.theatre.localization.fixture

/** UI resources only; content BCP-47 tags never pass through this registry. */
enum class UiLocale(val tag: String, val autonym: String, val direction: String) {
    Vietnamese("vi-VN", "Tiếng Việt", "ltr"),
    English("en", "English", "ltr"),
    Japanese("ja", "日本語", "ltr"),
    ;

    companion object {
        val Default: UiLocale = Vietnamese
    }
}

/** Canonical persisted values only: no OS negotiation or silent string-to-English mapping. */
fun savedUiLocale(tag: String): UiLocale? = UiLocale.entries.firstOrNull { it.tag == tag }
