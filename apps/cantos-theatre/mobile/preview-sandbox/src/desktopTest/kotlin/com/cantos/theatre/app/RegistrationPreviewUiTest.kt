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
import androidx.compose.ui.semantics.SemanticsProperties
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
import com.cantos.theatre.adapters.http.NativeIdentityV1Client
import com.cantos.theatre.adapters.preview.PreviewLoginMode
import com.cantos.theatre.adapters.preview.previewLibraryStore
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.application.identity.RegistrationController
import com.cantos.theatre.domain.identity.RegistrationPhase
import com.cantos.theatre.domain.identity.RegistrationFailure
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

/** Real production UI/controller/Identity v1 HTTP client and desktop transport against a loopback
 * protocol fixture (the explicit debug loopback origin). No mailbox, account store or SQL is
 * present: mocked integration only, never proof of real signup or mail delivery.
 */
@OptIn(ExperimentalTestApi::class)
class RegistrationPreviewUiTest {
    private var rendererDensity = Float.NaN
    private class ApiFixture : AutoCloseable {
        private val executor = Executors.newCachedThreadPool()
        private val server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        val paths = CopyOnWriteArrayList<String>()
        val bodies = CopyOnWriteArrayList<String>()
        /** 202 by default; a scenario can answer the documented 429 or 503 instead. */
        @Volatile var status = 202
        init {
            server.executor = executor
            server.createContext("/api/v1/theatre/native/identity") { exchange ->
                paths += exchange.requestURI.path
                bodies += exchange.requestBody.use { it.readBytes().toString(Charsets.UTF_8) }
                // The real native transport must not manufacture browser or session authority.
                check(listOf("Origin", "Cookie", "Authorization", "Sec-Fetch-Site").none { exchange.requestHeaders.containsKey(it) })
                val response = if (status == 202) """{"accepted":true,"proof_ttl_seconds":900}""" else """{"error":"unavailable"}"""
                exchange.responseHeaders.set("Content-Type", "application/json")
                exchange.responseHeaders.set("Cache-Control", "no-store")
                if (status == 429) exchange.responseHeaders.set("Retry-After", "30")
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

    @Test fun production_register_sends_the_email_once_over_real_http_and_waits_for_web_without_fabricating_sign_in() {
        ApiFixture().use { api ->
            runDesktopComposeUiTest(width = 390, height = 844) {
                lateinit var owners: PreviewOwners
                lateinit var registration: RegistrationController
                mount(api, 390, 844, ShellTheme.Light, ShellLocale.Vietnamese, 1f) { owner, controller -> owners = owner; registration = controller }
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                onNodeWithTag("register-email").performScrollTo().performTextInput("Linh@example.test")
                click("register-submit")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Waiting }
                onNodeWithTag("register-status").assertTextContains("trình duyệt", substring = true)
                // Web owns proof and password: this form offers no link import, confirmation or setup.
                for (tag in listOf("register-link", "register-import", "register-confirm", "register-setup", "register-submit")) {
                    onNodeWithTag(tag).assertDoesNotExist()
                }
                assertEquals(NativeAccountState.SignedOut, owners.account.state.value)
                assertEquals(0, owners.identity.loginRequests)
                capture("waiting-light-390x844", 390, 844, ShellTheme.Light, ShellLocale.Vietnamese, 1f)
                click("register-login")
                onNodeWithTag("login-primary").assertExists()
                assertEquals(NativeAccountState.SignedOut, owners.account.state.value)
                assertEquals(listOf("/api/v1/theatre/native/identity/signup"), api.paths.toList())
                assertEquals(listOf("""{"email":"Linh@example.test"}"""), api.bodies.toList())
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
                assertEquals(0, api.paths.size) // Invalid input dispatches nothing.
                click("back")
                onNodeWithTag("login-primary").assertExists()
                click("login-create")
                onNodeWithTag("register-email").performScrollTo().assertTextContains("invalid")
                assertEquals(NativeAccountState.SignedOut, owner.account.state.value)
            }
        }
    }

    @Test fun a_rate_limited_signup_says_so_dispatches_once_and_only_an_explicit_retry_edits_again() {
        ApiFixture().use { api ->
            api.status = 429
            runDesktopComposeUiTest(width = 390, height = 844) {
                lateinit var registration: RegistrationController
                lateinit var owner: PreviewOwners
                mount(api, 390, 844, ShellTheme.Dark, ShellLocale.English, 1f) { h, c -> owner = h; registration = c }
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                onNodeWithTag("register-email").performScrollTo().performTextInput("a@example.test")
                click("register-submit")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Failed(RegistrationFailure.RateLimited) }
                onNodeWithTag("register-status").assertTextContains("Too many requests", substring = true)
                onNodeWithTag("register-submit").assertDoesNotExist()
                assertEquals(1, api.paths.size, "a refusal is never replayed automatically")
                api.status = 202
                click("register-retry")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                onNodeWithTag("register-email").performScrollTo().assertTextContains("a@example.test")
                assertEquals(1, api.paths.size, "retry returns to editing; it does not resend")
                assertEquals(NativeAccountState.SignedOut, owner.account.state.value)
            }
        }
    }

