package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import androidx.compose.ui.text.intl.LocaleList
import androidx.compose.ui.text.AnnotatedString
import com.cantos.theatre.application.catalog.PublicCatalogEvent
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.catalog.PublicCatalogState
import com.cantos.theatre.application.catalog.UnitsResolved
import com.cantos.theatre.application.catalog.WorkResolved
import com.cantos.theatre.application.catalog.WorksResolved
import com.cantos.theatre.application.catalog.reducePublicCatalog
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.cursor
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unit
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unitsPage
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.work
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.worksPage
import com.cantos.theatre.domain.catalog.PublicUnitKind
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.features.catalog.presentation.PublicCatalogScreen
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import java.io.File
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test

/** The production catalog surface on the JVM Compose renderer; no Android launch is claimed. */
@OptIn(ExperimentalTestApi::class)
class PublicCatalogPresentationTest {
    private val story = work(1, PublicWorkKind.Story, title = "Ánh đèn cuối sân khấu")
    private val podcast = work(2, PublicWorkKind.Podcast, title = "Người dẫn chuyện kể chuyện đêm khuya")

    private fun fold(vararg events: PublicCatalogEvent): PublicCatalogState =
        events.fold(PublicCatalogState.Initial) { state, event -> reducePublicCatalog(state, event).state }

    private val listMore = arrayOf<PublicCatalogEvent>(
        PublicCatalogIntent.Start,
        WorksResolved(1, PublicCatalogRead.Ready(worksPage(listOf(story, podcast), cursor(1)))),
    )

    private val detailReady = arrayOf(
        *listMore,
        PublicCatalogIntent.OpenWork(story),
        WorkResolved(2, PublicCatalogRead.Ready(story)),
        UnitsResolved(3, PublicCatalogRead.Ready(unitsPage(story, listOf(
            unit(1, title = "Chương 1: Một tiếng chuông trong sương", durationMs = 45_000),
            unit(2, title = "Chương 2: Người giữ tiếng chuông", durationMs = 1_830_000),
        ), cursor(2)))),
    )

