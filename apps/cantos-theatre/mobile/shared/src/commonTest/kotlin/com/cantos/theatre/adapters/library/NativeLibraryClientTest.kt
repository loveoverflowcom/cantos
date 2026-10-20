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
    private fun presented(name: String) = libraryPresentationFixtures.getValue("$name.json")
    @Test fun pagesAskForPresentationAndReturnActualRowsAndOpaqueCursorWithBearerSessionFence() = runTest {
        val http = Http(); val client = client(http)
        val saved = Json.parseToJsonElement(presented("presented_saved")).jsonObject
        val cursor = "c".repeat(64)
        http.response = NativeHttpResponse(200, "application/json", JsonObject(saved + ("next_cursor" to JsonPrimitive(cursor))).toString(), null, session)
        val result = assertIs<LibraryResult.Value<com.cantos.theatre.adapters.library.wire.PresentedSavedPage>>(client.savedPage(auth))
        assertEquals(cursor, result.value.next_cursor)
        assertEquals(saved.getValue("items").jsonArray.size, result.value.items.size)
        assertTrue(result.value.items.all { it.saved.saved })
        assertEquals("Ánh đèn cuối sân khấu — Một lời hẹn chưa nói", result.value.items.single().presentation?.title)
        assertEquals(auth.bearer, http.requests.single().bearer)
        assertEquals(session, http.requests.single().expectedSession)
        assertTrue(http.requests.single().url.endsWith("/library/saved?limit=20&presentation=1"))
        client.savedPage(auth, cursor)
        assertTrue(http.requests.last().url.endsWith("?limit=20&presentation=1&cursor=$cursor"))
        http.response = NativeHttpResponse(200, "application/json", presented("presented_continue"), null, session)
        assertIs<LibraryResult.Value<*>>(client.continuePage(auth))
        assertTrue(http.requests.last().url.endsWith("/library/continue?limit=20&presentation=1"))
        http.response = NativeHttpResponse(200, "application/json", presented("presented_history"), null, session)
        assertIs<LibraryResult.Value<*>>(client.historyPage(auth))
        assertTrue(http.requests.last().url.endsWith("/library/history?limit=20&presentation=1"))
        http.response = NativeHttpResponse(200, "application/json", libraryFixtures.getValue("progress_read.json"), null, session)
        assertIs<LibraryResult.Value<*>>(client.progress(auth, id))
        assertFalse(http.requests.last().url.contains("presentation"), "single-record reads never opt in")
    }
    @Test fun unavailablePresentationIsAValidNullNotAProtocolFailure() = runTest {
        val http = Http(); val client = client(http)
        for ((name, call) in listOf<Pair<String, suspend () -> LibraryResult<*>>>(
            "presented_continue_unavailable" to { client.continuePage(auth) },
            "presented_saved_unavailable" to { client.savedPage(auth) },
            "presented_history_unavailable" to { client.historyPage(auth) },
        )) {
            http.response = NativeHttpResponse(200, "application/json", presented(name), null, session)
            assertIs<LibraryResult.Value<*>>(call(), name)
        }
    }
    @Test fun aPageWithoutRequiredPresentationOrFromTheDefaultContractIsAProtocolFailure() = runTest {
        val http = Http(); val client = client(http)
        // A default-shape page (no presentation key) answered to an opted-in request is not conformant.
        http.response = NativeHttpResponse(200, "application/json", libraryFixtures.getValue("continue.json"), null, session)
        assertEquals("protocol", assertIs<LibraryResult.Failed>(client.continuePage(auth)).code)
        http.response = NativeHttpResponse(200, "application/json", libraryFixtures.getValue("saved.json"), null, session)
        assertEquals("protocol", assertIs<LibraryResult.Failed>(client.savedPage(auth)).code)
        http.response = NativeHttpResponse(200, "application/json", libraryFixtures.getValue("history.json"), null, session)
        assertEquals("protocol", assertIs<LibraryResult.Failed>(client.historyPage(auth)).code)
        // Missing is not null: dropping the key from an otherwise valid presented item is rejected.
        val page = Json.parseToJsonElement(presented("presented_continue_unavailable")).jsonObject
        val item = page.getValue("items").jsonArray.single().jsonObject
        http.response = NativeHttpResponse(200, "application/json",
            JsonObject(page + ("items" to JsonArray(listOf(JsonObject(item - "presentation"))))).toString(), null, session)
        assertEquals("protocol", assertIs<LibraryResult.Failed>(client.continuePage(auth)).code)
    }
    @Test fun presentationNamingAnotherReleaseOrWorkIsAProtocolFailureNeverALabel() = runTest {
        val http = Http(); val client = client(http)
        val other = "00000000-0000-4000-8000-0000000000ff"
        fun swap(name: String, edit: (JsonObject) -> JsonObject): String {
            val page = Json.parseToJsonElement(presented(name)).jsonObject
            val item = page.getValue("items").jsonArray.single().jsonObject
            return JsonObject(page + ("items" to JsonArray(listOf(edit(item))))).toString()
        }
        fun JsonObject.with(key: String, value: JsonElement) = JsonObject(this + (key to value))
        fun JsonObject.nested(key: String, edit: (JsonObject) -> JsonObject) = with(key, edit(getValue(key).jsonObject))
        // Another release of the same unit supplies the metadata.
        http.response = NativeHttpResponse(200, "application/json", swap("presented_continue") { item ->
            item.nested("presentation") { p -> p.nested("work") { it.with("publication_id", JsonPrimitive(other)) } } }, null, session)
        assertEquals("protocol", assertIs<LibraryResult.Failed>(client.continuePage(auth)).code)
        // Another unit.
        http.response = NativeHttpResponse(200, "application/json", swap("presented_continue") { item ->
            item.nested("presentation") { it.with("unit_id", JsonPrimitive(other)) } }, null, session)
        assertEquals("protocol", assertIs<LibraryResult.Failed>(client.continuePage(auth)).code)
        // History item metadata for another Work.
        http.response = NativeHttpResponse(200, "application/json", swap("presented_history") { item ->
            item.nested("presentation") { p -> p.nested("work") { it.with("work_id", JsonPrimitive(other)) } } }, null, session)
        assertEquals("protocol", assertIs<LibraryResult.Failed>(client.historyPage(auth)).code)
        // Saved metadata for another Work.
        http.response = NativeHttpResponse(200, "application/json", swap("presented_saved") { item ->
            item.nested("presentation") { it.with("work_id", JsonPrimitive(other)) } }, null, session)
        assertEquals("protocol", assertIs<LibraryResult.Failed>(client.savedPage(auth)).code)
        // The exact release still decodes.
        http.response = NativeHttpResponse(200, "application/json", presented("presented_continue"), null, session)
        assertIs<LibraryResult.Value<*>>(client.continuePage(auth))
    }
    @Test fun anInvalidCursorNeverReachesTheNetworkForAnySection() = runTest {
        val http = Http(); val client = client(http)
        assertEquals("invalid_cursor", assertIs<LibraryResult.Failed>(client.continuePage(auth, "not-a-cursor")).code)
        assertEquals("invalid_cursor", assertIs<LibraryResult.Failed>(client.savedPage(auth, "C".repeat(64))).code)
        assertEquals("invalid_cursor", assertIs<LibraryResult.Failed>(client.historyPage(auth, "c".repeat(63))).code)
        assertTrue(http.requests.isEmpty())
    }
    @Test fun oversizedPageIsRejectedByBytesAndByItemCount() = runTest {
        val http = Http(); val client = client(http)
        val page = Json.parseToJsonElement(presented("presented_saved")).jsonObject
        val item = page.getValue("items").jsonArray.single()
        http.response = NativeHttpResponse(200, "application/json", JsonObject(page + ("items" to JsonArray(List(51) { item }))).toString(), null, session)
        assertEquals("protocol", assertIs<LibraryResult.Failed>(client.savedPage(auth)).code)
        http.response = NativeHttpResponse(200, "application/json", " ".repeat(1_048_577) + presented("presented_saved"), null, session)
        assertEquals("protocol", assertIs<LibraryResult.Failed>(client.savedPage(auth)).code)
        http.response = NativeHttpResponse(200, "application/json", JsonObject(page + ("items" to JsonArray(List(50) { item }))).toString(), null, session)
        assertEquals(50, assertIs<LibraryResult.Value<com.cantos.theatre.adapters.library.wire.PresentedSavedPage>>(client.savedPage(auth)).value.items.size)
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
    @Test fun ownerFenceIsNonretryableAndPreservesUnknownMutationForReceiptRecovery() = runTest {
        val http = Http(); val client = client(http)
        http.response = NativeHttpResponse(0, null, null, "session_changed")
        val page = assertIs<LibraryResult.Failed>(client.savedPage(auth))
        assertEquals("session_changed", page.code)
        assertFalse(page.retryable)
        assertFalse(page.unknownOutcome)
        val body = libraryFixtures.getValue("history_clear.json")
        val operation = Json.parseToJsonElement(body).jsonObject.getValue("operation").jsonObject.getValue("operation_id").jsonPrimitive.content
        val mutation = assertIs<LibraryResult.Failed>(client.mutate(auth,
            LocalLibraryIntent(operation, "history", "POST", "/library/history/clear", body)))
        assertEquals("session_changed", mutation.code)
        assertFalse(mutation.retryable)
        assertTrue(mutation.unknownOutcome, "a discarded response cannot prove the mutation did not commit")
    }
    @Test fun discardedRotatedLeaseDoesNotInvalidateItsHealthyReplacementAndReconcilesMutations() = runTest {
        val http = Http(); val client = client(http)
        http.response = NativeHttpResponse(0, null, null, "authority_changed")
        val page = assertIs<LibraryResult.Failed>(client.savedPage(auth))
        assertEquals("authority_changed", page.code)
        assertTrue(page.retryable)
        assertFalse(page.unknownOutcome)
        val body = libraryFixtures.getValue("history_clear.json")
        val operation = Json.parseToJsonElement(body).jsonObject.getValue("operation").jsonObject.getValue("operation_id").jsonPrimitive.content
        val mutation = assertIs<LibraryResult.Failed>(client.mutate(auth,
            LocalLibraryIntent(operation, "history", "POST", "/library/history/clear", body)))
        assertEquals("authority_changed", mutation.code)
        assertTrue(mutation.retryable)
        assertTrue(mutation.unknownOutcome)
    }
    @Test fun mutationCannotEscapeTheatreLibraryOrChangeOperationBinding() = runTest {
        val http = Http(); val client = client(http)
        assertEquals("invalid_request", assertIs<LibraryResult.Failed>(client.mutate(auth, LocalLibraryIntent(id, "history", "POST", "/../studio/publish", "{}"))).code)
        assertEquals("invalid_request", assertIs<LibraryResult.Failed>(client.mutate(auth, LocalLibraryIntent(id, "history", "POST", "/library/history/clear", libraryFixtures.getValue("history_clear.json")))).code)
        assertTrue(http.requests.isEmpty())
    }
}
