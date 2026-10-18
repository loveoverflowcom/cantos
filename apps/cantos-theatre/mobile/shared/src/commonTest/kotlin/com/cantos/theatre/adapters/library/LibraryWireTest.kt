package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.*
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.*
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class LibraryWireTest {
    private inline fun <reified T> check(file: String, type: String) {
        val raw = libraryFixtures.getValue("$file.json")
        val value = assertNotNull(LibraryCodec.decode<T>(type, raw), "$file must decode")
        assertEquals(LibraryCodec.json.parseToJsonElement(raw), LibraryCodec.json.parseToJsonElement(LibraryCodec.json.encodeToString(value)), "$file roundtrip")
    }
    @Test fun everyCanonicalFixtureUsesGeneratedSchemaAndTypes() {
        check<ProgressWrite>("progress_write", "ProgressWrite")
        check<SavedWrite>("saved_write", "SavedWrite")
        check<HistoryStart>("history_start", "HistoryStart")
        check<HistoryClear>("history_clear", "HistoryClear")
        check<ContinuePage>("continue", "ContinuePage")
        check<ContinuePage>("empty", "ContinuePage")
        check<SavedPage>("saved", "SavedPage")
        check<HistoryPage>("history", "HistoryPage")
        check<MutationState>("progress_read", "MutationState")
        listOf("accepted", "progress_conflict", "epoch_conflict").forEach { check<Receipt>(it, "Receipt") }
        val errors = libraryFixtures.keys - setOf("progress_write.json", "saved_write.json", "history_start.json", "history_clear.json", "continue.json", "empty.json", "saved.json", "history.json", "progress_read.json", "accepted.json", "progress_conflict.json", "epoch_conflict.json")
        errors.forEach { check<LibraryError>(it.removeSuffix(".json"), "LibraryError") }
        assertEquals(22, libraryFixtures.size)
    }
    @Test fun unknownMembersMissingNullablesAndInvalidScalarBoundsAreRejected() {
        val raw = LibraryCodec.json.parseToJsonElement(libraryFixtures.getValue("progress_write.json")).jsonObject
        assertNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", JsonObject(raw + ("actor_id" to JsonPrimitive("attacker"))).toString()))
        val op = raw.getValue("operation").jsonObject
        assertNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", JsonObject(raw + ("operation" to JsonObject(op - "device_reported_at_ms"))).toString()))
        assertNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", JsonObject(raw + ("base_revision" to JsonPrimitive(-1))).toString()))
        val start = LibraryCodec.json.parseToJsonElement(libraryFixtures.getValue("history_start.json")).jsonObject
        assertNull(LibraryCodec.decode<HistoryStart>("HistoryStart", JsonObject(start + ("event" to JsonPrimitive("play_requested"))).toString()))
    }
}
