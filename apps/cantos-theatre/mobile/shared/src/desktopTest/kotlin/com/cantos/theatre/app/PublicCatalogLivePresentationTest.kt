package com.cantos.theatre.app

import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.JvmCatalogTransport
import com.cantos.theatre.adapters.http.PublicCatalogHttpAdapter
import com.cantos.theatre.application.catalog.DetailPhase
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.catalog.WalkPhase
import com.cantos.theatre.features.catalog.presentation.PublicCatalogScreen
import com.cantos.ui.UiPreferences
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Assume
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Opt-in real API → platform transport → controller → production Compose surface. JVM only. */
@OptIn(ExperimentalTestApi::class, ExperimentalCoroutinesApi::class)
class PublicCatalogLivePresentationTest {
    @Test
    fun clean_state_browses_searches_and_refreshes_the_live_api_without_demo_mapping() {
        val rawOrigin = System.getenv("CANTOS_THEATRE_LIVE_ORIGIN")
        Assume.assumeTrue("CANTOS_THEATRE_LIVE_ORIGIN is not set", !rawOrigin.isNullOrEmpty())
        val seeded = System.getenv("CANTOS_THEATRE_REQUIRE_SEEDED") == "true"
        val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
        check(output.isDirectory || output.mkdirs())
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default.limitedParallelism(1))
        val controller = PublicCatalogController(scope, PublicCatalogHttpAdapter(ApiOrigin.parse(requireNotNull(rawOrigin)), JvmCatalogTransport()))
        try {
            runDesktopComposeUiTest(width = 390, height = 844) {
                setContent {
                    val state by controller.state.collectAsState()
                    PublicCatalogScreen(state, controller::dispatch, UiPreferences())
                }
                controller.dispatch(PublicCatalogIntent.Start)
                waitUntil(timeoutMillis = 20_000) { controller.state.value.works.phase != WalkPhase.Loading && controller.state.value.works.phase != WalkPhase.NotStarted }
                assertTrue(controller.state.value.works.received, "live API must answer a valid page")
                assertTrue(!seeded || controller.state.value.works.items.isNotEmpty(), "explicit seeded acceptance requires server Works")
                fun capture(tag: String, name: String) {
                    val image = onNodeWithTag(tag).captureToImage()
                    val data = requireNotNull(Image.makeFromBitmap(image.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG))
                    File(output, name).writeBytes(data.bytes)
                }
                capture("catalog-works", "catalog-live-list-vi-390x844.png")
                val initial = controller.state.value.works.items
                if (initial.isNotEmpty()) {
                    // Optional operator handshake: mutate/release backend data, then signal the
                    // test. This exercises the mounted app's Refresh with no client rebuild.
                    System.getenv("CANTOS_THEATRE_LIVE_REFRESH_DIR")?.let { directory ->
                        val handoff = File(directory)
                        check(handoff.isDirectory || handoff.mkdirs())
                        check(!File(handoff, "catalog-changed").exists()) { "Use a fresh refresh-handshake directory" }
                        File(handoff, "catalog-ready").writeText(initial.first().id.value)
                        waitUntil(timeoutMillis = 60_000) { File(handoff, "catalog-changed").isFile }
                        val beforeRefresh = controller.state.value.issued
                        onNodeWithTag("catalog-refresh").performClick()
                        waitUntil(timeoutMillis = 20_000) {
                            controller.state.value.issued > beforeRefresh && controller.state.value.works.phase != WalkPhase.Loading
                        }
                        assertTrue(controller.state.value.works.received)
                        assertTrue(initial.map { it.id to it.title } != controller.state.value.works.items.map { it.id to it.title },
                            "refresh must observe the signalled eligible backend change")
                        capture("catalog-works", "catalog-live-refreshed-vi-390x844.png")
                        File(handoff, "catalog-refreshed").writeText("PASS")
                    }
                    val work = controller.state.value.works.items.first()
                    onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-work-${work.id.value}"))
                    onNodeWithTag("catalog-work-${work.id.value}").performClick()
                    waitUntil(timeoutMillis = 20_000) { controller.state.value.detail?.units?.received == true || controller.state.value.detail?.phase is DetailPhase.Failed }
                    assertTrue(controller.state.value.detail?.phase is DetailPhase.Ready, "live detail must be available")
                    assertEquals(work.id, controller.state.value.detail?.id)
                    assertTrue(controller.state.value.detail?.units?.received == true, "live units must answer a valid page")
                    capture("catalog-detail", "catalog-live-detail-vi-390x844.png")
                    var pageActions = 1
                    while (controller.state.value.detail?.units?.canLoadMore == true) {
                        onNodeWithTag("catalog-detail").performScrollToNode(hasTestTag("catalog-load-more"))
                        val beforePage = controller.state.value.issued
                        onNodeWithTag("catalog-load-more").performClick()
                        waitUntil(timeoutMillis = 20_000) {
                            controller.state.value.issued > beforePage && controller.state.value.detail?.units?.phase != WalkPhase.Loading
                        }
                        assertTrue(controller.state.value.detail?.units?.phase !is WalkPhase.Failed)
                        assertTrue(++pageActions <= 1_000, "live unit walk must terminate")
                    }
                    val units = requireNotNull(controller.state.value.detail?.units).items
                    assertEquals(units.size, units.map { it.id }.toSet().size)
                    println("Live production Compose work=${work.id.value} units=${units.size} page_actions=$pageActions")
                    onNodeWithTag("catalog-detail").performScrollToNode(hasTestTag("catalog-back"))
                    onNodeWithTag("catalog-back").performClick()
                    waitUntil(timeoutMillis = 10_000) { controller.state.value.detail == null }
                    onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-search-open"))
                    onNodeWithTag("catalog-search-open").performClick()
                    val searchText = work.title.take(64)
                    onNodeWithTag("catalog-search-input").performTextReplacement(searchText)
                    onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-kind-${work.kind.name.lowercase()}"))
                    onNodeWithTag("catalog-kind-${work.kind.name.lowercase()}").performClick()
                    onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-search-submit"))
                    onNodeWithTag("catalog-search-submit").performClick()
                    waitUntil(timeoutMillis = 20_000) { controller.state.value.query.text == searchText.trim() && controller.state.value.works.received }
                    assertTrue(controller.state.value.works.items.any { it.id == work.id }, "server query must find the selected released Work")
                    onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-search-input"))
                    onNodeWithTag("catalog-search-input").performTextReplacement("Cantos không có tác phẩm ${System.nanoTime()}")
                    onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-search-submit"))
                    onNodeWithTag("catalog-search-submit").performClick()
                    waitUntil(timeoutMillis = 20_000) { controller.state.value.query.text.startsWith("Cantos không có tác phẩm") && controller.state.value.works.received }
                    assertTrue(controller.state.value.works.isEmptyResult, "a real empty server query never falls back")
                    onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-empty"))
                    capture("catalog-works", "catalog-live-empty-vi-390x844.png")
                }
            }
        } finally {
            controller.close()
            scope.cancel()
        }
    }
}
