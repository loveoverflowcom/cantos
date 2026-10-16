package com.cantos.theatre.app

import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.presentation.model.ShellLocale

/** Read-only presentation projection of the single application preference authority. */
fun PreferenceLocale.toShellLocale(): ShellLocale = when (this) {
    PreferenceLocale.Vietnamese -> ShellLocale.Vietnamese
    PreferenceLocale.English -> ShellLocale.English
}
