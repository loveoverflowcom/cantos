package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.preferences.PreferenceLocale
import com.cantos.theatre.application.preferences.PreferenceTheme
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.ports.StreamingMediaEngine
import java.io.File
import java.security.MessageDigest
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/**
 * The actual application root over its real controllers, with explicit fake network and decoder
 * ports. JVM semantics and pixels establish no OS playback, audio, TLS, account or device acceptance.
 */
@OptIn(ExperimentalTestApi::class)
class CantosTheatreAppTest {
    private class Fixture(
        scope: CoroutineScope,
        val works: List<PublicWork>,
        val units: Map<PublicWork, List<PublicUnit>>,
        val worksReply: (CatalogCursor?) -> PublicCatalogRead<com.cantos.theatre.domain.catalog.PublicWorkPage> = {
            PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(works))
        },
        val workReply: (PublicWorkId) -> PublicCatalogRead<PublicWork>? = { null },
    ) {
        val work = works.first()
        val unit = units.getValue(work).first()
        val audio = requireNotNull(AudioIdentity.of(work.id.value, unit.id.value, unit.publicationId!!.value,
            "60000000-0000-4000-8000-000000000006", "b".repeat(64), unit.durationMs!!))
        val engine = Decoder()
        var factories = 0
        var authorizations = 0
        var worksReads = 0
        val preferences = PreferencesController()
        val catalog = PublicCatalogController(scope, object : PublicCatalogPort {
            override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery): PublicCatalogRead<com.cantos.theatre.domain.catalog.PublicWorkPage> {
                worksReads += 1
                return worksReply(cursor)
            }
            override suspend fun work(id: PublicWorkId) = workReply(id) ?: PublicCatalogRead.Ready(works.first { it.id == id })
            override suspend fun units(work: PublicWork, cursor: CatalogCursor?) =
                PublicCatalogRead.Ready(PublicCatalogSamples.unitsPage(work, units.getValue(work)))
        }).also { it.dispatch(PublicCatalogIntent.Start) }
        val runtime = NativeSessionRuntime(scope, nativeFactory = { factories += 1; engine },
            access = object : PlaybackAccessPort {
                override suspend fun manifest(publicationId: String) = PlaybackAccessResult.Accepted(audio)
                override suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant> {
                    assertEquals(audio, identity)
                    authorizations += 1
                    return PlaybackAccessResult.Accepted(PlaybackGrant(PlaybackLocation("https://media.invalid/test-only"), 1_000_000))
                }
            }, account = null, library = null,
            newUuid = { "00000000-0000-4000-8000-000000000001" }, nowMs = { 1_000 })
        val shell = TheatreShell(scope, catalog, runtime, preferences)
        fun close() { shell.close(); runtime.close(); catalog.close() }
    }

    private class Decoder : StreamingMediaEngine {
        private val observations = MutableSharedFlow<PlaybackObservation>(extraBufferCapacity = 64)
        override val events = observations.asSharedFlow()
        val loads = mutableListOf<PlaybackEngineCommand>()
        val starts = mutableListOf<PlaybackEngineCommand>()
        var closed = false
        fun emit(observation: PlaybackObservation) { check(observations.tryEmit(observation)) }
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) { loads += command }
        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
        override suspend fun start(command: PlaybackEngineCommand) { starts += command }
        override suspend fun pause(command: PlaybackEngineCommand) { emit(PlaybackEvent.EnginePaused(command.identity)) }
        override suspend fun seek(command: PlaybackEngineCommand) { emit(PlaybackEvent.EnginePosition(command.identity, command.positionMs)) }
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() { closed = true }
    }

    private fun single(): Pair<List<PublicWork>, Map<PublicWork, List<PublicUnit>>> {
        val work = PublicCatalogSamples.work(1, title = "Ánh đèn cuối sân khấu")
        val unit = PublicCatalogSamples.unit(1, title = "Chương 1: Người dẫn chuyện trở về", durationMs = 125_000)
        return listOf(work) to mapOf(work to listOf(unit))
    }

    @Test
    fun me_keeps_system_selected_while_the_host_appearance_changes() =
        runDesktopComposeUiTest(width = 390, height = 844) {
            lateinit var fixture: Fixture
            var hostDark by mutableStateOf(false)
            var renderedDark = false
            setContent {
                val scope = rememberCoroutineScope()
                val owner = remember { val (works, units) = single(); Fixture(scope, works, units).also { fixture = it } }
                DisposableEffect(owner) { onDispose { owner.close() } }
                CantosTheatreApp(owner.shell, systemIsDark = hostDark, onThemeChanged = { renderedDark = it })
            }
            onNodeWithTag("account").performClick()
            onNodeWithTag("setting-Theme").performScrollTo().performClick()
            onNodeWithTag("theme-System").performClick()
            waitForIdle()
            assertEquals(PreferenceTheme.System, fixture.preferences.state.value.theme)
            assertEquals(false, renderedDark)
            runOnIdle { hostDark = true }
            waitForIdle()
            assertEquals(true, renderedDark)
            assertEquals(PreferenceTheme.System, fixture.preferences.state.value.theme)
            onNodeWithText("C", substring = false).assertDoesNotExist()

            runOnIdle { fixture.shell.dispatch(ShellEvent.Navigate(ShellRoute.Account)) }
            onNodeWithTag("setting-Theme").performScrollTo().performClick()
            onNodeWithTag("theme-System").assertIsSelected()
            onNodeWithTag("theme-Dark").performClick()
            waitForIdle()
            assertEquals(PreferenceTheme.Dark, fixture.preferences.state.value.theme)
            runOnIdle { hostDark = false }
            waitForIdle()
            assertEquals(true, renderedDark, "an explicit choice ignores later host changes")
            onNodeWithTag("setting-Language").performScrollTo().performClick()
            onNodeWithTag("locale-English").performClick()
            waitForIdle()
            assertEquals(PreferenceLocale.English, fixture.preferences.state.value.locale)
            assertEquals(PreferenceTheme.Dark, fixture.preferences.state.value.theme)
        }

    @Test
    fun the_known_experience_plays_real_catalog_audio_through_one_player_owner_across_navigation() {
        for (theme in listOf(PreferenceTheme.Light, PreferenceTheme.Dark)) for ((width, height, scale) in listOf(
            Triple(390, 844, 1f), Triple(320, 568, 2f))) {
            val name = "theatre-${theme.name.lowercase()}-${width}x$height-text$scale"
            runDesktopComposeUiTest(width = width, height = height) {
                lateinit var fixture: Fixture
                setContent {
                    val scope = rememberCoroutineScope()
                    val owner = remember { val (works, units) = single(); Fixture(scope, works, units).also { fixture = it } }
                    DisposableEffect(owner) { onDispose { owner.close() } }
                    remember(owner) { owner.preferences.setTheme(theme) }
                    Box(Modifier.requiredSize(width.dp, height.dp).testTag("capture-root")) {
                        CantosTheatreApp(owner.shell, textScale = scale)
                    }
                }
                waitForIdle()
                // The familiar Discover screen over the API's work, with no sample notice or tools.
                onNodeWithTag("work-${fixture.work.id.value}").assertExists()
                onNodeWithText("Ánh đèn cuối sân khấu").assertExists()
                onNodeWithTag("demo-notice").assertDoesNotExist()
                onNodeWithTag("reset").assertDoesNotExist()
                capture(name + "-discover", "capture-root", width, height, scale, theme)
                onNodeWithTag("work-${fixture.work.id.value}").performScrollTo().performClick()
                waitUntil { onAllNodesWithTagExists("episode-${fixture.unit.id.value}") }
                capture(name + "-work", "capture-root", width, height, scale, theme)
                onNodeWithTag("episode-${fixture.unit.id.value}").performScrollTo().performClick()
                waitUntil { fixture.engine.starts.isNotEmpty() }
                onNodeWithTag("simulation-status").assertTextEquals("Đang tải âm thanh…")
                assertEquals(PlaybackStatus.Buffering, fixture.runtime.listening.playback.state.value.status)
                val first = fixture.engine.loads.single()
                runOnIdle { fixture.engine.emit(PlaybackEvent.EngineReady(first.identity)) }
                waitForIdle()
                assertEquals(PlaybackStatus.Buffering, fixture.runtime.listening.playback.state.value.status)
                runOnIdle { fixture.engine.emit(PlaybackEvent.EngineStarted(first.identity)) }
                waitForIdle()
                onNodeWithTag("simulation-status").assertTextEquals("Đang phát")
                for (tag in listOf("transport", "rewind", "forward")) {
                    val bounds = onNodeWithTag(tag).performScrollTo().fetchSemanticsNode().boundsInRoot
                    assertTrue(bounds.width >= 48f && bounds.height >= 48f, "$name undersized $tag: $bounds")
                }
                // Real bookmarks have no API, so the control is absent rather than a local-only fiction.
                onNodeWithTag("bookmark").assertDoesNotExist()
                capture(name + "-player", "capture-root", width, height, scale, theme)
                onNodeWithTag("transport").performScrollTo().performClick()
                waitForIdle()
                assertEquals(PlaybackStatus.Paused, fixture.runtime.listening.playback.state.value.status)
                onNodeWithTag("player-speed").performScrollTo().performClick()
                onNodeWithTag("speed-150").performClick()
                waitForIdle()
                assertEquals(1500, fixture.runtime.listening.playback.state.value.session?.speedPermille)
                // Closing the room keeps the one owner and the mini-player on Discover.
                onNodeWithTag("back").performClick()
                onNodeWithTag("mini-player").assertIsDisplayed()
                onNodeWithTag("mini-transport").performClick()
                waitUntil { fixture.engine.starts.size == 2 }
                assertEquals(PlaybackStatus.Buffering, fixture.runtime.listening.playback.state.value.status)
                runOnIdle { fixture.engine.emit(PlaybackEvent.EngineStarted(fixture.engine.loads.last().identity)) }
                waitForIdle()
                capture(name + "-mini", "capture-root", width, height, scale, theme)
                onNodeWithTag("nav-library").performClick()
                onNodeWithTag("library-continue").assertExists()
                onNodeWithTag("mini-player").assertIsDisplayed()
                onNodeWithTag("account").performClick()
                onNodeWithTag("account-profile").assertExists()
                assertEquals(1, fixture.factories)
                assertEquals(2, fixture.authorizations)
                assertEquals(fixture.audio, fixture.runtime.listening.playback.state.value.session?.source?.audio)
            }
        }
    }

    @Test
    fun first_load_failure_offers_retry_and_a_partial_list_pages_without_a_false_total() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            val first = PublicCatalogSamples.work(1, title = "Trang một")
            val second = PublicCatalogSamples.work(2, title = "Trang hai")
            val units = mapOf(first to listOf(PublicCatalogSamples.unit(1)), second to listOf(PublicCatalogSamples.unit(2)))
            val next = PublicCatalogSamples.cursor(1)
            var fail = true
            lateinit var fixture: Fixture
            setContent {
                val scope = rememberCoroutineScope()
                val owner = remember {
                    Fixture(scope, listOf(first, second), units, worksReply = { cursor ->
                        when {
                            fail -> PublicCatalogRead.Transport
                            cursor == null -> PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(first), next))
                            else -> PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(second)))
                        }
                    }).also { fixture = it }
                }
                DisposableEffect(owner) { onDispose { owner.close() } }
                CantosTheatreApp(owner.shell)
            }
            waitForIdle()
            onNodeWithTag("error-dialog").assertExists()
            onNodeWithText("Chưa kết nối được với máy chủ. Hãy kiểm tra mạng và thử lại.").assertExists()
            fail = false
            onNodeWithTag("error-dialog-primary").performClick()
            waitUntil { onAllNodesWithTagExists("work-${first.id.value}") }
            // Page one of two: no count claims a total, and the footer offers the next page.
            onNodeWithText("1 tập").assertDoesNotExist()
            onNodeWithTag("list-load-more").performScrollTo().performClick()
            waitUntil { onAllNodesWithTagExists("work-${second.id.value}") }
            onNodeWithTag("list-load-more").assertDoesNotExist()
            assertTrue(fixture.worksReads >= 3)
        }
    }

    @Test
    fun an_unreadable_work_says_so_and_a_cached_catalog_offers_refresh() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            val (works, units) = single()
            lateinit var fixture: Fixture
            setContent {
                val scope = rememberCoroutineScope()
                val owner = remember {
                    Fixture(scope, works, units, workReply = { PublicCatalogRead.NotFound }).also { fixture = it }
                }
                DisposableEffect(owner) { onDispose { owner.close() } }
                CantosTheatreApp(owner.shell)
            }
            waitForIdle()
            onNodeWithTag("work-${fixture.work.id.value}").performScrollTo().performClick()
            waitUntil { onAllNodesWithTagExists("work-not-found") }
            onNodeWithText("Tác phẩm này hiện không mở được").assertExists()
            onNodeWithTag("episode-${fixture.unit.id.value}").assertDoesNotExist()
            onNodeWithTag("back").performClick()
            onNodeWithTag("work-${fixture.work.id.value}").assertExists()
            fixture.preferences.setLocale(PreferenceLocale.English)
            waitForIdle()
            onNodeWithText("A world to listen to").assertExists()
        }
    }

    @Test
    fun an_offline_cached_catalog_says_so_and_refresh_asks_the_api_again_while_a_guest_library_asks_to_sign_in() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            val (works, units) = single()
            val cache = object : com.cantos.theatre.ports.CatalogCachePort {
                override val status = kotlinx.coroutines.flow.MutableStateFlow(com.cantos.theatre.ports.CatalogCacheStatus.Cached)
                override fun account(): String? = null
                override fun read(key: String, owner: String?): String? = null
                override fun put(key: String, validatedJson: String, owner: String?) = Unit
                override fun network() { status.value = com.cantos.theatre.ports.CatalogCacheStatus.Network }
                override fun invalidate() = Unit
            }
            lateinit var fixture: Fixture
            setContent {
                val scope = rememberCoroutineScope()
                val owner = remember { Fixture(scope, works, units).also { fixture = it } }
                DisposableEffect(owner) { onDispose { owner.close() } }
                CantosTheatreApp(owner.shell, cache = cache)
            }
            waitForIdle()
            onNodeWithTag("catalog-cached").assertExists()
            onNodeWithText("Danh sách đã lưu trên máy. Chọn Làm mới để kiểm tra bản hiện tại.").assertExists()
            val reads = fixture.worksReads
            onNodeWithTag("catalog-refresh").performClick()
            waitUntil { fixture.worksReads > reads }
            // Guest Library: sign-in is the explanation; no sample row or fixture content appears.
            onNodeWithTag("nav-library").performClick()
            onNodeWithTag("library-sync").assertExists()
            onNodeWithText("Đăng nhập để lưu và đồng bộ tủ sách, lịch sử và điểm nghe.").assertExists()
            onNodeWithTag("library-continue").assertExists()
            onNodeWithTag("library-empty-hero").assertExists()
        }
    }

    private fun ComposeUiTest.onAllNodesWithTagExists(tag: String): Boolean =
        onAllNodes(androidx.compose.ui.test.hasTestTag(tag)).fetchSemanticsNodes().isNotEmpty()

    private fun ComposeUiTest.capture(name: String, tag: String, width: Int, height: Int, scale: Float, theme: PreferenceTheme) {
        val node = onNodeWithTag(tag)
        val image = node.captureToImage()
        val bytes = requireNotNull(Image.makeFromBitmap(image.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
        val output = File(System.getenv("CANTOS_THEATRE_NATIVE_CAPTURE_DIR") ?: "build/reports/theatre-app")
        check(output.isDirectory || output.mkdirs())
        File(output, "$name.png").writeBytes(bytes)
        File(output, "$name.semantics.txt").writeText(node.printToString())
        val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
        File(output, "$name.json").writeText("""{"renderer":"JVM Desktop Compose UI test","entrypoint":"CantosTheatreApp","widthDp":$width,"heightDp":$height,"textScale":$scale,"theme":"$theme","locale":"vi-VN","media":"explicit fake decoder/access ports; no audio","png_sha256":"$hash"}""")
        assertTrue(image.width > 0 && image.height > 0)
    }
}
