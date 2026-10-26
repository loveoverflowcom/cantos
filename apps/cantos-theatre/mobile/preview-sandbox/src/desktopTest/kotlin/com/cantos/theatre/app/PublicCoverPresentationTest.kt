package com.cantos.theatre.app

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.theatre.features.shell.presentation.CantosPresentation
import com.cantos.theatre.features.shell.presentation.CoverImageSource
import com.cantos.theatre.features.shell.presentation.CoverLoad
import com.cantos.theatre.features.shell.presentation.LocalCoverImages
import com.cantos.theatre.presentation.model.ShellCatalogView
import com.cantos.theatre.presentation.model.ShellCover
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.presentation.model.ShellWork
import com.cantos.theatre.presentation.model.ShellWorkId
import java.io.File
import kotlin.test.assertEquals
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/**
 * Cover rendering through the production compositor from controlled facts: a real image with the
 * Server's alt text, a null cover, a cover whose image fails, and a Work detail, at a phone and
 * a wide viewport with realistic Vietnamese. No catalog adapter, network or device is involved.
 */
@OptIn(ExperimentalTestApi::class)
class PublicCoverPresentationTest {
    private val none = ShellEpisodeId("none", ShellWorkId("none"))

    private fun cover(tag: String, alt: String) = ShellCover(
        url = "https://api.cantos.example/api/v1/theatre/works/$tag/cover/${tag.padEnd(64, 'a')}",
        sha256 = tag.padEnd(64, 'a'), byteLength = 1, mime = "image/png", widthPx = 300, heightPx = 450, altText = alt,
    )

    private val shown = ShellWork(
        ShellWorkId("a"), "Ánh đèn cuối sân khấu", "Truyện", "Một lời hẹn chưa bao giờ nói ra, ở lại sau tấm màn nhung.", emptyList(),
        cover = cover("a", "Sân khấu tối, một ngọn đèn vàng chiếu xuống chiếc ghế trống."),
    )
    private val plain = ShellWork(ShellWorkId("b"), "Chuyện đêm muộn", "Podcast", "Những câu chuyện kể khi thành phố đã ngủ.", emptyList())
    private val broken = ShellWork(
        ShellWorkId("c"), "Mùa nước nổi", "Tiểu thuyết", "Một ngôi làng ven sông và những người ở lại.", emptyList(),
        cover = cover("c", "Con thuyền nhỏ giữa cánh đồng ngập nước lúc hoàng hôn."),
    )

    /** A deterministic portrait "cover": a dark field with a warm disc, drawn once. */
    private val artwork: ImageBitmap = ImageBitmap(300, 450).also { bitmap ->
        val canvas = Canvas(bitmap)
        val paint = Paint()
        paint.color = Color(0xFF1B2A41)
        canvas.drawRect(0f, 0f, 300f, 450f, paint)
        paint.color = Color(0xFFE0A93B)
        canvas.drawCircle(Offset(150f, 170f), 80f, paint)
    }

    private val source = CoverImageSource { cover -> if (cover.sha256.startsWith("c")) CoverLoad.Unavailable else CoverLoad.Ready(artwork) }

    private fun ComposeUiTest.mount(state: ShellState, events: MutableList<ShellEvent> = mutableListOf()) {
        setContent {
            CompositionLocalProvider(LocalCoverImages provides source) {
                CantosPresentation(state, ShellCatalogView(works = listOf(shown, plain, broken)), events::add)
            }
        }
        waitForIdle()
    }

    private fun ComposeUiTest.capture(name: String) {
        val bitmap = onNodeWithTag("work-a").captureToImage()
        val bytes = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
        val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-cover")
        output.mkdirs()
        File(output, "cover-$name.png").writeBytes(bytes)
    }

    @Test
    fun phone_cards_show_the_real_cover_and_keep_the_fallback_for_null_and_failed_images() = runDesktopComposeUiTest(width = 390, height = 844) {
        val events = mutableListOf<ShellEvent>()
        mount(ShellState(selectedEpisode = none), events)
        onAllNodesWithTag("cover-image", useUnmergedTree = true).assertCountEquals(1)
        onAllNodesWithContentDescription(shown.cover!!.altText, useUnmergedTree = true).assertCountEquals(1)
        capture("phone-discovery")
        listOf(shown, plain, broken).forEach { onNodeWithText(it.title).performScrollTo().assertIsDisplayed() }
        // Opening any card routes by the Work, whether or not its image exists.
        onNodeWithTag("work-c").performScrollTo().performClick()
        assertEquals(listOf<ShellEvent>(ShellEvent.Navigate(ShellRoute.Work(ShellWorkId("c")))), events.toList())
    }

    @Test
    fun a_wide_grid_and_the_detail_show_the_cover_whole() = runDesktopComposeUiTest(width = 1100, height = 900) {
        mount(ShellState(selectedEpisode = none))
        onAllNodesWithTag("cover-image", useUnmergedTree = true).assertCountEquals(1)
        capture("wide-discovery")
    }

    @Test
    fun the_detail_screen_shows_the_cover_or_the_fallback_beside_the_title() = runDesktopComposeUiTest(width = 390, height = 844) {
        mount(ShellState(stack = listOf(ShellRoute.Discovery, ShellRoute.Work(ShellWorkId("a"))), selectedEpisode = none))
        onAllNodesWithTag("cover-image", useUnmergedTree = true).assertCountEquals(1)
        onAllNodesWithContentDescription(shown.cover!!.altText, useUnmergedTree = true).assertCountEquals(1)
        onNodeWithText(shown.title).assertIsDisplayed()
    }

    @Test
    fun the_failed_cover_detail_is_the_fallback_with_its_title() = runDesktopComposeUiTest(width = 390, height = 844) {
        mount(ShellState(stack = listOf(ShellRoute.Discovery, ShellRoute.Work(ShellWorkId("c"))), selectedEpisode = none))
        onAllNodesWithTag("cover-image", useUnmergedTree = true).assertCountEquals(0)
        onNodeWithText(broken.title).assertIsDisplayed()
    }
}
