package com.cantos.theatre.app

import android.content.res.Configuration
import android.graphics.Color
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import com.cantos.theatre.adapters.preferences.AndroidLocalePreferenceStore
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.features.launch.presentation.CantosListeningUnavailable
import com.cantos.theatre.features.launch.presentation.CantosLocalLaunch
import com.cantos.ui.ThemeMode
import com.cantos.ui.UiPreferences
import com.cantos.ui.generated.CantosTokens
import com.cantos.ui.resolve

/**
 * The Cantos Theatre application: the listener experience over the live catalog, account,
 * durable Library and native player. The API is the packaged override or the production default;
 * a refused configuration shows its typed diagnostic and never a sample.
 */
class TheatreActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // The process owners initialize locally and synchronously. Never hold this window for
        // catalog/identity IO or an animation, and never add a second splash Activity.
        installSplashScreen()
        super.onCreate(savedInstanceState)
        // A uiMode change recreates this Activity, so the resolved theme never goes stale.
        val dark = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
        updateSystemBars(dark)
        val resolved = liveCatalogOrigin()
        if (resolved is HostOrigin.Ready) {
            val owner = theatreOwner(resolved.origin, dark)
            setContent {
                val preferences by owner.preferences.state.collectAsState()
                CantosLocalLaunch(localReady = true, preferences = UiPreferences(), locale = preferences.locale.toShellLocale()) {
                    CantosTheatreApp(
                        owner.shell,
                        onThemeChanged = ::updateSystemBars,
                        backHandler = { enabled, onBack -> BackHandler(enabled = enabled, onBack = onBack) },
                        cache = owner.cache,
                    )
                }
            }
            return
        }
        val ui = UiPreferences(theme = ThemeMode.System.resolve(systemIsDark = dark))
        val locale = PreferencesController(localePreferenceStore = AndroidLocalePreferenceStore(this))
            .state.value.locale.toShellLocale()
        setContent {
            CantosListeningUnavailable(ui, locale = locale, fault = (resolved as HostOrigin.Refused).fault)
        }
    }

    override fun onStop() {
        ProductionProcess.current()?.runtime?.backgrounded()
        super.onStop()
    }

    private fun updateSystemBars(dark: Boolean) {
        val lightScrim = Color.parseColor(CantosTokens.color("color.background", "light"))
        val darkScrim = Color.parseColor(CantosTokens.color("color.background", "dark"))
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT) { dark },
            // Before API 26, navigation-bar dark icons are unavailable. The auto style retains
            // the canonical dark scrim there, instead of white icons over the light app surface.
            navigationBarStyle = SystemBarStyle.auto(lightScrim, darkScrim) { dark },
        )
    }
}
