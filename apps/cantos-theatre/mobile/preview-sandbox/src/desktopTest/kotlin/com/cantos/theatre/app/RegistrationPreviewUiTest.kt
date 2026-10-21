package com.cantos.theatre.app

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
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import androidx.compose.ui.input.key.Key
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.NativeRegistrationHttp
import com.cantos.theatre.adapters.preview.PreviewLoginMode
import com.cantos.theatre.adapters.preview.previewLibraryStore
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.application.identity.RegistrationController
import com.cantos.theatre.domain.identity.RegistrationPhase
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.ports.RegistrationBrowser
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import com.sun.net.httpserver.HttpServer
import java.io.File
import java.net.InetSocketAddress
import java.security.MessageDigest
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.Executors
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Real production UI/controller/generated HTTP adapter and desktop transport against a
 * loopback protocol fixture. Firebase, Kanidm and SQL are not present: mocked integration only.
 */
@OptIn(ExperimentalTestApi::class)
class RegistrationPreviewUiTest {
    private var rendererDensity = Float.NaN
    private class ApiFixture : AutoCloseable {
        private val executor = Executors.newCachedThreadPool()
        private val server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        val paths = CopyOnWriteArrayList<String>()
        val bodies = CopyOnWriteArrayList<String>()
        var proofUnavailable = false
        init {
            server.executor = executor
            server.createContext("/api/v1/theatre/native/auth/registration") { exchange ->
                paths += exchange.requestURI.path
                val body = exchange.requestBody.use { it.readBytes().toString(Charsets.UTF_8) }
                bodies += body
                // The real native transport must not manufacture browser or session authority.
                check(listOf("Origin", "Cookie", "Authorization").none { exchange.requestHeaders.containsKey(it) })
                val path = exchange.requestURI.path
                val (status, response) = when {
                    path.endsWith("/configuration") -> 200 to """{"status":"firebase_email_verification","action_origin":"https://cantos.example"}"""
                    path.endsWith("/confirm") -> 200 to if (proofUnavailable) """{"status":"unavailable"}""" else
                        """{"status":"setup_ready","login":"listener_fixture","setup_url":"https://idm.example/ui/reset?token=fixture-only"}"""
                    else -> 202 to """{"status":"accepted"}"""
                }
                exchange.responseHeaders.set("Content-Type", "application/json")
                exchange.responseHeaders.set("Cache-Control", "no-store")
                val bytes = response.toByteArray(Charsets.UTF_8)
                exchange.sendResponseHeaders(status, bytes.size.toLong())
                exchange.responseBody.use { it.write(bytes) }
                exchange.close()
            }
            server.start()
        }
        val origin: ApiOrigin get() = ApiOrigin.parse("http://127.0.0.1:${server.address.port}")
        override fun close() { server.stop(0); executor.shutdownNow() }
    }
    private val action = "https://cantos.example/?mode=verifyEmail&oobCode=fixture-only&continueUrl=https%3A%2F%2Fcantos.example%2F%3FfirebaseEnrollment%3D00000000-0000-4000-8000-000000000001"

