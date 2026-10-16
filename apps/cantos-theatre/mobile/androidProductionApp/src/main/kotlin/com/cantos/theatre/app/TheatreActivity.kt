package com.cantos.theatre.app

import android.content.res.Configuration
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.cantos.theatre.features.launch.presentation.CantosListeningUnavailable
import com.cantos.ui.ThemeMode
import com.cantos.ui.UiPreferences
import com.cantos.ui.resolve

/**
 * Production composition root: `:shared` plus production adapters only. No production catalog,
 * progress or media adapter exists yet (#14), so no [AppContainer] can be composed and this
 * host renders the explicit unavailable state. It has no mock, sample or demo fallback.
 */
class TheatreActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // A uiMode change recreates this Activity, so the resolved theme never goes stale.
        val dark = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
        val preferences = UiPreferences(theme = ThemeMode.System.resolve(systemIsDark = dark))
        setContent { CantosListeningUnavailable(preferences) }
    }
}
