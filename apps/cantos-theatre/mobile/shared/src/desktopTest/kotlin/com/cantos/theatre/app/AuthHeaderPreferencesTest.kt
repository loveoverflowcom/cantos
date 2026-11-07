package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.features.auth.presentation.AuthHeader
import com.cantos.theatre.features.auth.presentation.AuthReturn
import com.cantos.theatre.features.auth.presentation.LoginScreen
import com.cantos.theatre.features.auth.presentation.RegisterScreen
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellThemeMode
import com.cantos.theatre.presentation.model.sampleState
import com.cantos.theatre.presentation.model.sampleWorks
import java.io.File
import java.security.MessageDigest
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/**
 * #690 / #686: every account-task route renders the one guest Theme + Language pair inside the auth
 * header row (Back | Cantos | Theme | Language), with no auth-only theme toggle and no second
 * preferences row. JVM Desktop Compose evidence only; it is not Android device or TalkBack acceptance.
 */
@OptIn(ExperimentalTestApi::class)
class AuthHeaderPreferencesTest {
    private data class Frame(val width: Int, val height: Int, val scale: Float, val theme: ShellTheme, val locale: ShellLocale)

    private val frames = listOf(
        Frame(390, 844, 1f, ShellTheme.Light, ShellLocale.Vietnamese),
        Frame(390, 844, 1f, ShellTheme.Dark, ShellLocale.English),
        Frame(320, 568, 2f, ShellTheme.Dark, ShellLocale.Vietnamese),
        Frame(320, 568, 2f, ShellTheme.Light, ShellLocale.English),
    )

    private fun ComposeUiTest.bounds(tag: String): Rect =
        onNodeWithTag(tag).assertIsDisplayed().fetchSemanticsNode().boundsInRoot

    @Test fun account_task_routes_keep_one_header_preferences_pair_and_the_system_theme_choice() {
        for (route in listOf(ShellRoute.Login, ShellRoute.Register, ShellRoute.VerifyEmail)) for (frame in frames) {
            runDesktopComposeUiTest(width = frame.width, height = frame.height) {
                val events = mutableListOf<ShellEvent>()
                val state = sampleState(stack = listOf(ShellRoute.Landing, route), theme = frame.theme, locale = frame.locale)
                setContent {
                    Box(Modifier.requiredSize(frame.width.dp, frame.height.dp).testTag("auth-capture")) {
                        CantosPresentation(state, ShellCatalogView(sampleWorks), { events += it }, textScale = frame.scale,
                            authContent = {
                                when (route) {
                                    ShellRoute.Login -> LoginScreen(frame.locale, AccountView.SignedOut, nothingSaved = false, onIntent = {})
                                    ShellRoute.Register -> RegisterScreen(frame.locale, {})
                                    else -> Unit
                                }
                            },
                            authHeader = { preferences ->
                                AuthHeader(frame.locale, frame.theme, AuthReturn.Landing, {}, preferences = preferences)
                            })
                    }
                }
                onAllNodesWithTag("auth-theme").assertCountEquals(0)
                onAllNodesWithTag("guest-theme").assertCountEquals(1)
                onAllNodesWithTag("guest-language").assertCountEquals(1)
                val px = density.density
                val back = bounds("back")
                val logo = bounds("header-title")
                val theme = bounds("guest-theme")
                val language = bounds("guest-language")
                val where = "$route ${frame.width}dp ${frame.scale}x"
                // One row: Back | Cantos | Theme | Language, nothing overlapping and nothing clipped.
                for (control in listOf(logo, theme, language)) {
                    assertTrue(control.top < back.bottom && control.bottom > back.top, "$where: header row")
                }
                assertTrue(back.left >= 0f && back.right <= logo.left && logo.right <= theme.left &&
                    theme.right <= language.left && language.right <= frame.width * px + 0.5f, "$where: order and fit")
                assertTrue(logo.width > 0f, "$where: logo kept")
                for (control in listOf(back, theme, language)) {
                    assertTrue(control.width >= 48f * px - 1f && control.height >= 48f * px - 1f, "$where: 48dp target")
                }
                val png = onNodeWithTag("auth-capture").captureToImage()
                val bytes = requireNotNull(Image.makeFromBitmap(png.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
                val name = "${route::class.simpleName}-${frame.width}-${frame.theme}-${frame.locale}-text${frame.scale}"
                val output = File("build/reports/auth-header-preferences").apply { mkdirs() }
                File(output, "$name.png").writeBytes(bytes)
                val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
                File(output, "$name.json").writeText("""{"renderer":"JVM Desktop Compose UI test","entrypoint":"CantosPresentation+AuthHeader","route":"${route::class.simpleName}","widthDp":${frame.width},"heightDp":${frame.height},"textScale":${frame.scale},"theme":"${frame.theme}","locale":"${frame.locale}","png_sha256":"$hash"}""")

                // The shared preferences owner keeps System and returns focus to the control that opened it.
                onNodeWithTag("guest-theme").performClick()
                onNodeWithTag("theme-Light").assertIsDisplayed()
                onNodeWithTag("theme-Dark").assertIsDisplayed()
                onNodeWithTag("theme-System").performClick()
                onNodeWithTag("guest-theme").assertIsFocused()
                onNodeWithTag("guest-language").performClick()
                onNodeWithTag("locale-English").performClick()
                onNodeWithTag("guest-language").assertIsFocused()
                assertEquals(listOf(ShellEvent.SetThemeMode(ShellThemeMode.System), ShellEvent.SetLocale(ShellLocale.English)), events, where)
            }
        }
    }

    @Test fun a_signed_in_account_task_shows_no_guest_preferences_or_auth_theme_toggle() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            val state = sampleState(stack = listOf(ShellRoute.Discovery, ShellRoute.VerifyEmail))
            setContent {
                Box(Modifier.requiredSize(390.dp, 844.dp)) {
                    CantosPresentation(state, ShellCatalogView(sampleWorks), {}, accountSignedIn = true,
                        authHeader = { preferences ->
                            AuthHeader(state.locale, state.theme, AuthReturn.Home, {}, preferences = preferences)
                        })
                }
            }
            onNodeWithTag("back").assertIsDisplayed()
            // Signed-in preferences stay in Me; the auth header never invents a theme state of its own.
            onAllNodesWithTag("guest-theme").assertCountEquals(0)
            onAllNodesWithTag("guest-language").assertCountEquals(0)
            onAllNodesWithTag("auth-theme").assertCountEquals(0)
        }
    }
}