    @Test
    fun work_list_renders_server_rows_and_emits_typed_intents() {
        val intents = mutableListOf<PublicCatalogIntent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { PublicCatalogScreen(fold(*listMore), { intents += it }, UiPreferences()) }
            onNodeWithTag("catalog-work-${story.id.value}").assertIsDisplayed()
            onNodeWithText("Ánh đèn cuối sân khấu").assertExists()
            onNodeWithText("Truyện").assertExists()
            onNodeWithText("Podcast").assertExists()
            onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-load-more"))
            onNodeWithTag("catalog-load-more").performClick()
            onNodeWithTag("catalog-refresh").performClick()
            onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-work-${podcast.id.value}"))
            onNodeWithTag("catalog-work-${podcast.id.value}").performClick()
        }
        assertEquals(
            listOf(PublicCatalogIntent.LoadMoreWorks, PublicCatalogIntent.RefreshWorks, PublicCatalogIntent.OpenWork(podcast)),
            intents,
        )
    }

    @Test
    fun detail_shows_metadata_units_and_explains_listening_without_any_play_control() {
        val intents = mutableListOf<PublicCatalogIntent>()
        var backEnabled = false
        var back: () -> Unit = {}
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent {
                PublicCatalogScreen(fold(*detailReady), { intents += it }, UiPreferences(),
                    backHandler = { enabled, onBack -> backEnabled = enabled; back = onBack })
            }
            onNodeWithTag("catalog-detail-title").assertTextEquals("Ánh đèn cuối sân khấu")
                .assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Heading))
            onNodeWithText("Truyện").assertExists()
            onNodeWithText("Ngôn ngữ: Tiếng Việt").assertExists()
            onNodeWithTag("catalog-detail-description").assertTextEquals("Một lời hẹn chưa bao giờ nói ra.")
            onNodeWithTag("catalog-listening-note").assertExists()
            onNodeWithTag("catalog-detail").performScrollToNode(hasText("Các chương"))
            onNodeWithTag("catalog-detail").performScrollToNode(hasTestTag("catalog-unit-${unit(1).id.value}"))
            onNodeWithText("Chương 1: Một tiếng chuông trong sương").assertExists()
            onNodeWithText("45 giây").assertExists()
            onNodeWithTag("catalog-detail").performScrollToNode(hasTestTag("catalog-unit-${unit(2).id.value}"))
            onNodeWithText("31 phút").assertExists()
            // Selecting a row exposes metadata without issuing any playback command.
            val tree = onNodeWithTag("catalog-detail").printToString()
            for (forbidden in listOf("Phát", "Play", "Tạm dừng", "mini-player")) assertTrue(forbidden !in tree, forbidden)
            onNodeWithTag("catalog-unit-${unit(1).id.value}").assert(hasClickAction()).performClick()
            onNodeWithText("Đang xem chương hoặc tập này").assertExists()
            onNodeWithTag("catalog-detail").performScrollToNode(hasTestTag("catalog-load-more"))
            onNodeWithTag("catalog-load-more").performClick()
            onNodeWithTag("catalog-detail").performScrollToNode(hasTestTag("catalog-back"))
            onNodeWithTag("catalog-back").performClick()
        }
        assertTrue(backEnabled, "system Back is enabled on the detail route")
        back()
        assertEquals(listOf(PublicCatalogIntent.LoadMoreUnits, PublicCatalogIntent.Back, PublicCatalogIntent.Back), intents)
    }

    @Test
    fun search_submits_raw_text_to_the_owner_and_library_navigation_keeps_the_query() {
        val intents = mutableListOf<PublicCatalogIntent>()
        val locale = mutableStateOf(ShellLocale.Vietnamese)
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { PublicCatalogScreen(fold(*listMore), { intents += it }, UiPreferences(), locale = locale.value) }
            onNodeWithTag("catalog-search-input").performTextInput("Đêm & mưa {time}")
            onNodeWithTag("catalog-search-input").performImeAction()
            onNodeWithText("Của tôi").performClick()
            onNodeWithTag("catalog-library-unavailable").assertIsDisplayed()
            onNodeWithText("Khám phá").performClick()
            onNodeWithTag("catalog-search-input").assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("Đêm & mưa {time}")))
            runOnIdle { locale.value = ShellLocale.English }
            onNodeWithText("Work title or description").assertExists()
            onNodeWithTag("catalog-search-input").assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("Đêm & mưa {time}")))
            onNodeWithTag("catalog-search-clear").performClick()
        }
        assertEquals(listOf<PublicCatalogIntent>(PublicCatalogIntent.Search("Đêm & mưa {time}"), PublicCatalogIntent.Search("")), intents)
    }

    @Test
    fun server_search_empty_and_invalid_query_are_honest_and_do_not_offer_a_retry_loop() {
        val searched = fold(PublicCatalogIntent.Search("Không tồn tại"),
            WorksResolved(1, PublicCatalogRead.Ready(worksPage(emptyList()))))
        runDesktopComposeUiTest(width = 390, height = 844) {
            var state by mutableStateOf(searched)
            setContent { PublicCatalogScreen(state, {}, UiPreferences()) }
            onNodeWithTag("catalog-empty").assertExists()
            onNodeWithText("Không tìm thấy tác phẩm phù hợp").assertExists()
            state = fold(PublicCatalogIntent.Search("x".repeat(513)))
            waitForIdle()
            onNodeWithTag("catalog-failure").assertExists()
            onNodeWithTag("catalog-retry").assertDoesNotExist()
            onNodeWithTag("catalog-search-input").assertExists()
        }
    }

    @Test
    fun podcast_units_are_episodes_and_english_copy_is_used() {
        val state = fold(
            PublicCatalogIntent.OpenWork(podcast),
            WorkResolved(1, PublicCatalogRead.Ready(podcast)),
            UnitsResolved(2, PublicCatalogRead.Ready(unitsPage(podcast, listOf(unit(5, PublicUnitKind.Episode, title = "Tập mở đầu", durationMs = 60_000))))),
        )
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { PublicCatalogScreen(state, {}, UiPreferences(), locale = ShellLocale.English) }
            onNodeWithText("Podcast").assertExists()
            onNodeWithText("Language: Tiếng Việt").assertExists()
            onNodeWithText("Listening isn’t available in this version yet. You can still browse chapters and episodes.").assertExists()
            onNodeWithTag("catalog-detail").performScrollToNode(hasText("Episodes"))
            onNodeWithTag("catalog-detail").performScrollToNode(hasText("Tập mở đầu"))
            onNodeWithText("1 minute").assertExists()
            onNodeWithTag("catalog-load-more").assertDoesNotExist()
        }
    }

    @Test
    fun locale_switch_relabels_the_mounted_detail_without_changing_content_language_or_catalog_state() {
        val literalTitle = work(9, title = "Lời hẹn {time}")
        val state = fold(
            PublicCatalogIntent.OpenWork(literalTitle),
            WorkResolved(1, PublicCatalogRead.Ready(literalTitle)),
            UnitsResolved(2, PublicCatalogRead.Ready(unitsPage(literalTitle, emptyList()))),
        )
        val intents = mutableListOf<PublicCatalogIntent>()
        val locale = mutableStateOf(ShellLocale.Vietnamese)
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { PublicCatalogScreen(state, { intents += it }, UiPreferences(), locale = locale.value) }
            onNodeWithTag("catalog-back").assertContentDescriptionEquals("Quay lại")
            onNodeWithText("Ngôn ngữ: Tiếng Việt").assertExists()
            val before = onNodeWithTag("catalog-detail-title").fetchSemanticsNode().config[SemanticsProperties.Text].single()
            assertEquals("Lời hẹn {time}", before.text)
            assertEquals(LocaleList("vi-VN"), before.spanStyles.single().item.localeList)
            runOnIdle { locale.value = ShellLocale.English }
            onNodeWithTag("catalog-back").assertContentDescriptionEquals("Back")
            onNodeWithText("Language: Tiếng Việt").assertExists()
            val after = onNodeWithTag("catalog-detail-title").fetchSemanticsNode().config[SemanticsProperties.Text].single()
            assertEquals(before, after)
            runOnIdle { locale.value = ShellLocale.Vietnamese }
            onNodeWithTag("catalog-back").assertContentDescriptionEquals("Quay lại")
            assertEquals(literalTitle, state.detail?.summary)
        }
        assertTrue(intents.isEmpty(), "changing presentation locale never issues catalog or navigation commands")
    }

    @Test
    fun locale_switch_relabels_catalog_failure_and_recovery_on_the_same_mounted_screen() {
        val state = fold(PublicCatalogIntent.Start, WorksResolved(1, PublicCatalogRead.Transport))
        val locale = mutableStateOf(ShellLocale.Vietnamese)
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { PublicCatalogScreen(state, {}, UiPreferences(), locale = locale.value) }
            onNodeWithText("Chưa kết nối được danh sách").assertExists()
            onNodeWithTag("catalog-retry").assertTextEquals("Thử lại")
            runOnIdle { locale.value = ShellLocale.English }
            onNodeWithText("The list couldn’t be reached").assertExists()
            onNodeWithText("Check your connection and try again.").assertExists()
            onNodeWithTag("catalog-retry").assertTextEquals("Retry")
        }
    }

    @Test
    fun first_load_states_are_loading_empty_failure_and_not_found() {
        val intents = mutableListOf<PublicCatalogIntent>()
        runDesktopComposeUiTest(width = 390, height = 844) {
            var state by mutableStateOf(fold(PublicCatalogIntent.Start))
            setContent { PublicCatalogScreen(state, { intents += it }, UiPreferences()) }
            onNodeWithTag("catalog-loading").assertExists()
            onNodeWithTag("catalog-refresh").assertDoesNotExist()

            state = fold(PublicCatalogIntent.Start, WorksResolved(1, PublicCatalogRead.Ready(worksPage(emptyList(), null))))
            waitForIdle()
            onNodeWithTag("catalog-empty").assertExists()
            onNodeWithText("Chưa có tác phẩm nào để nghe").assertExists()

            state = fold(PublicCatalogIntent.Start, WorksResolved(1, PublicCatalogRead.Transport))
            waitForIdle()
            onNodeWithText("Chưa kết nối được danh sách").assertExists()
            onNodeWithTag("catalog-retry").performClick()

            state = fold(PublicCatalogIntent.OpenWork(story), WorkResolved(1, PublicCatalogRead.NotFound))
            waitForIdle()
            onNodeWithText("Tác phẩm này hiện không mở được").assertExists()
            onNodeWithTag("catalog-retry").assertDoesNotExist()
            onNodeWithTag("catalog-listening-note").assertDoesNotExist()
        }
        assertEquals(listOf<PublicCatalogIntent>(PublicCatalogIntent.RetryWorks), intents)
    }

    @Test
    fun next_page_failure_keeps_rows_and_offers_retry() {
        val intents = mutableListOf<PublicCatalogIntent>()
        val state = fold(*listMore, PublicCatalogIntent.LoadMoreWorks, WorksResolved(2, PublicCatalogRead.Unavailable(retryable = true)))
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { PublicCatalogScreen(state, { intents += it }, UiPreferences()) }
            onNodeWithTag("catalog-work-${story.id.value}").assertExists()
            onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-retry"))
            onNodeWithText("Chưa tải thêm được. Các mục đã hiện vẫn được giữ.").assertExists()
            onNodeWithTag("catalog-retry").performClick()
        }
        assertEquals(listOf<PublicCatalogIntent>(PublicCatalogIntent.RetryWorks), intents)
    }

    @Test
    fun layout_matrix_keeps_copy_inside_the_frame_and_is_captured_on_the_jvm() {
        val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
        check(output.isDirectory || output.mkdirs())
        val scenarios = listOf("list" to fold(*listMore), "detail" to fold(*detailReady))
        for ((scenario, state) in scenarios) for (theme in listOf(Theme.Light, Theme.Dark)) {
            for ((width, height, textScale) in listOf(Triple(390, 844, 1f), Triple(320, 568, 2f))) {
                val name = "catalog-$scenario-${theme.name.lowercase()}-${width}x$height-text$textScale"
                runDesktopComposeUiTest(width = width, height = height) {
                    setContent {
                        Box(Modifier.requiredSize(width.dp, height.dp).testTag("capture-root")) {
                            PublicCatalogScreen(state, {}, UiPreferences(theme = theme, textScale = textScale))
                        }
                    }
                    val root = onNodeWithTag("capture-root").fetchSemanticsNode().boundsInRoot
                    val tag = if (scenario == "list") "catalog-search-input" else "catalog-detail-title"
                    val bounds = onNodeWithTag(tag).fetchSemanticsNode().boundsInRoot
                    assertTrue(bounds.left >= root.left && bounds.right <= root.right, "$name clips $tag: $bounds outside $root")
                    val bitmap = onNodeWithTag("capture-root").captureToImage()
                    val data = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG))
                    File(output, "$name.png").writeBytes(data.bytes)
                    File(output, "$name.semantics.txt").writeText(onNodeWithTag("capture-root").printToString())
                    if (scenario == "list") {
                        onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-search"))
                        onNodeWithTag("catalog-search").assertIsDisplayed()
                        onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-work-${story.id.value}"))
                        onNodeWithTag("catalog-work-${story.id.value}").assertIsDisplayed()
                    }
                }
            }
        }
    }
}
