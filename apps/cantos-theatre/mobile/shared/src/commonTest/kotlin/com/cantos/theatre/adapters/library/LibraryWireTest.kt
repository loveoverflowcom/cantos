package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.*
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.*
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

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

    private fun bookmark(name: String) = libraryBookmarkFixtures.getValue("$name.json")
    @Test fun everyBookmarkFixtureUsesGeneratedSchemaAndTypes() {
        assertEquals(7, libraryBookmarkFixtures.size)
        fun <T> roundtrip(file: String, value: T?, encode: (T) -> String) {
            val raw = bookmark(file)
            assertNotNull(value, "$file must decode")
            assertEquals(LibraryCodec.json.parseToJsonElement(raw), LibraryCodec.json.parseToJsonElement(encode(value)), "$file roundtrip keeps explicit null")
        }
        roundtrip("bookmark_write", LibraryCodec.decode<BookmarkWrite>("BookmarkWrite", bookmark("bookmark_write")), { LibraryCodec.json.encodeToString(it) })
        roundtrip("bookmark_delete", LibraryCodec.decode<BookmarkDelete>("BookmarkDelete", bookmark("bookmark_delete")), { LibraryCodec.json.encodeToString(it) })
        for (receipt in listOf("bookmark_accepted", "bookmark_delete_accepted", "bookmark_deleted")) {
            roundtrip(receipt, LibraryCodec.decode<Receipt>("Receipt", bookmark(receipt)), { LibraryCodec.json.encodeToString(it) })
        }
        for (page in listOf("bookmarks", "bookmarks_empty")) {
            roundtrip(page, LibraryCodec.decode<BookmarkPage>("BookmarkPage", bookmark(page)), { LibraryCodec.json.encodeToString(it) })
        }
        val refused = assertNotNull(LibraryCodec.decode<Receipt>("Receipt", bookmark("bookmark_deleted")))
        assertEquals("bookmark_deleted", refused.outcome)
        assertEquals("keep_server", refused.resolution)
        assertTrue(assertIs<MutationState.Bookmark>(refused.state).current.deleted)
    }
    @Test fun bookmarkContractIsStrictAboutAuthorityLabelsNullablesAndBounds() {
        val write = LibraryCodec.json.parseToJsonElement(bookmark("bookmark_write")).jsonObject
        for (member in listOf("actor_id", "listener_id", "label", "note", "revision")) {
            assertNull(LibraryCodec.decode<BookmarkWrite>("BookmarkWrite", JsonObject(write + (member to JsonPrimitive("x"))).toString()), member)
        }
        assertNull(LibraryCodec.decode<BookmarkWrite>("BookmarkWrite", JsonObject(write - "bookmark_id").toString()))
        assertNull(LibraryCodec.decode<BookmarkWrite>("BookmarkWrite", JsonObject(write + ("bookmark_id" to JsonPrimitive("NOT-A-UUID"))).toString()))
        assertNull(LibraryCodec.decode<BookmarkWrite>("BookmarkWrite", JsonObject(write + ("position_ms" to JsonPrimitive(-1))).toString()))
        val page = LibraryCodec.json.parseToJsonElement(bookmark("bookmarks")).jsonObject
        val item = page.getValue("items").jsonArray.first().jsonObject
        assertNull(LibraryCodec.decode<BookmarkPage>("BookmarkPage", JsonObject(page + ("items" to JsonArray(listOf(JsonObject(item - "presentation"))))).toString()), "missing is not null")
        assertNull(LibraryCodec.decode<BookmarkPage>("BookmarkPage", JsonObject(page + ("items" to JsonArray(List(51) { item }))).toString()), "at most 50 items")
        val unavailable = assertNotNull(LibraryCodec.decode<BookmarkPage>("BookmarkPage", bookmark("bookmarks")))
        assertNull(unavailable.items[1].presentation, "an unavailable release keeps the bookmark with null metadata")
        assertEquals(unavailable.items[0].bookmark.audio, unavailable.items[1].bookmark.audio)
    }
    private fun presented(name: String) = libraryPresentationFixtures.getValue("$name.json")
    private fun JsonObject.item() = getValue("items").jsonArray.single().jsonObject
    private fun JsonObject.withItem(edit: (JsonObject) -> JsonObject) = JsonObject(this + ("items" to JsonArray(listOf(edit(item()))))).toString()
    @Test fun everyOfficialPresentationFixtureUsesGeneratedSchemaAndTypes() {
        assertEquals(6, libraryPresentationFixtures.size)
        for ((file, type) in listOf("presented_continue" to "PresentedContinuePage", "presented_continue_unavailable" to "PresentedContinuePage",
            "presented_saved" to "PresentedSavedPage", "presented_saved_unavailable" to "PresentedSavedPage",
            "presented_history" to "PresentedHistoryPage", "presented_history_unavailable" to "PresentedHistoryPage")) {
            val raw = presented(file)
            val value: Any = when (type) {
                "PresentedContinuePage" -> assertNotNull(LibraryCodec.decode<PresentedContinuePage>(type, raw), "$file must decode")
                "PresentedSavedPage" -> assertNotNull(LibraryCodec.decode<PresentedSavedPage>(type, raw), "$file must decode")
                else -> assertNotNull(LibraryCodec.decode<PresentedHistoryPage>(type, raw), "$file must decode")
            }
            val encoded = when (value) {
                is PresentedContinuePage -> LibraryCodec.json.encodeToString(value)
                is PresentedSavedPage -> LibraryCodec.json.encodeToString(value)
                else -> LibraryCodec.json.encodeToString(value as PresentedHistoryPage)
            }
            assertEquals(LibraryCodec.json.parseToJsonElement(raw), LibraryCodec.json.parseToJsonElement(encoded), "$file roundtrip keeps explicit null")
        }
    }
    @Test fun presentationNullIsDistinctFromMissingAndFromTheDefaultShape() {
        val available = assertNotNull(LibraryCodec.decode<PresentedContinuePage>("PresentedContinuePage", presented("presented_continue")))
        assertEquals("Một tiếng chuông trong sương", available.items.single().presentation?.unit_title)
        assertEquals("vi-VN", available.items.single().presentation?.work?.language)
        val unavailable = assertNotNull(LibraryCodec.decode<PresentedContinuePage>("PresentedContinuePage", presented("presented_continue_unavailable")))
        assertNull(unavailable.items.single().presentation)
        val raw = LibraryCodec.json.parseToJsonElement(presented("presented_continue_unavailable")).jsonObject
        assertNull(LibraryCodec.decode<PresentedContinuePage>("PresentedContinuePage", raw.withItem { JsonObject(it - "presentation") }), "missing is not null")
        // The default fixtures have no presentation key at all: they are not Presented pages.
        assertNull(LibraryCodec.decode<PresentedContinuePage>("PresentedContinuePage", libraryFixtures.getValue("continue.json")))
        assertNull(LibraryCodec.decode<PresentedSavedPage>("PresentedSavedPage", libraryFixtures.getValue("saved.json")))
        assertNull(LibraryCodec.decode<PresentedHistoryPage>("PresentedHistoryPage", libraryFixtures.getValue("history.json")))
        // ... and a presented page is not a default page: strict readers keep refusing the new field.
        assertNull(LibraryCodec.decode<ContinuePage>("ContinuePage", presented("presented_continue")))
    }
    @Test fun presentationScalarsAreBoundedByTheSchema() {
        val page = LibraryCodec.json.parseToJsonElement(presented("presented_continue")).jsonObject
        fun withPresentation(edit: (JsonObject) -> JsonObject) = page.withItem { JsonObject(it + ("presentation" to edit(it.getValue("presentation").jsonObject))) }
        fun withWork(edit: (JsonObject) -> JsonObject) = withPresentation { p -> JsonObject(p + ("work" to edit(p.getValue("work").jsonObject))) }
        fun decodes(raw: String) = LibraryCodec.decode<PresentedContinuePage>("PresentedContinuePage", raw) != null
        assertTrue(decodes(withWork { JsonObject(it + ("title" to JsonPrimitive("Đ".repeat(512)))) }), "512 characters is the limit")
        assertTrue(!decodes(withWork { JsonObject(it + ("title" to JsonPrimitive("Đ".repeat(513)))) }), "oversized Work title")
        assertTrue(!decodes(withPresentation { JsonObject(it + ("unit_title" to JsonPrimitive("a".repeat(513)))) }), "oversized unit title")
        assertTrue(!decodes(withWork { JsonObject(it + ("language" to JsonPrimitive("v".repeat(64)))) }), "oversized language")
        assertTrue(!decodes(withWork { JsonObject(it + ("kind" to JsonPrimitive("audiobook"))) }), "closed kind")
        assertTrue(!decodes(withWork { JsonObject(it + ("title" to JsonPrimitive(7))) }), "wrong scalar type")
        assertTrue(!decodes(withWork { JsonObject(it + ("work_id" to JsonPrimitive("NOT-A-UUID"))) }), "identity pattern")
        assertTrue(!decodes(withWork { JsonObject(it + ("studio_only" to JsonPrimitive("leak"))) }), "unknown member")
        assertTrue(!decodes(withWork { JsonObject(it - "language") }), "required member")
        assertTrue(!decodes(JsonObject(page + ("next_cursor" to JsonPrimitive("C".repeat(64)))).toString()), "cursor is 64 lower-case hex digits")
        assertTrue(!decodes(JsonObject(page + ("next_cursor" to JsonPrimitive("c".repeat(63)))).toString()))
        assertTrue(decodes(JsonObject(page + ("next_cursor" to JsonPrimitive("c".repeat(64)))).toString()))
        assertTrue(!decodes(JsonObject(page + ("items" to JsonArray(List(51) { page.item() }))).toString()), "at most 50 items")
    }
}