    @Test fun production_register_performs_real_http_and_one_shot_setup_without_fabricating_sign_in() {
        ApiFixture().use { api ->
            runDesktopComposeUiTest(width = 390, height = 844) {
                lateinit var owners: PreviewOwners
                lateinit var registration: RegistrationController
                var opens = 0
                mount(api, 390, 844, ShellTheme.Light, ShellLocale.Vietnamese, 1f, { opens += 1 }) { owner, controller -> owners = owner; registration = controller }
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                onNodeWithTag("register-email").performScrollTo().performTextInput("Linh@example.test")
                click("register-submit")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Waiting }
                onNodeWithTag("register-link").performScrollTo().performTextInput(action)
                click("register-import")
                assertEquals(0, api.paths.count { it.endsWith("/confirm") })
                click("register-confirm")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase is RegistrationPhase.SetupReady }
                assertEquals(NativeAccountState.SignedOut, owners.account.state.value)
                assertEquals(0, owners.identity.loginRequests)
                capture("setup-ready-light-390x844", 390, 844, ShellTheme.Light, ShellLocale.Vietnamese, 1f)
                click("register-setup")
                assertEquals(1, opens)
                onNodeWithTag("register-setup").assertDoesNotExist()
                click("register-login")
                onNodeWithTag("login-primary").assertExists()
                assertEquals(NativeAccountState.SignedOut, owners.account.state.value)
                assertEquals(1, api.paths.count { it.endsWith("/confirm") })
                assertTrue(api.bodies.any { it == """{"email":"Linh@example.test"}""" })
            }
        }
    }

    @Test fun registration_is_usable_in_both_themes_languages_narrow_large_and_large_text() {
        data class Scenario(val width: Int, val height: Int, val theme: ShellTheme, val locale: ShellLocale, val scale: Float)
        for (scenario in listOf(
            Scenario(390, 844, ShellTheme.Light, ShellLocale.Vietnamese, 1f),
            Scenario(390, 844, ShellTheme.Dark, ShellLocale.English, 1f),
            Scenario(320, 800, ShellTheme.Light, ShellLocale.Vietnamese, 2f),
            Scenario(320, 800, ShellTheme.Dark, ShellLocale.English, 2f),
            Scenario(768, 1024, ShellTheme.Light, ShellLocale.Vietnamese, 1f),
        )) ApiFixture().use { api ->
            runDesktopComposeUiTest(width = scenario.width, height = scenario.height) {
                lateinit var owner: PreviewOwners
                lateinit var registration: RegistrationController
                mount(api, scenario.width, scenario.height, scenario.theme, scenario.locale, scenario.scale) { h, c -> owner = h; registration = c }
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                capture("register-${scenario.theme}-${scenario.locale}-${scenario.width}-text${scenario.scale}", scenario.width, scenario.height, scenario.theme, scenario.locale, scenario.scale)
                onNodeWithTag("register-email").performScrollTo().performClick().assertIsFocused().performKeyInput { pressKey(Key.Tab) }
                onNodeWithTag("register-submit").assertIsFocused()
                onNodeWithTag("register-submit").performScrollTo()
                capture("register-actions-${scenario.theme}-${scenario.locale}-${scenario.width}-text${scenario.scale}", scenario.width, scenario.height, scenario.theme, scenario.locale, scenario.scale)
                onNodeWithTag("register-email").performScrollTo().performTextInput("invalid")
                click("register-submit")
                onNodeWithTag("register-status").assertExists()
                assertEquals(1, api.paths.size) // Metadata only: invalid input dispatches nothing.
                click("back")
                onNodeWithTag("login-primary").assertExists()
                click("login-create")
                onNodeWithTag("register-email").performScrollTo().assertTextContains("invalid")
                assertEquals(NativeAccountState.SignedOut, owner.account.state.value)
            }
        }
    }

    @Test fun cancelled_and_expired_or_replayed_proof_never_offer_setup_or_sign_in() {
        ApiFixture().use { api ->
            api.proofUnavailable = true
            runDesktopComposeUiTest(width = 390, height = 844) {
                lateinit var registration: RegistrationController
                lateinit var owner: PreviewOwners
                mount(api, 390, 844, ShellTheme.Dark, ShellLocale.Vietnamese, 1f) { h, c -> owner = h; registration = c }
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                onNodeWithTag("register-email").performScrollTo().performTextInput("a@example.test")
                click("register-submit")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Waiting }
                onNodeWithTag("register-link").performScrollTo().performTextInput(action)
                click("register-import"); click("register-confirm")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase is RegistrationPhase.Failed }
                onNodeWithTag("register-status").assertTextContains("Mã đã hết hạn", substring = true)
                onNodeWithTag("register-confirm").assertDoesNotExist()
                onNodeWithTag("register-setup").assertDoesNotExist()
                assertEquals(NativeAccountState.SignedOut, owner.account.state.value)
            }
        }
    }

    @Test fun real_connection_refusal_shows_unknown_submit_then_retryable_capability_failure_without_mock_fallback() {
        ApiFixture().use { api ->
            runDesktopComposeUiTest(width = 390, height = 844) {
                lateinit var registration: RegistrationController
                lateinit var owner: PreviewOwners
                mount(api, 390, 844, ShellTheme.Light, ShellLocale.English, 1f) { h, c -> owner = h; registration = c }
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                api.close()
                onNodeWithTag("register-email").performScrollTo().performTextInput("a@example.test")
                click("register-submit")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome) }
                onNodeWithTag("register-status").assertTextContains("submission outcome is unknown", substring = true)
                onNodeWithTag("register-submit").assertDoesNotExist()
                click("register-retry")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Failed(RegistrationFailure.Unavailable) }
                onNodeWithTag("register-status").assertTextContains("could not be reached", substring = true)
                onNodeWithTag("register-retry").assertIsEnabled()
                assertEquals(NativeAccountState.SignedOut, owner.account.state.value)
                capture("api-down-light-en-390x844", 390, 844, ShellTheme.Light, ShellLocale.English, 1f)
            }
        }
    }

    @Test fun explicit_cancel_posts_the_imported_action_once_and_returns_to_login_as_a_guest() {
        ApiFixture().use { api ->
            runDesktopComposeUiTest(width = 390, height = 844) {
                lateinit var registration: RegistrationController
                lateinit var owner: PreviewOwners
                mount(api, 390, 844, ShellTheme.Light, ShellLocale.Vietnamese, 1f) { h, c -> owner = h; registration = c }
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                onNodeWithTag("register-email").performScrollTo().performTextInput("a@example.test")
                click("register-submit")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Waiting }
                onNodeWithTag("register-link").performScrollTo().performTextInput(action)
                click("register-import"); click("register-cancel")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Cancelled }
                assertEquals(1, api.paths.count { it.endsWith("/cancel") })
                assertEquals(0, api.paths.count { it.endsWith("/confirm") })
                click("register-login")
                assertEquals(NativeAccountState.SignedOut, owner.account.state.value)
                assertEquals(0, owner.identity.loginRequests)
            }
        }
    }

    private fun ComposeUiTest.click(tag: String) {
        val node = onNodeWithTag(tag)
        if (tag != "back") node.performScrollTo()
        node.assertIsEnabled().performClick()
        waitForIdle()
    }

    private fun ComposeUiTest.mount(api: ApiFixture, width: Int, height: Int, theme: ShellTheme, locale: ShellLocale, scale: Float,
                                   onOpen: () -> Unit = {}, owners: (PreviewOwners, RegistrationController) -> Unit) {
        setContent {
            rendererDensity = LocalDensity.current.density
            val scope = rememberCoroutineScope()
            val registration = remember {
                RegistrationController(scope, NativeRegistrationHttp(api.origin, "https://idm.example", PreviewNativeHttp(scope, api.origin)), object : RegistrationBrowser {
                    override fun openSetup(url: String, completion: (Boolean) -> Unit) { onOpen(); completion(true) }
                })
            }
            val seed = remember { PreviewSeed(previewState(stack = listOf(ShellRoute.Discovery, ShellRoute.Login), theme = theme, locale = locale)) }
            val owner = remember { PreviewOwners(scope, seed, previewLibraryStore(), registration = registration).also { it.identity.mode = PreviewLoginMode.Hold; it.shell.openRegister(); owners(it, registration) } }
            DisposableEffect(owner) { onDispose { registration.close(); owner.close() } }
            Box(Modifier.requiredSize(width.dp, height.dp).testTag("registration-capture")) { CantosTheatreApp(owner.shell, textScale = scale) }
        }
    }

    private fun ComposeUiTest.capture(name: String, width: Int, height: Int, theme: ShellTheme, locale: ShellLocale, scale: Float) {
        val pixels = onNodeWithTag("registration-capture").captureToImage()
        val bytes = requireNotNull(Image.makeFromBitmap(pixels.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
        val output = File("build/reports/theatre-registration").also { it.mkdirs() }
        File(output, "$name.png").writeBytes(bytes)
        val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it.toInt() and 0xff) }
        assertTrue(rendererDensity.isFinite() && rendererDensity > 0f)
        File(output, "$name.json").writeText("""{"renderer":"JVM Desktop Compose UI test","widthDp":$width,"heightDp":$height,"density":$rendererDensity,"textScale":$scale,"theme":"$theme","locale":"$locale","motion":"Reduced","screenReader":"not exercised","fixture":"loopback protocol fixture; mocked Firebase/Kanidm, no SQL","png_sha256":"$hash"}""" + "\n")
    }
}
