package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkPage
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.features.shell.presentation.CoverImageSource
import com.cantos.theatre.features.shell.presentation.CoverLoad
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.ports.StreamingMediaEngine
import com.cantos.theatre.presentation.model.ShellCover
import kotlin.test.assertEquals
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import org.junit.Test

/**
 * Discovery cards and Work Detail over real catalog and shell owners with a scripted cover
 * pipeline. Semantics prove the image, its alt text and the fallback; they do not establish
 * Android/iOS rendering, a real network read or TalkBack/VoiceOver behavior.
 */
@OptIn(ExperimentalTestApi::class)
class PublicCoverUiTest {
    private val sample = PublicCatalogSamples
    private val shown = sample.work(1, title = "Ánh đèn cuối sân khấu", cover = sample.cover(1))
    private val plain = sample.work(2, title = "Chuyện đêm muộn", cover = null)
    private val broken = sample.work(3, title = "Mùa nước nổi", cover = sample.cover(3, alt = "Con thuyền nhỏ giữa đồng ngập nước."))

    /** Scripted pipeline: [reply] decides each cover; [gate] holds every read until released. */
    private class Covers(val reply: (ShellCover) -> CoverLoad) : CoverImageSource {
        val loads = mutableListOf<String>()
        var gate: CompletableDeferred<Unit>? = null
        override suspend fun load(cover: ShellCover): CoverLoad {
            loads += cover.sha256
            gate?.await()
            return reply(cover)
        }
    }

    private class Quiet : StreamingMediaEngine {
        private val observations = MutableSharedFlow<PlaybackObservation>(extraBufferCapacity = 8)
        override val events = observations.asSharedFlow()
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = Unit
        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
        override suspend fun start(command: PlaybackEngineCommand) = Unit
        override suspend fun pause(command: PlaybackEngineCommand) = Unit
        override suspend fun seek(command: PlaybackEngineCommand) = Unit
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() = Unit
    }

