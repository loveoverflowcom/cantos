package com.cantos.theatre.demo

import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.cantos.theatre.app.CantosDemoApp
import com.cantos.theatre.app.NativeIdentityApp
import com.cantos.theatre.adapters.identity.parseAndroidCallback
import com.cantos.ui.generated.CantosTokens

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        updateSystemBars(dark = false)
        deliverNativeCallback(intent)
        setContent {
            val native = (application as CantosDemoApplication).nativeIdentity
            if (native != null) NativeIdentityApp(native) else CantosDemoApp(
                appContainer = (application as CantosDemoApplication).container,
                onThemeChanged = ::updateSystemBars,
                backHandler = { enabled, onBack ->
                    BackHandler(enabled = enabled, onBack = onBack)
                },
            )
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
