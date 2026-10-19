package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.PreviewLoginMode
import com.cantos.theatre.app.PreviewOwners
import com.cantos.theatre.app.PreviewSeed
import com.cantos.theatre.app.PreviewTestRoot
import com.cantos.theatre.app.previewState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellTheme
import java.io.File
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/**
 * Same-scenario JVM captures of the guest Account, Login, Register and signed-in Account. They are
 * rendered pixels plus semantics from the desktop renderer: qualitative inspection evidence only,
 * never Android/iOS layout, IME, OS Back, TalkBack/VoiceOver or system-browser acceptance.
 * Account states come from deterministic identity ports, not a provider.
 */
@OptIn(ExperimentalTestApi::class)
class AuthCaptureTest {
    private fun login(theme: ShellTheme = ShellTheme.Light, locale: ShellLocale = ShellLocale.Vietnamese) =
        PreviewSeed(previewState(stack = listOf(ShellRoute.Discovery, ShellRoute.Login), theme = theme, locale = locale))

    private fun PreviewSeed.themed(theme: ShellTheme) = copy(state = state.copy(theme = theme))

    private val account = PreviewSeed(previewState(stack = listOf(ShellRoute.Discovery, ShellRoute.Account)))

    @Test
    fun login_is_captured_across_widths_themes_languages_and_text_scales() {
        capture("login-light-390x844", 390, 844, login())
        capture("login-dark-390x844", 390, 844, login(ShellTheme.Dark))
        capture("login-light-en-390x844", 390, 844, login(locale = ShellLocale.English))
        capture("login-light-768x1024", 768, 1024, login())
        capture("login-dark-320x568", 320, 568, login(ShellTheme.Dark))
        capture("login-light-text2-320x800", 320, 800, login(), textScale = 2f)
        capture("login-light-text2-actions-320x800", 320, 800, login(), textScale = 2f, scrollToTag = "login-create")
    }

    @Test
    fun login_states_are_captured() {
        capture("login-pending-light-390x844", 390, 844, login()) { it.click("login-primary") }
        capture("login-failed-light-390x844", 390, 844, login()) { d ->
            d.click("login-primary")
            d.h.identity.returnFromBrowser(approve = false)
        }
        capture("login-cleanup-dark-390x844", 390, 844, login(ShellTheme.Dark)) { d ->
            d.click("login-primary")
            d.h.identity.returnFromBrowser(approve = true)
            d.h.identity.clearFails = true
            d.h.shell.logout()
            d.h.shell.openLogin()
        }
    }

    @Test
    fun registration_unavailable_is_captured() {
        val state = PreviewSeed(previewState(stack = listOf(ShellRoute.Discovery, ShellRoute.Login, ShellRoute.Register)))
        capture("register-light-390x844", 390, 844, state)
        capture("register-dark-320x568", 320, 568, state.themed(ShellTheme.Dark))
        capture("register-light-text2-320x800", 320, 800, state, textScale = 2f)
    }

    @Test
    fun account_is_captured_as_guest_and_signed_in() {
        capture("account-guest-light-390x844", 390, 844, account)
        capture("account-guest-dark-390x844", 390, 844, account.themed(ShellTheme.Dark))
        capture("account-guest-light-text2-320x800", 320, 800, account, textScale = 2f)
        capture("account-guest-light-text2-settings-320x800", 320, 800, account, textScale = 2f, scrollToTag = "settings")
        capture("account-guest-light-768x1024", 768, 1024, account)
        capture("account-signed-light-390x844", 390, 844, account) { it.completeLogin() }
        capture("account-signed-dark-390x844", 390, 844, account.themed(ShellTheme.Dark)) { it.completeLogin() }
        capture("account-signed-light-text2-320x800", 320, 800, account, textScale = 2f) { it.completeLogin() }
        capture("account-signed-light-text2-logout-320x800", 320, 800, account, textScale = 2f, scrollToTag = "logout") {
            it.completeLogin()
        }
    }

    /** The preview owners plus the Compose test that drives them, so a step can click and settle. */
    private class Drive(val test: ComposeUiTest, val h: PreviewOwners) {
        fun click(tag: String) {
            test.onNodeWithTag(tag).performScrollTo().performClick()
            test.waitForIdle()
        }

        fun completeLogin() {
            click("sign-in")
            click("login-primary")
            h.identity.returnFromBrowser(approve = true)
            test.waitForIdle()
        }
    }

    private fun capture(
        name: String,
        width: Int,
        height: Int,
        state: PreviewSeed,
        textScale: Float = 1f,
        scrollToTag: String? = null,
        steps: (Drive) -> Unit = {},
    ) {
        var rendererDensity = Float.NaN
        runDesktopComposeUiTest(width = width, height = height) {
            lateinit var owners: PreviewOwners
            setContent {
                rendererDensity = LocalDensity.current.density
                Box(Modifier.requiredSize(width.dp, height.dp).testTag("capture-root")) {
                    // The provider is held so each step decides when the browser returns.
                    PreviewTestRoot(state, textScale = textScale, onOwners = { it.identity.mode = PreviewLoginMode.Hold; owners = it })
                }
            }
            steps(Drive(this, owners))
            waitForIdle()
            if (scrollToTag != null) onNodeWithTag(scrollToTag).performScrollTo()
            waitForIdle()
            assertTrue(rendererDensity.isFinite() && rendererDensity > 0f)
            val bitmap = onNodeWithTag("capture-root").captureToImage()
            assertEquals(width, bitmap.width, "$name must retain the explicitly sized renderer width")
            assertEquals(height, bitmap.height, "$name must retain the explicitly sized renderer height")
            val output = File(System.getenv("CANTOS_THEATRE_AUTH_CAPTURE_DIR") ?: "build/reports/theatre-auth")
            check(output.isDirectory || output.mkdirs()) { "Could not create the auth capture output directory" }
            val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
            File(output, "$name.png").writeBytes(bytes)
            File(output, "$name.semantics.txt").writeText(onNodeWithTag("capture-root", useUnmergedTree = true).printToString())
            File(output, "$name.json").writeText(
                """
                {
                  "renderer": "JVM Desktop Compose UI test",
                  "entrypoint": "PreviewTheatreApp → CantosTheatreApp over deterministic identity ports",
                  "requestedWidthDp": $width,
                  "requestedHeightDp": $height,
                  "rendererDensity": $rendererDensity,
                  "requestedAppTextScale": $textScale,
                  "theme": "${state.state.theme}",
                  "locale": "${state.state.locale}",
                  "scrolledToTag": ${scrollToTag?.let { "\"$it\"" } ?: "null"},
                  "evidence": "screenshot-captured; inspection pending"
                }
                """.trimIndent(),
            )
        }
    }
}
