package com.cantos.theatre.app

import android.content.Intent
import android.content.res.Configuration
import com.cantos.theatre.adapters.identity.parseAndroidCallback
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

/** Production catalog, account, SQLite/outbox and native streaming composition. */
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
            deliverCallback(intent)
            return
        }
        setContent { CantosListeningUnavailable(preferences, locale = locale) }
    }
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        deliverCallback(intent)
    }

    private fun deliverCallback(intent: Intent?) {
        val uri = intent?.data ?: return
        if (uri.scheme != "com.cantos.theatre") return
        intent.data = null
        val callback = parseAndroidCallback(uri) ?: return
        ProductionProcess.current()?.runtime?.account?.callback(callback)
    }

    override fun onStop() {
        ProductionProcess.current()?.runtime?.backgrounded()
        super.onStop()
    }
}
