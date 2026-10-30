package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import java.io.File
import java.security.MessageDigest
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/** Actual production composition; only external ports are synthetic. Never native acceptance. */
@OptIn(ExperimentalTestApi::class)
class SessionExpiryCaptureTest {
    @Test
    fun confirmed_loss_on_public_discovery_is_captured() {
        capture("discovery-expired-vi-dark-390", 390, ShellTheme.Dark, ShellLocale.Vietnamese)
        capture("discovery-expired-en-light-430", 430, ShellTheme.Light, ShellLocale.English)
        capture("discovery-expired-vi-light-text2-320", 320, ShellTheme.Light, ShellLocale.Vietnamese, 2f)
    }

    @Test fun contextual_dialog_is_captured_across_locales_themes_widths_and_large_text() {
        capture("dialog-expired-vi-dark-390", 390, ShellTheme.Dark, ShellLocale.Vietnamese, modal = true)
        capture("dialog-expired-en-light-430", 430, ShellTheme.Light, ShellLocale.English, modal = true)
        capture("dialog-expired-vi-light-text2-320", 320, ShellTheme.Light, ShellLocale.Vietnamese, 2f, modal = true, height = 568)
        capture("dialog-expired-en-dark-text2-320", 320, ShellTheme.Dark, ShellLocale.English, 2f, modal = true, height = 568)
    }

    private fun capture(name: String, width: Int, theme: ShellTheme, locale: ShellLocale, scale: Float = 1f, modal: Boolean = false, height: Int = 900) {
        runDesktopComposeUiTest(width = width, height = height) {
            lateinit var owners: PreviewOwners
            var rendererDensity = Float.NaN
            setContent {
                rendererDensity = LocalDensity.current.density
                Box(Modifier.requiredSize(width.dp, height.dp).testTag("expiry-capture-root")) {
                    PreviewTestRoot(PreviewSeed(previewState(theme = theme, locale = locale), signedIn = true),
                        textScale = scale, onOwners = { owners = it })
                }
            }
            waitUntil { owners.account.state.value is NativeAccountState.SignedIn }
            runOnIdle {
                owners.identity.sessionFailure = IdentityV1Failure.Unauthorized
                owners.account.revalidate(requireNotNull(owners.account.verifiedSession.value).session.session)
            }
            waitUntil { owners.account.state.value is NativeAccountState.Failed }
            waitForIdle()
            if (modal) runOnIdle { owners.shell.dispatch(ShellEvent.Navigate(ShellRoute.Library)) }
            waitForIdle()
            val target = if (modal) "error-dialog" else "expiry-capture-root"
            val output = File(System.getenv("CANTOS_SESSION_CAPTURE_DIR") ?: "build/reports/session-expiry")
            check(output.isDirectory || output.mkdirs())
            val bitmap = onNodeWithTag(target).captureToImage()
            val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
            File(output, "$name.png").writeBytes(bytes)
            File(output, "$name.semantics.txt").writeText(onNodeWithTag(target, useUnmergedTree = true).printToString())
            val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
            File(output, "$name.json").writeText("""
                {"renderer":"JVM Desktop Compose UI test","entrypoint":"CantosTheatreApp",
                 "widthDp":$width,"heightDp":$height,"density":$rendererDensity,"captureTarget":"$target","textScale":$scale,
                 "theme":"$theme","locale":"$locale","motion":"Reduced",
                 "scenario":"API-confirmed session loss after signed-in restore; private Library intent only when modal=$modal",
                 "png_sha256":"$hash","evidence":"screenshot-captured; inspection pending"}
            """.trimIndent())
        }
    }
}
