package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performTextInputSelection
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import androidx.compose.ui.text.TextRange
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.identity.EmailVerificationController
import com.cantos.theatre.application.identity.LinkReceipt
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.application.identity.StubIdentityV1Api
import com.cantos.theatre.application.identity.v1Grant
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.identity.CompletionStatus
import com.cantos.theatre.domain.identity.ProductionVerifyEmailHandler
import com.cantos.theatre.domain.identity.SignupCompletionRequest
import com.cantos.theatre.domain.identity.VerificationPhase
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.ports.SignupCompletionApi
import com.cantos.theatre.ports.StreamingMediaEngine
import java.io.File
import java.security.MessageDigest
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.emptyFlow
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/**
 * The actual application root, shell, account owner and completion controller over explicit fake
 * catalog, credential and completion ports. JVM semantics and pixels establish no Android App Link,
 * mail client, device or real Identity acceptance.
 */
@OptIn(ExperimentalTestApi::class)
class VerifyEmailFlowTest {
    private val op = "0123456789abcdef0123456789abcdef"
    private val proof = "fedcba9876543210fedcba9876543210"
    private val link = "$ProductionVerifyEmailHandler?mode=verifyEmail&oobCode=synthetic-action&apiKey=k" +
        "&continueUrl=https%3A%2F%2Fcantos.cc%2Fverify-email%3Foperation%3D$op%26proof%3D$proof&lang=vi"
    private val password = "Ánh đèn cuối sân khấu"

    private class Completion : SignupCompletionApi {
        val requests = mutableListOf<SignupCompletionRequest>()
        var gate: CompletableDeferred<IdentityV1Result<CompletionStatus>>? = null
        override suspend fun completeSignup(request: SignupCompletionRequest): IdentityV1Result<CompletionStatus> {
            requests += request
            return gate?.await() ?: IdentityV1Result.Accepted(CompletionStatus.Completed)
        }
    }

    private class Journal(var record: NativeCredentialRecord?) : NativeCredentialJournal {
        var writes = 0
        override fun read(): NativeJournalRead = record?.let(NativeJournalRead::Found) ?: NativeJournalRead.Missing
        override fun write(record: NativeCredentialRecord): CredentialFailure? { writes += 1; this.record = record; return null }
        override fun clear(): CredentialFailure? { writes += 1; record = null; return null }
    }

