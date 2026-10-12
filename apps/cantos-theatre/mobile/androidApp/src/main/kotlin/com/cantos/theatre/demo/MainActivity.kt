package com.cantos.theatre.demo

import android.graphics.Color
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.cantos.theatre.app.CantosDemoApp
import com.cantos.ui.generated.CantosTokens

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        updateSystemBars(dark = false)
        setContent {
            CantosDemoApp(
                appContainer = (application as CantosDemoApplication).container,
                onThemeChanged = ::updateSystemBars,
                backHandler = { enabled, onBack ->
                    BackHandler(enabled = enabled, onBack = onBack)
                },
            )
        }
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
