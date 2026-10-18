package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.CatalogHttpResponse
import com.cantos.theatre.adapters.http.CatalogHttpTransport
import com.cantos.theatre.adapters.http.PublicCatalogHttpAdapter
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.ports.CatalogCacheStatus
import com.cantos.theatre.ports.PublicCatalogRead
import kotlinx.coroutines.test.runTest
import java.nio.file.Files
import kotlin.test.*

class AccountCatalogCacheTest {
    @Test fun accountCacheSurvivesSqliteReopenAndInvalidationRetainsLastValidContent() {
        val folder = Files.createTempDirectory("cantos-catalog-ssot").toFile()
        try {
            val path = folder.resolve("catalog.sqlite").absolutePath
            var account: String? = "A"
            val first = desktopLibraryStore(path)
            val cache = AccountCatalogCache(first) { account }
            cache.put("/works", "validated", "A")
            assertEquals("validated", cache.read("/works", "A"))
            account = "B"
            assertNull(cache.read("/works", "B"))
            cache.put("/works", "late-A-response", "A")
            assertNull(cache.read("/works", "B"))
            first.close()
            val second = desktopLibraryStore(path)
            account = "A"
            val restored = AccountCatalogCache(second) { account }
            assertEquals("validated", restored.read("/works", "A"))
            restored.invalidate()
            assertNull(restored.read("/works", "A"))
            assertEquals("validated", second.transaction("A") { record("catalog", "/works")?.body })
            account = null
            restored.put("/works", "guest", null)
            assertNull(restored.read("/works", null))
            second.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun validatedHttpCatalogReadsCacheWithoutNetworkAndExplicitRefreshCanFailHonestly() = runTest {
        val folder = Files.createTempDirectory("cantos-catalog-http").toFile()
        try {
            val store = desktopLibraryStore(folder.resolve("catalog.sqlite").absolutePath)
            val cache = AccountCatalogCache(store) { "A" }
            var requests = 0
            var fail = false
            val response = """{"items":[{"work_id":"10000000-0000-0000-0000-000000000001","kind":"story","title":"Một lời hẹn","description":"Nghe bằng tiếng Việt","language":"vi","first_published_at_ms":1}],"next_cursor":null}"""
            val api = PublicCatalogHttpAdapter(ApiOrigin.parse("https://api.example"), CatalogHttpTransport {
                requests += 1
                if (fail) error("offline")
                CatalogHttpResponse(200, "application/json", response.encodeToByteArray())
            }, cache)
            assertIs<PublicCatalogRead.Ready<*>>(api.works(null, PublicCatalogQuery.All))
            assertEquals(1, requests)
            val cached = assertIs<PublicCatalogRead.Ready<com.cantos.theatre.domain.catalog.PublicWorkPage>>(api.works(null, PublicCatalogQuery.All))
            assertEquals(1, requests)
            assertNull(cached.value.next, "expired opaque cursors never persist")
            assertEquals(CatalogCacheStatus.Cached, cache.status.value)
            cache.invalidate(); fail = true
            assertEquals(PublicCatalogRead.Transport, api.works(null, PublicCatalogQuery.All))
            assertEquals(2, requests)
            store.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun nonterminalPageIsNeverPersistedAsTruncatedTerminalSuccess() = runTest {
        val folder = Files.createTempDirectory("cantos-catalog-walk").toFile()
        try {
            val store = desktopLibraryStore(folder.resolve("catalog.sqlite").absolutePath)
            val cache = AccountCatalogCache(store) { "A" }
            var requests = 0
            val page = """{"items":[],"next_cursor":"lc1_${"a".repeat(64)}"}"""
            val api = PublicCatalogHttpAdapter(ApiOrigin.parse("https://api.example"), CatalogHttpTransport {
                requests += 1
                CatalogHttpResponse(200, "application/json", page.encodeToByteArray())
            }, cache)
            assertIs<PublicCatalogRead.Ready<*>>(api.works(null, PublicCatalogQuery.All))
            assertIs<PublicCatalogRead.Ready<*>>(api.works(null, PublicCatalogQuery.All))
            assertEquals(2, requests)
            assertTrue(store.transaction("A") { records().none { it.resource == "catalog" } })
            store.close()
        } finally { folder.deleteRecursively() }
    }
}
