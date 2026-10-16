package com.cantos.theatre.demo

import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import android.content.res.Configuration
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import com.cantos.theatre.app.toShellLocale
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import com.cantos.theatre.app.CantosDemoApp
import com.cantos.theatre.adapters.identity.parseAndroidCallback
import com.cantos.ui.generated.CantosTokens
import com.cantos.theatre.features.launch.presentation.CantosLocalLaunch
import com.cantos.ui.UiPreferences

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // The process owners initialize locally and synchronously. Never hold this window
        // for catalog/identity IO or an animation, and never add a second splash Activity.
        installSplashScreen()
        super.onCreate(savedInstanceState)
        updateSystemBars(dark = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES)
        deliverNativeCallback(intent)
        // One guest shell. The process-owned container already carries the optional account
        // owner, so identity never replaces Home and system Back means the same everywhere.
        val container = (application as CantosDemoApplication).container
        setContent {
            // Both local constructors have completed. Remote identity/catalog work belongs
            // to the destination and cannot turn this back into an opening presentation.
            val preferences by container.preferences.state.collectAsState()
            CantosLocalLaunch(localReady = true, preferences = UiPreferences(), locale = preferences.locale.toShellLocale()) {
                CantosDemoApp(
                    appContainer = container,
                    onThemeChanged = ::updateSystemBars,
                    backHandler = { enabled, onBack ->
                        BackHandler(enabled = enabled, onBack = onBack)
                    },
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        deliverNativeCallback(intent)
    }

    private fun deliverNativeCallback(intent: Intent?) {
        val uri = intent?.data ?: return
        if (uri.scheme != "com.cantos.theatre") return
        intent.data = null
        val callback = parseAndroidCallback(uri) ?: return
        (application as CantosDemoApplication).nativeIdentity?.callback(callback)
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
