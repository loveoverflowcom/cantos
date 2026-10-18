package com.cantos.theatre.app

import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.runDesktopComposeUiTest
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.CatalogHttpResponse
import com.cantos.theatre.adapters.http.CatalogHttpTransport
import com.cantos.theatre.adapters.http.PublicCatalogHttpAdapter
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.features.catalog.presentation.PublicCatalogScreen
import com.cantos.ui.UiPreferences
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertTrue

/** Real production composition with only the remote HTTP peer replaced; no demo mapping. */
@OptIn(ExperimentalTestApi::class, ExperimentalCoroutinesApi::class)
class PublicCatalogProductionFlowTest {
    @Test
    fun cold_catalog_search_retry_and_refresh_render_server_owned_uuid_and_changed_title() {
        val id = "c946f680-69db-42f2-815d-bd44f790aa56"
        // HTTP writes and Compose polling iterate concurrently; each iterator needs a snapshot.
        val urls = CopyOnWriteArrayList<String>()
        val firstSearch = AtomicBoolean(true)
        val refreshed = AtomicBoolean(false)
        fun page(title: String) = """{"items":[{"work_id":"$id","kind":"story","title":"$title","description":"Nội dung đến từ API.","language":"vi-VN","first_published_at_ms":1000}],"next_cursor":null}"""
        val transport = CatalogHttpTransport { request ->
            urls += request.url
            when {
                "/search?" in request.url && firstSearch.getAndSet(false) ->
                    CatalogHttpResponse(503, "application/json", """{"code":"unavailable","retryable":true}""".encodeToByteArray())
                "/search?" in request.url -> CatalogHttpResponse(200, "application/json", page("Mặt trăng ở sân ga").encodeToByteArray())
                else -> CatalogHttpResponse(200, "application/json", page(if (refreshed.get()) "Lời hẹn vừa cập nhật" else "Lời hẹn từ máy chủ").encodeToByteArray())
            }
        }
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default.limitedParallelism(1))
        val controller = PublicCatalogController(scope, PublicCatalogHttpAdapter(ApiOrigin.parse("https://api.cantos.example"), transport))
        try {
            runDesktopComposeUiTest(width = 390, height = 844) {
                setContent {
                    val state by controller.state.collectAsState()
                    PublicCatalogScreen(state, controller::dispatch, UiPreferences())
                }
                fun capture(name: String) {
                    val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
                    check(output.isDirectory || output.mkdirs())
                    val bitmap = onNodeWithTag("catalog-works").captureToImage()
                    val png = requireNotNull(Image.makeFromBitmap(bitmap.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG))
                    File(output, name).writeBytes(png.bytes)
                }
                controller.dispatch(PublicCatalogIntent.Start)
                waitUntil(timeoutMillis = 10_000) { controller.state.value.works.received }
                onNodeWithText("Lời hẹn từ máy chủ").assertExists()
                onNodeWithTag("catalog-work-$id").assertExists()
                onNodeWithTag("catalog-search-open").performClick()
                onNodeWithTag("catalog-search-input").performTextReplacement("mặt trăng")
                onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-kind-story"))
                onNodeWithTag("catalog-kind-story").performClick()
                onNodeWithTag("catalog-kind-story").assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
                onNodeWithTag("catalog-kind-all").assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, false))
                onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-search-submit"))
                onNodeWithTag("catalog-search-submit").performClick()
                waitUntil(timeoutMillis = 10_000) { urls.any { "/search?" in it } }
                onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-retry"))
                capture("catalog-production-search-error-vi-390x844.png")
                onNodeWithTag("catalog-retry").performClick()
                waitUntil(timeoutMillis = 10_000) { controller.state.value.works.items.firstOrNull()?.title == "Mặt trăng ở sân ga" }
                onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-work-$id"))
                onNodeWithText("Mặt trăng ở sân ga").assertExists()
                onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-search-clear"))
                onNodeWithTag("catalog-search-clear").performClick()
                waitUntil(timeoutMillis = 10_000) { controller.state.value.works.items.firstOrNull()?.title == "Lời hẹn từ máy chủ" }
                onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-refresh"))
                refreshed.set(true)
                onNodeWithTag("catalog-refresh").performClick()
                waitUntil(timeoutMillis = 10_000) { controller.state.value.works.items.firstOrNull()?.title == "Lời hẹn vừa cập nhật" }
                onNodeWithTag("catalog-works").performScrollToNode(hasTestTag("catalog-work-$id"))
                onNodeWithText("Lời hẹn vừa cập nhật").assertExists()
                onNodeWithTag("catalog-nav-library").performClick()
                onNodeWithTag("catalog-personal-unavailable").assertExists()
                onNodeWithTag("catalog-nav-discover").performClick()
                onNodeWithTag("catalog-account").performClick()
                onNodeWithTag("catalog-personal-unavailable").assertExists()
                onNodeWithTag("catalog-nav-discover").performClick()
                capture("catalog-production-refresh-vi-390x844.png")
            }
            assertTrue(urls.any { "/search?" in it && "q=m%E1%BA%B7t%20tr%C4%83ng" in it && "kind=story" in it })
        } finally {
            controller.close()
            scope.cancel()
        }
    }
}