    private class Owners(scope: CoroutineScope, val works: List<PublicWork>) {
        val preferences = PreferencesController()
        val catalog = PublicCatalogController(scope, object : PublicCatalogPort {
            override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery): PublicCatalogRead<PublicWorkPage> =
                PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(works))
            override suspend fun work(id: com.cantos.theatre.domain.catalog.PublicWorkId) =
                PublicCatalogRead.Ready(works.first { it.id == id })
            override suspend fun units(work: PublicWork, cursor: CatalogCursor?) =
                PublicCatalogRead.Ready(PublicCatalogSamples.unitsPage(work, listOf<PublicUnit>(PublicCatalogSamples.unit(1))))
        }).also { it.dispatch(PublicCatalogIntent.Start) }
        val runtime = NativeSessionRuntime(scope, nativeFactory = { Quiet() },
            access = object : PlaybackAccessPort {
                override suspend fun manifest(publicationId: String): PlaybackAccessResult<AudioIdentity> = error("not used")
                override suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant> = error("not used")
            }, account = null, library = null, newUuid = { "00000000-0000-4000-8000-000000000001" }, nowMs = { 1_000 })
        val shell = TheatreShell(scope, catalog, runtime, preferences)
        fun close() { shell.close(); runtime.close(); catalog.close() }
    }

    private fun card(work: PublicWork) = hasAnyAncestor(hasTestTag("work-${work.id.value}"))

    private fun androidx.compose.ui.test.ComposeUiTest.mount(covers: CoverImageSource?, width: Int = 390, height: Int = 844, scale: Float = 1f) {
        setContent {
            val scope = rememberCoroutineScope()
            val owners = remember { Owners(scope, listOf(shown, plain, broken)) }
            DisposableEffect(owners) { onDispose { owners.close() } }
            Box(Modifier.requiredSize(width.dp, height.dp)) { CantosTheatreApp(owners.shell, textScale = scale, coverImages = covers) }
        }
        waitForIdle()
    }

    private val image = ImageBitmap(8, 12)
    private fun pipeline() = Covers { cover -> if (cover.sha256 == sample.cover(3).sha256) CoverLoad.Unavailable else CoverLoad.Ready(image) }

    @Test
    fun a_card_shows_its_real_cover_with_alt_text_and_null_or_failed_covers_keep_the_fallback() = runDesktopComposeUiTest(width = 390, height = 844) {
        val covers = pipeline()
        mount(covers)
        waitUntil { onAllNodesWithTag("cover-image", useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty() }
        // Only the Work whose cover loaded shows an image, named by the Server's alt text.
        onAllNodesWithTag("cover-image", useUnmergedTree = true).assertCountEquals(1)
        onAllNodesWithContentDescription(shown.cover!!.altText, useUnmergedTree = true).assertCountEquals(1)
        onAllNodesWithContentDescription(broken.cover!!.altText, useUnmergedTree = true).assertCountEquals(0)
        onNodeWithTag("cover-image", useUnmergedTree = true).assert(card(shown))
        // The null cover issues no read at all; both covers the Server authorized were requested once.
        assertEquals(setOf(shown.cover!!.sha256, broken.cover!!.sha256), covers.loads.toSet())
        // Titles and navigation never depend on the image.
        listOf(shown, plain, broken).forEach { onNodeWithText(it.title).assertIsDisplayed() }
    }

    @Test
    fun the_detail_shows_the_cover_and_back_returns_to_the_same_card() = runDesktopComposeUiTest(width = 390, height = 844) {
        mount(pipeline())
        waitUntil { onAllNodesWithTag("cover-image", useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty() }
        onNodeWithTag("work-${shown.id.value}").performScrollTo().performClick()
        waitUntil { onAllNodesWithText("Chương 1: Một tiếng chuông trong sương").fetchSemanticsNodes().isNotEmpty() }
        onAllNodesWithTag("cover-image", useUnmergedTree = true).assertCountEquals(1)
        onAllNodesWithContentDescription(shown.cover!!.altText, useUnmergedTree = true).assertCountEquals(1)
        onNodeWithTag("back").performClick()
        waitForIdle()
        onNodeWithTag("work-${shown.id.value}").assertIsDisplayed()
        onAllNodesWithContentDescription(shown.cover!!.altText, useUnmergedTree = true).assertCountEquals(1)
        // A Work without a cover opens its detail on the fallback, with its title.
        onNodeWithTag("work-${plain.id.value}").performScrollTo().performClick()
        waitUntil { onAllNodesWithText("Chương 1: Một tiếng chuông trong sương").fetchSemanticsNodes().isNotEmpty() }
        onAllNodesWithTag("cover-image", useUnmergedTree = true).assertCountEquals(0)
        onAllNodesWithText(plain.title).assertCountEquals(1)
    }

    @Test
    fun a_slow_or_throwing_pipeline_never_blocks_the_title_or_navigation() = runDesktopComposeUiTest(width = 390, height = 844) {
        val slow = pipeline().also { it.gate = CompletableDeferred() }
        mount(slow)
        // While every image is loading the cards are complete and operable.
        onAllNodesWithTag("cover-image", useUnmergedTree = true).assertCountEquals(0)
        onNodeWithText(shown.title).assertIsDisplayed()
        onNodeWithTag("work-${plain.id.value}").performScrollTo().performClick()
        waitUntil { onAllNodesWithText("Chương 1: Một tiếng chuông trong sương").fetchSemanticsNodes().isNotEmpty() }
        slow.gate!!.complete(Unit)
        waitForIdle()
        onNodeWithTag("back").performClick()
        waitUntil { onAllNodesWithTag("cover-image", useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty() }
        onAllNodesWithTag("cover-image", useUnmergedTree = true).assertCountEquals(1)
    }

    @Test
    fun an_exception_from_the_pipeline_is_the_fallback_not_a_crash() = runDesktopComposeUiTest(width = 390, height = 844) {
        mount(Covers { error("decoder exploded") })
        onAllNodesWithTag("cover-image", useUnmergedTree = true).assertCountEquals(0)
        listOf(shown, plain, broken).forEach { onNodeWithText(it.title).assertIsDisplayed() }
        onNodeWithTag("work-${shown.id.value}").performScrollTo().performClick()
        waitUntil { onAllNodesWithText("Chương 1: Một tiếng chuông trong sương").fetchSemanticsNodes().isNotEmpty() }
    }

    @Test
    fun a_host_without_an_image_pipeline_shows_only_the_fallback() = runDesktopComposeUiTest(width = 390, height = 844) {
        mount(null)
        onAllNodesWithTag("cover-image", useUnmergedTree = true).assertCountEquals(0)
        listOf(shown, plain, broken).forEach { onNodeWithText(it.title).assertIsDisplayed() }
    }

    @Test
    fun a_small_phone_at_double_text_keeps_the_title_and_the_cover() = runDesktopComposeUiTest(width = 320, height = 568) {
        mount(pipeline(), width = 320, height = 568, scale = 2f)
        waitUntil { onAllNodesWithTag("cover-image", useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty() }
        onNodeWithText(shown.title).performScrollTo().assertIsDisplayed()
        onAllNodesWithContentDescription(shown.cover!!.altText, useUnmergedTree = true).assertCountEquals(1)
    }
}