    private class Fixture(scope: CoroutineScope, signedIn: Boolean) {
        val work: PublicWork = PublicCatalogSamples.work(1, title = "Ánh đèn cuối sân khấu")
        val grant = v1Grant('a', 'c', "actor-a", "00000000-0000-4000-8000-000000000001", 121, 601, 901, authenticatedAt = 1, now = 1)
        val journal = Journal(if (signedIn) NativeCredentialRecord.Active(grant) else null)
        val account = NativeAccountController(scope, StubIdentityV1Api(), journal, object : LogoutBarrier {
            override fun read() = LogoutBarrierRead.Clear
            override fun mark(): CredentialFailure? = null
            override fun clear(): CredentialFailure? = null
        }, now = { 1 })
        val completion = Completion()
        val verification = EmailVerificationController(scope, completion, ProductionVerifyEmailHandler) { 0 }
        val runtime = NativeSessionRuntime(scope, nativeFactory = { Silent }, access = object : PlaybackAccessPort {
            override suspend fun manifest(publicationId: String): PlaybackAccessResult<AudioIdentity> = error("no playback here")
            override suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant> = error("no playback here")
        }, account = account, library = null, newUuid = { "00000000-0000-4000-8000-000000000001" }, nowMs = { 1_000 })
        val catalog = PublicCatalogController(scope, object : PublicCatalogPort {
            override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery) =
                PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(work)))
            override suspend fun work(id: PublicWorkId) = PublicCatalogRead.Ready(work)
            override suspend fun units(work: PublicWork, cursor: CatalogCursor?) =
                PublicCatalogRead.Ready(PublicCatalogSamples.unitsPage(work, emptyList()))
        }).also { it.dispatch(PublicCatalogIntent.Start) }
        val preferences = PreferencesController()
        val shell = TheatreShell(scope, catalog, runtime, preferences, verification = verification)
        fun close() { shell.close(); runtime.close(); catalog.close() }
    }

    private object Silent : StreamingMediaEngine {
        override val events = emptyFlow<PlaybackObservation>()
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = Unit
        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
        override suspend fun start(command: PlaybackEngineCommand) = Unit
        override suspend fun pause(command: PlaybackEngineCommand) = Unit
        override suspend fun seek(command: PlaybackEngineCommand) = Unit
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() = Unit
    }

    private fun scenario(signedIn: Boolean, theme: PreferenceTheme = PreferenceTheme.Light, width: Int = 390, height: Int = 844,
                         scale: Float = 1f, body: ComposeUiTest.(Fixture) -> Unit) = runDesktopComposeUiTest(width = width, height = height) {
        lateinit var fixture: Fixture
        setContent {
            val scope = rememberCoroutineScope()
            val owner = remember { Fixture(scope, signedIn).also { fixture = it } }
            DisposableEffect(owner) { onDispose { owner.close() } }
            remember(owner) { owner.preferences.setTheme(theme) }
            Box(Modifier.requiredSize(width.dp, height.dp).testTag("capture-root")) { CantosTheatreApp(owner.shell, textScale = scale) }
        }
        waitForIdle()
        body(fixture)
    }

    private fun ComposeUiTest.fill(fixture: Fixture) {
        runOnIdle { assertEquals(LinkReceipt.Shown, fixture.shell.receiveLink(link)) }
        waitForIdle()
        onNodeWithTag("verify-create").performScrollTo().assertIsNotEnabled()
        onNodeWithTag("verify-password").performTextInput(password)
        onNodeWithTag("verify-repeat").performTextInput(password)
        onNodeWithTag("verify-create").performScrollTo().assertIsNotEnabled()
        onNodeWithTag("verify-confirm").performScrollTo().performClick()
        onNodeWithTag("verify-create").performScrollTo().assertIsEnabled()
    }

    @Test
    fun independent_eyes_preserve_focus_selection_and_confirmation_while_next_moves_to_repeat() = scenario(signedIn = false) { f ->
        runOnIdle { assertEquals(LinkReceipt.Shown, f.shell.receiveLink(link)) }
        val first = onNodeWithTag("verify-password")
        val repeat = onNodeWithTag("verify-repeat")
        first.performScrollTo().performTextInput(password)
        first.performImeAction()
        repeat.assertIsFocused().performTextInput(password)
        repeat.performTextInputSelection(TextRange(2, 5))
        val selection = repeat.fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange]
        onNodeWithTag("verify-repeat-eye").performClick()
        repeat.assertIsFocused()
        assertEquals(selection, repeat.fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange])
        assertEquals(password, repeat.fetchSemanticsNode().config[SemanticsProperties.EditableText].text)
        assertTrue(first.fetchSemanticsNode().config[SemanticsProperties.EditableText].text != password)
        onNodeWithTag("verify-password-eye").performClick()
        assertEquals(password, first.fetchSemanticsNode().config[SemanticsProperties.EditableText].text)
        onNodeWithTag("verify-repeat-eye").performClick()
        assertTrue(repeat.fetchSemanticsNode().config[SemanticsProperties.EditableText].text != password)
        val eye = onNodeWithTag("verify-repeat-eye")
        eye.performSemanticsAction(SemanticsActions.RequestFocus) { assertTrue(it()) }
        eye.assertIsFocused().performKeyInput { pressKey(Key.Enter) }
        assertEquals(password, repeat.fetchSemanticsNode().config[SemanticsProperties.EditableText].text)
        onNodeWithTag("verify-create").performScrollTo().assertIsNotEnabled()
        assertEquals(0, f.completion.requests.size)
        onNodeWithTag("verify-confirm").performScrollTo().performClick()
        onNodeWithTag("verify-create").assertIsEnabled()
        first.performScrollTo().performTextInput("x")
        onNodeWithTag("verify-create").performScrollTo().assertIsNotEnabled()
        assertFalse(f.verification.state.value.confirmed)
        assertEquals(0, f.completion.requests.size)
    }

    @Test
    fun a_signed_out_listener_completes_in_the_app_and_goes_on_to_login_without_a_session() = scenario(signedIn = false) { f ->
        runOnIdle { assertEquals(NativeAccountState.SignedOut, f.account.state.value) }
        fill(f)
        onNodeWithTag("verify-create").performClick()
        waitForIdle()
        assertEquals(1, f.completion.requests.size)
        assertEquals(password, f.completion.requests.single().password)
        onNodeWithTag("verify-status").assertTextContains("Tài khoản đã được tạo", substring = true)
        runOnIdle {
            assertEquals(VerificationPhase.Completed(CompletionStatus.Completed), f.verification.state.value.phase)
            // Completion is not a session: the account owner never moved and nothing was stored.
            assertEquals(NativeAccountState.SignedOut, f.account.state.value)
            assertEquals(0, f.journal.writes)
        }
        onNodeWithTag("verify-continue").performScrollTo().performClick()
        waitForIdle()
        runOnIdle {
            assertEquals(TheatreRoute.Login, f.shell.navigation.value.route)
            assertFalse(TheatreRoute.VerifyEmail in f.shell.navigation.value.stack)
            assertEquals(VerificationPhase.Idle, f.verification.state.value.phase, "leaving forgets the link")
        }
    }

    @Test
    fun a_signed_in_listener_keeps_the_current_session_and_is_offered_only_account() = scenario(signedIn = true) { f ->
        waitUntil { f.account.state.value is NativeAccountState.SignedIn }
        val before = f.account.state.value
        val writes = f.journal.writes
        fill(f)
        onNodeWithTag("verify-create").performClick()
        waitForIdle()
        onNodeWithTag("verify-status").assertTextContains("Bạn vẫn đang đăng nhập bằng tài khoản hiện tại", substring = true)
        onNodeWithTag("verify-continue").assertTextContains("Mở Tài khoản")
        runOnIdle {
            assertEquals(before, f.account.state.value)
            assertEquals(writes, f.journal.writes)
        }
        onNodeWithTag("verify-continue").performScrollTo().performClick()
        waitForIdle()
        runOnIdle {
            assertEquals(TheatreRoute.Account, f.shell.navigation.value.route)
            assertEquals(before, f.account.state.value)
        }
    }

    @Test
    fun foreign_links_are_ignored_and_back_forgets_a_captured_link() = scenario(signedIn = false) { f ->
        runOnIdle {
            assertEquals(LinkReceipt.NotClaimed, f.shell.receiveLink("https://other.example/verify-email?mode=verifyEmail"))
            assertEquals(listOf(TheatreRoute.Discovery), f.shell.navigation.value.stack)
            assertEquals(LinkReceipt.Shown, f.shell.receiveLink("$ProductionVerifyEmailHandler?mode=verifyEmail"))
        }
        waitForIdle()
        onNodeWithTag("verify-status").assertTextContains("Chưa đọc được liên kết xác minh", substring = true)
        assertTrue(onAllNodesWithTag("verify-password").fetchSemanticsNodes().isEmpty())
        fill(f)
        onNodeWithTag("back").performClick()
        waitForIdle()
        runOnIdle {
            assertEquals(listOf(TheatreRoute.Discovery), f.shell.navigation.value.stack)
            assertEquals(VerificationPhase.Idle, f.verification.state.value.phase)
            assertEquals("", f.verification.state.value.password)
            assertEquals(0, f.completion.requests.size)
        }
    }

    @Test
    fun the_completion_screen_is_captured_for_inspection_across_theme_and_text_scale() {
        for (theme in PreferenceTheme.entries) for ((width, height, scale) in listOf(Triple(390, 844, 1f), Triple(320, 568, 2f))) {
            val name = "verify-email-${theme.name.lowercase()}-${width}x$height-text$scale"
            scenario(signedIn = false, theme = theme, width = width, height = height, scale = scale) { f ->
                fill(f)
                capture("$name-ready", width, height, scale, theme)
                runOnIdle { f.completion.gate = CompletableDeferred() }
                onNodeWithTag("verify-create").performClick()
                waitForIdle()
                onNodeWithTag("verify-create").assertIsNotEnabled()
                runOnIdle { f.completion.gate?.complete(IdentityV1Result.Accepted(CompletionStatus.Completed)) }
                waitForIdle()
                onNodeWithTag("verify-continue").assertIsEnabled()
                assertTrue(onNodeWithTag("verify-status").fetchSemanticsNode().config.contains(SemanticsProperties.LiveRegion))
                capture("$name-completed", width, height, scale, theme)
            }
        }
    }

    private fun ComposeUiTest.capture(name: String, width: Int, height: Int, scale: Float, theme: PreferenceTheme) {
        val node = onNodeWithTag("capture-root")
        val bytes = requireNotNull(Image.makeFromBitmap(node.captureToImage().asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
        val output = File(System.getenv("CANTOS_THEATRE_NATIVE_CAPTURE_DIR") ?: "build/reports/verify-email").apply { mkdirs() }
        File(output, "$name.png").writeBytes(bytes)
        File(output, "$name.semantics.txt").writeText(node.printToString())
        val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
        File(output, "$name.json").writeText("""{"renderer":"JVM Desktop Compose UI test","entrypoint":"CantosTheatreApp","route":"VerifyEmail","widthDp":$width,"heightDp":$height,"textScale":$scale,"theme":"${theme.name.lowercase()}","locale":"vi-VN","identity":"explicit fake completion and credential ports","png_sha256":"$hash"}""")
    }
}
