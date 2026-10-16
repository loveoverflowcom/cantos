package com.cantos.theatre.app

import android.content.res.Configuration
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.cantos.theatre.adapters.preferences.AndroidLocalePreferenceStore
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.features.launch.presentation.CantosListeningUnavailable
import com.cantos.ui.ThemeMode
import com.cantos.ui.UiPreferences
import com.cantos.ui.resolve

/**
 * Production composition root: `:shared` plus production adapters only, with no mock, sample or
 * demo fallback. When [liveCatalogOrigin] admits a configured API origin (see LiveCatalogHost),
 * it composes the live anonymous public catalog: browse only, since no production media engine
 * or progress adapter exists (#14). Otherwise it renders the explicit unavailable state.
 */
class TheatreActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // A uiMode change recreates this Activity, so the resolved theme never goes stale.
        val dark = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
        val preferences = UiPreferences(theme = ThemeMode.System.resolve(systemIsDark = dark))
        val locale = PreferencesController(localePreferenceStore = AndroidLocalePreferenceStore(this))
            .state.value.locale.toShellLocale()
        val origin = liveCatalogOrigin()
        if (origin != null) {
            setLiveCatalogContent(origin, preferences, locale)
            return
        }
        setContent { CantosListeningUnavailable(preferences, locale = locale) }
    }
}