    @Test fun real_connection_refusal_shows_an_unknown_outcome_with_no_mock_fallback_and_no_automatic_resend() {
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
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                onNodeWithTag("register-submit").assertIsEnabled()
                assertEquals(NativeAccountState.SignedOut, owner.account.state.value)
                capture("api-down-light-en-390x844", 390, 844, ShellTheme.Light, ShellLocale.English, 1f)
            }
        }
    }

    @Test fun a_503_leaves_the_mail_effect_unknown_and_explicit_cancel_after_waiting_returns_to_login_as_a_guest() {
        ApiFixture().use { api ->
            api.status = 503
            runDesktopComposeUiTest(width = 390, height = 844) {
                lateinit var registration: RegistrationController
                lateinit var owner: PreviewOwners
                mount(api, 390, 844, ShellTheme.Light, ShellLocale.Vietnamese, 1f) { h, c -> owner = h; registration = c }
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                onNodeWithTag("register-email").performScrollTo().performTextInput("a@example.test")
                click("register-submit")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Failed(RegistrationFailure.StartUnknownOutcome) }
                api.status = 202
                click("register-retry")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                click("register-submit")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Waiting }
                click("register-cancel")
                assertEquals(RegistrationPhase.Cancelled, registration.state.value.phase)
                assertEquals(2, api.paths.size, "cancel sends nothing and recalls no mail")
                click("register-login")
                assertEquals(NativeAccountState.SignedOut, owner.account.state.value)
                assertEquals(0, owner.identity.loginRequests)
            }
        }
    }

    @Test fun editing_offers_no_cancel_and_login_and_register_fields_share_one_anatomy() {
        ApiFixture().use { api ->
            runDesktopComposeUiTest(width = 390, height = 844) {
                lateinit var registration: RegistrationController
                mount(api, 390, 844, ShellTheme.Light, ShellLocale.Vietnamese, 1f) { _, c -> registration = c }
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                // Nothing is pending while editing: Back and the Login link already leave, so no Cancel.
                onNodeWithTag("register-cancel").assertDoesNotExist()
                val register = onNodeWithTag("register-email").performScrollTo().fetchSemanticsNode().boundsInRoot
                click("back")
                val email = onNodeWithTag("login-email").performScrollTo().fetchSemanticsNode()
                val password = onNodeWithTag("login-password").performScrollTo().fetchSemanticsNode()
                assertEquals(register.height, email.boundsInRoot.height, "one field height across Login and Register")
                assertEquals(register.width, email.boundsInRoot.width)
                assertEquals(register.height, password.boundsInRoot.height)
                assertTrue(password.config.contains(SemanticsProperties.Password), "the shared field keeps password semantics")
                assertTrue(email.config.contains(SemanticsProperties.ContentDescription), "both fields name themselves the same way")
                assertTrue(password.config.contains(SemanticsProperties.ContentDescription))
                assertEquals(0, api.paths.size)
            }
        }
    }

    @Test fun submitting_keeps_one_cancel_and_waiting_keeps_it_until_the_listener_stops() {
        ApiFixture().use { api ->
            runDesktopComposeUiTest(width = 390, height = 844) {
                lateinit var registration: RegistrationController
                mount(api, 390, 844, ShellTheme.Light, ShellLocale.English, 1f) { _, c -> registration = c }
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Editing }
                onNodeWithTag("register-email").performScrollTo().performTextInput("a@example.test")
                click("register-submit")
                waitUntil(timeoutMillis = 10_000) { registration.state.value.phase == RegistrationPhase.Waiting }
                onNodeWithTag("register-cancel").performScrollTo().assertIsEnabled()
                assertEquals(1, api.paths.size, "a single dispatch")
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
                                   owners: (PreviewOwners, RegistrationController) -> Unit) {
        setContent {
            rendererDensity = LocalDensity.current.density
            val scope = rememberCoroutineScope()
            val registration = remember {
                RegistrationController(scope, NativeIdentityV1Client(api.origin, PreviewNativeHttp(scope, api.origin)) { System.currentTimeMillis() / 1000 })
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
        File(output, "$name.json").writeText("""{"renderer":"JVM Desktop Compose UI test","widthDp":$width,"heightDp":$height,"density":$rendererDensity,"textScale":$scale,"theme":"$theme","locale":"$locale","motion":"Reduced","screenReader":"not exercised","fixture":"loopback Identity v1 protocol fixture; no mailbox, no account store, no SQL","png_sha256":"$hash"}""" + "\n")
    }
}
