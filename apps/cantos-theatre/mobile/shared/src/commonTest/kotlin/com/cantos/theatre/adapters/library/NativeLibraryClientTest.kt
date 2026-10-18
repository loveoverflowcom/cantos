package com.cantos.theatre.adapters.library

import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.ports.*
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.*
import kotlin.test.*

class NativeLibraryClientTest {
    private val session = "00000000-0000-0000-0000-000000000001"
    private val id = "10000000-0000-0000-0000-000000000001"
    private val auth = LibraryAuthorization("A", session, "b".repeat(64))
    private class Http : NativeHttpPort {
        val requests = mutableListOf<NativeHttpRequest>()
        var response = NativeHttpResponse(0, null, null, "offline")
        override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
            requests += request; completion(response)
            return object : NativeRequestHandle { override fun cancel() = Unit }
        }
    }
    private fun client(http: Http) = NativeLibraryClient(requireNotNull(NativeClientConfiguration.create("https://api.example", "https://idm.example/oauth2/openid/cantos-theatre-native")), http)
    @Test fun pagesReturnActualRowsAndOpaqueCursorWithBearerSessionFence() = runTest {
        val http = Http(); val client = client(http)
        val saved = Json.parseToJsonElement(libraryFixtures.getValue("saved.json")).jsonObject
        val cursor = "c".repeat(64)
        http.response = NativeHttpResponse(200, "application/json", JsonObject(saved + ("next_cursor" to JsonPrimitive(cursor))).toString(), null, session)
        val result = assertIs<LibraryResult.Value<com.cantos.theatre.adapters.library.wire.SavedPage>>(client.savedPage(auth))
        assertEquals(cursor, result.value.next_cursor)
        assertEquals(saved.getValue("items").jsonArray.size, result.value.items.size)
        assertTrue(result.value.items.all { it.saved.saved })
        assertEquals(auth.bearer, http.requests.single().bearer)
        assertEquals(session, http.requests.single().expectedSession)
        client.savedPage(auth, cursor)
        assertTrue(http.requests.last().url.endsWith("?limit=20&cursor=$cursor"))
        http.response = NativeHttpResponse(200, "application/json", libraryFixtures.getValue("continue.json"), null, session)
        assertIs<LibraryResult.Value<*>>(client.continuePage(auth))
        http.response = NativeHttpResponse(200, "application/json", libraryFixtures.getValue("history.json"), null, session)
        assertIs<LibraryResult.Value<*>>(client.historyPage(auth))
        http.response = NativeHttpResponse(200, "application/json", libraryFixtures.getValue("progress_read.json"), null, session)
        assertIs<LibraryResult.Value<*>>(client.progress(auth, id))
    }
    @Test fun mutationsUsePutOrPostAndRetainExactBodyAndUnknownOutcome() = runTest {
        val http = Http(); val client = client(http)
        val body = libraryFixtures.getValue("progress_write.json")
        val parsed = Json.parseToJsonElement(body).jsonObject
        val operation = parsed.getValue("operation").jsonObject.getValue("operation_id").jsonPrimitive.content
        val publication = parsed.getValue("audio").jsonObject.getValue("publication_id").jsonPrimitive.content
        val intent = LocalLibraryIntent(operation, "progress:$publication", "PUT", "/progress/$publication", body)
        val failed = assertIs<LibraryResult.Failed>(client.mutate(auth, intent))
        assertTrue(failed.unknownOutcome)
        assertEquals("PUT", http.requests.single().method)
        assertEquals(body, http.requests.single().body)
        http.response = NativeHttpResponse(200, "application/json", libraryFixtures.getValue("accepted.json"), null, session)
        assertIs<LibraryResult.Value<*>>(client.mutate(auth, intent))
        client.receipt(auth, id)
        assertEquals("GET", http.requests.last().method)
        assertTrue(http.requests.last().url.endsWith("/operations/$id"))
        val clear = libraryFixtures.getValue("history_clear.json")
        val clearOperation = Json.parseToJsonElement(clear).jsonObject.getValue("operation").jsonObject.getValue("operation_id").jsonPrimitive.content
        client.mutate(auth, LocalLibraryIntent(clearOperation, "history", "POST", "/library/history/clear", clear))
        assertEquals("POST", http.requests.last().method)
    }
    @Test fun successfulResponseFromDifferentSessionCannotPopulateLibrary() = runTest {
        val http = Http(); val client = client(http)
        http.response = NativeHttpResponse(200, "application/json", libraryFixtures.getValue("saved.json"), null, id)
        assertEquals("session_changed", assertIs<LibraryResult.Failed>(client.savedPage(auth)).code)
    }
    @Test fun mutationCannotEscapeTheatreLibraryOrChangeOperationBinding() = runTest {
        val http = Http(); val client = client(http)
        assertEquals("invalid_request", assertIs<LibraryResult.Failed>(client.mutate(auth, LocalLibraryIntent(id, "history", "POST", "/../studio/publish", "{}"))).code)
        assertEquals("invalid_request", assertIs<LibraryResult.Failed>(client.mutate(auth, LocalLibraryIntent(id, "history", "POST", "/library/history/clear", libraryFixtures.getValue("history_clear.json")))).code)
        assertTrue(http.requests.isEmpty())
    }
}
