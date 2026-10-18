package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.app.NativeCatalogScreen
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.ports.StreamingMediaEngine
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import java.io.File
import java.security.MessageDigest
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/** Actual production screen/controller composition with explicit fake network and decoder ports.
 * JVM semantics/pixels establish no OS playback, audio, TLS, account or device acceptance. */
@OptIn(ExperimentalTestApi::class)
class NativeCatalogPresentationTest {
    private class Fixture(scope: CoroutineScope) {
        val work = PublicCatalogSamples.work(1, title = "Ánh đèn cuối sân khấu")
        val unit = PublicCatalogSamples.unit(1, title = "Chương 1: Người dẫn chuyện trở về", durationMs = 125_000)
        val audio = requireNotNull(AudioIdentity.of(work.id.value, unit.id.value, unit.publicationId.value,
            "60000000-0000-4000-8000-000000000006", "b".repeat(64), unit.durationMs))
        val engine = Decoder()
        var factories = 0
        var authorizations = 0
        val catalog = PublicCatalogController(scope, object : PublicCatalogPort {
            override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery) =
                PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(work)))
            override suspend fun work(id: PublicWorkId) = PublicCatalogRead.Ready(work)
            override suspend fun units(work: PublicWork, cursor: CatalogCursor?) =
                PublicCatalogRead.Ready(PublicCatalogSamples.unitsPage(work, listOf(unit)))
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
        fun close() { runtime.close(); catalog.close() }
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

    @Test
    fun production_screen_waits_for_decoder_ack_and_shares_mini_full_owner_across_navigation() {
        for (theme in listOf(Theme.Light, Theme.Dark)) for ((width, height, scale) in listOf(
            Triple(390, 844, 1f), Triple(320, 568, 2f))) {
            val name = "native-${theme.name.lowercase()}-${width}x$height-text$scale"
            runDesktopComposeUiTest(width = width, height = height) {
                lateinit var fixture: Fixture
                setContent {
                    val scope = rememberCoroutineScope()
                    val owner = remember { Fixture(scope).also { fixture = it } }
                    DisposableEffect(owner) { onDispose { owner.close() } }
                    Box(Modifier.requiredSize(width.dp, height.dp).testTag("native-capture-root")) {
                        NativeCatalogScreen(owner.catalog, owner.runtime, UiPreferences(theme = theme, textScale = scale), ShellLocale.Vietnamese)
                    }
                }
                waitForIdle()
                onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-work-${fixture.work.id.value}"))
                onNodeWithTag("catalog-work-${fixture.work.id.value}").performClick()
                onNodeWithTag("catalog-detail").performScrollToNode(hasTestTag("catalog-play-${fixture.unit.id.value}"))
                onNodeWithTag("catalog-play-${fixture.unit.id.value}").performClick()
                waitUntil { fixture.engine.starts.isNotEmpty() }
                onNodeWithTag("native-playback-status").assertTextEquals("Đang tải âm thanh…")
                assertEquals(PlaybackStatus.Buffering, fixture.runtime.listening.playback.state.value.status)
                val first = fixture.engine.loads.single()
                runOnIdle { fixture.engine.emit(PlaybackEvent.EngineReady(first.identity)) }
                waitForIdle()
                assertEquals(PlaybackStatus.Buffering, fixture.runtime.listening.playback.state.value.status)
                runOnIdle { fixture.engine.emit(PlaybackEvent.EngineStarted(first.identity)) }
                waitForIdle()
                onNodeWithTag("native-playback-status").assertTextEquals("Đang phát")
                onNodeWithTag("native-mini-player").assertIsDisplayed()
                for (tag in listOf("native-toggle", "native-open-player")) {
                    val bounds = onNodeWithTag(tag).fetchSemanticsNode().boundsInRoot
                    assertTrue(bounds.width >= 48f && bounds.height >= 48f, "$name undersized $tag: $bounds")
                }
                capture(name + "-mini", "native-capture-root", width, height, scale, theme)
                onNodeWithTag("native-open-player").performClick()
                for (tag in listOf("native-seek", "native-full-rewind", "native-full-toggle", "native-full-forward",
                    "native-speed-750", "native-speed-1000", "native-speed-1250", "native-speed-1500",
                    "native-speed-2000", "native-full-back")) {
                    val bounds = onNodeWithTag(tag).performScrollTo().fetchSemanticsNode().boundsInRoot
                    assertTrue(bounds.width >= 48f && bounds.height >= 48f, "$name undersized $tag: $bounds")
                }
                onNodeWithTag("native-full-toggle").performScrollTo().performClick()
                waitForIdle()
                assertEquals(PlaybackStatus.Paused, fixture.runtime.listening.playback.state.value.status)
                onNodeWithTag("native-speed-1500").performScrollTo().performClick()
                waitForIdle()
                assertEquals(1500, fixture.runtime.listening.playback.state.value.session?.speedPermille)
                capture(name + "-full", "native-full-player", width, height, scale, theme)
                onNodeWithTag("native-full-back").performScrollTo().performClick()
                onNodeWithTag("native-playback-status").assertTextEquals("Đang tạm dừng")
                onNodeWithTag("native-toggle").performClick()
                waitUntil { fixture.engine.starts.size == 2 }
                assertEquals(PlaybackStatus.Buffering, fixture.runtime.listening.playback.state.value.status)
                runOnIdle { fixture.engine.emit(PlaybackEvent.EngineStarted(fixture.engine.loads.last().identity)) }
                waitForIdle()
                onNodeWithTag("native-playback-status").assertTextEquals("Đang phát")
                onNodeWithTag("catalog-detail").performScrollToNode(hasTestTag("catalog-back"))
                onNodeWithTag("catalog-back").performClick()
                onNodeWithTag("catalog-nav-library").performClick()
                onNodeWithText("Chưa lưu được thay đổi. Dữ liệu trước đó được giữ lại.").assertExists()
                onNodeWithTag("native-library").assertDoesNotExist()
                onNodeWithText("Đăng nhập để lưu và đồng bộ tủ sách, lịch sử và điểm nghe.").assertDoesNotExist()
                onNodeWithTag("catalog-nav-discover").performClick()
                onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-account"))
                onNodeWithTag("catalog-account").performClick()
                onNodeWithTag("native-account").assertExists()
                onNodeWithTag("native-playback-status").assertTextEquals("Đang phát")
                assertEquals(1, fixture.factories)
                assertEquals(2, fixture.authorizations)
                assertEquals(fixture.audio, fixture.runtime.listening.playback.state.value.session?.source?.audio)
            }
        }
    }

    private fun ComposeUiTest.capture(name: String, tag: String, width: Int, height: Int, scale: Float, theme: Theme) {
        val node = onNodeWithTag(tag)
        val image = node.captureToImage()
        val bytes = requireNotNull(Image.makeFromBitmap(image.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
        val output = File(System.getenv("CANTOS_THEATRE_NATIVE_CAPTURE_DIR") ?: "build/reports/native-catalog")
        check(output.isDirectory || output.mkdirs())
        File(output, "$name.png").writeBytes(bytes)
        File(output, "$name.semantics.txt").writeText(node.printToString())
        val hash = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
        File(output, "$name.json").writeText("""{"renderer":"JVM Desktop Compose UI test","entrypoint":"NativeCatalogScreen","widthDp":$width,"heightDp":$height,"textScale":$scale,"theme":"$theme","locale":"vi-VN","media":"explicit fake decoder/access ports; no audio","png_sha256":"$hash"}""")
        assertTrue(image.width > 0 && image.height > 0)
    }
}
