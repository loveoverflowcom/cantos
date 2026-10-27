package com.cantos.theatre.features.auth.presentation

import com.cantos.theatre.presentation.model.ShellLocale

/** Existing integer display, pending qualified ICU4X Kotlin/native number/plural bindings. */
internal fun legacyRetryAfter(locale: ShellLocale, seconds: Long): String = when (locale) {
    ShellLocale.Vietnamese -> "Vui lòng đợi $seconds giây trước khi thử lại."
    ShellLocale.English -> "Please wait $seconds seconds before trying again."
}
