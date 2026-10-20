package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.AudioPresentation
import com.cantos.theatre.adapters.library.wire.BookmarkRecord
import com.cantos.theatre.adapters.library.wire.WorkPresentation
import com.cantos.theatre.adapters.library.wire.AudioIdentity as WireAudio
import com.cantos.theatre.application.library.NativeLibraryController
import com.cantos.theatre.domain.library.BookmarkSync
import com.cantos.theatre.domain.library.LibraryResume
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.library.valueOrNull
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.ports.LibraryLocalPort
import com.cantos.theatre.ports.NativeSession
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * Release-bound bookmarks through the production sync adapter, codec and SQLite store against a stateful
 * bookmark-authority double: local-first durability, offline and lost-response delivery, tombstone finality,
 * two devices of one account, account fences and failure states. The server is a double, never API evidence.
 */
@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class BookmarkSyncTest {
    private val actorA = "actor-A"
    private val actorB = "actor-B"
    private val sessionA = "00000000-0000-0000-0000-000000000001"
    private val sessionB = "00000000-0000-0000-0000-000000000002"
    private fun uuid(group: Int, n: Int) = "${group}0000000-0000-4000-8000-${n.toString().padStart(12, '0')}"
    private fun audio(n: Int, publication: Int = n, durationMs: Long = 600_000) = requireNotNull(
        AudioIdentity.of(uuid(1, n), uuid(2, n), uuid(3, publication), uuid(4, publication), "a".repeat(64), durationMs))
    private fun wire(a: AudioIdentity) = WireAudio(a.workId, a.unitId, a.publicationId, a.renditionId, a.sha256, a.durationMs)
    private fun presentation(a: AudioIdentity, title: String = "Tập ${a.unitId.takeLast(2)}") =
        AudioPresentation(WorkPresentation(a.workId, a.publicationId, "Truyện ${a.workId.takeLast(2)}", "story", "vi-VN"), a.unitId, title)

    private var sequence = 10
    private fun id(): String = "90000000-0000-0000-0000-${sequence++.toString().padStart(12, '0')}"
    private fun TestScope.controller(api: LibraryApi, store: LibraryLocalPort, now: () -> Long = { 1_000 }): NativeLibraryController =
        nativeLibraryController(backgroundScope, api, store, ::id, nowMs = now)
    private fun NativeLibraryController.enterAs(actor: String, session: String = sessionA) =
        enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64))
    private fun withFolder(block: (java.io.File) -> Unit) {
        val folder = Files.createTempDirectory("cantos-bookmarks").toFile()
        try { block(folder) } finally { folder.deleteRecursively() }
    }
    private fun java.io.File.store(name: String = "db.sqlite") = desktopLibraryStore(resolve(name).absolutePath)
    private val LibraryState.rows get() = bookmarks.map { it.id to it.sync }

    @Test fun a_bookmark_is_durable_before_the_network_synced_after_it_and_survives_a_restart() = withFolder { folder ->
        runTest {
            val server = BookmarkServer().apply { presentation = { presentation(audio(1)) } }
            val first = controller(server, folder.store())
            first.enterAs(actorA); runCurrent()
            server.unavailable = true
            val id = assertNotNull(first.createBookmark(audio(1), 15_000))
            assertEquals(listOf(id to BookmarkSync.Pending), first.state.value.rows, "shown at once, before any delivery")
            runCurrent()
            assertEquals(listOf(id to BookmarkSync.Offline), first.state.value.rows, "the failed attempt is offline, not lost")
            assertTrue(first.state.value.offline)
            assertEquals(1, first.state.value.pending)
            server.unavailable = false
            first.drain(); runCurrent()
            assertEquals(listOf(id to BookmarkSync.Synced), first.state.value.rows)
            assertEquals(0, first.state.value.pending)
            assertEquals(listOf(id), server.live(actorA).map { it.bookmark_id })
            assertEquals(15_000, first.state.value.bookmarks.single().positionMs)
            assertEquals(audio(1), first.state.value.bookmarks.single().audio)
            assertEquals("Tập 01", first.state.value.bookmarks.single().presentation.valueOrNull?.unitTitle)
            first.close()
            // A new process reads its own durable rows with no network at all.
            server.unavailable = true
            val second = controller(server, folder.store())
            second.enterAs(actorA)
            assertEquals(listOf(id to BookmarkSync.Synced), second.state.value.rows)
            second.close()
        }
    }

    @Test fun a_pending_bookmark_survives_a_restart_and_is_delivered_once() = withFolder { folder ->
        runTest {
            val server = BookmarkServer().apply { unavailable = true }
            val first = controller(server, folder.store())
            first.enterAs(actorA); runCurrent()
            val id = assertNotNull(first.createBookmark(audio(1), 30_000)); runCurrent()
            first.close()
            val second = controller(server, folder.store())
            second.enterAs(actorA)
            assertEquals(listOf(id to BookmarkSync.Pending), second.state.value.rows, "the durable intent is shown again after restart")
            server.unavailable = false
            second.drain(); runCurrent()
            advanceTimeBy(60_000); runCurrent()
            assertEquals(listOf(id to BookmarkSync.Synced), second.state.value.rows)
            assertEquals(1, server.live(actorA).size)
            assertEquals(1, server.account(actorA).receipts.size, "one committed operation, one receipt")
            second.close()
        }
    }

    @Test fun a_lost_response_reads_the_receipt_and_never_creates_a_duplicate() = withFolder { folder ->
        runTest {
            val server = BookmarkServer().apply { loseNextResponse = true }
            val library = controller(server, folder.store())
            library.enterAs(actorA); runCurrent()
            val id = assertNotNull(library.createBookmark(audio(1), 15_000))
            runCurrent()
            advanceTimeBy(5_000); runCurrent()
            assertEquals(1, server.live(actorA).size, "the committed bookmark is the only one")
            assertEquals(listOf(id to BookmarkSync.Synced), library.state.value.rows)
            assertEquals(1, server.calls.count { it.startsWith("PUT") }, "the identical request is never re-sent once its receipt is readable")
            assertEquals(0, library.state.value.pending)
            library.close()
        }
    }

    @Test fun pressing_twice_at_the_same_release_position_is_one_bookmark() = withFolder { folder ->
        runTest {
            val server = BookmarkServer()
            val library = controller(server, folder.store())
            library.enterAs(actorA); runCurrent()
            val first = assertNotNull(library.createBookmark(audio(1), 15_000))
            val second = assertNotNull(library.createBookmark(audio(1), 15_000))
            runCurrent()
            assertEquals(first, second)
            val third = assertNotNull(library.createBookmark(audio(1), 15_000))
            assertEquals(first, third, "also once it is synced")
            runCurrent()
            assertEquals(1, server.live(actorA).size)
            assertNotEquals(first, assertNotNull(library.createBookmark(audio(1), 15_001)), "another position is another bookmark")
            assertNotEquals(first, assertNotNull(library.createBookmark(audio(1, publication = 7), 15_000)), "another release is another bookmark")
            library.close()
        }
    }

    @Test fun a_position_outside_the_exact_audio_is_never_recorded() = withFolder { folder ->
        runTest {
            val library = controller(BookmarkServer(), folder.store())
            library.enterAs(actorA); runCurrent()
            assertNull(library.createBookmark(audio(1, durationMs = 60_000), 60_001))
            assertNull(library.createBookmark(audio(1), -1))
            assertTrue(library.state.value.bookmarks.isEmpty())
            assertEquals(0, library.state.value.pending)
            assertNotNull(library.createBookmark(audio(1, durationMs = 60_000), 60_000), "the exact end is inside [0, duration]")
            library.close()
        }
    }

    @Test fun a_deleted_bookmark_is_gone_at_once_synced_and_never_returned_by_a_late_or_replayed_response() = withFolder { folder ->
        runTest {
            val server = BookmarkServer()
            val library = controller(server, folder.store())
            library.enterAs(actorA); runCurrent()
            val id = assertNotNull(library.createBookmark(audio(1), 15_000)); runCurrent()
            assertEquals(listOf(id to BookmarkSync.Synced), library.state.value.rows)
            // A page whose answer was fixed before the delete reaches the server arrives after it was acknowledged.
            server.pageGate = CompletableDeferred()
            library.refresh(LibrarySection.Bookmarks); runCurrent()
            val staleGate = server.pageGate
            server.pageGate = null
            library.deleteBookmark(id)
            assertTrue(library.state.value.bookmarks.isEmpty(), "hidden the instant the listener deletes it")
            runCurrent()
            assertEquals(listOf(id), server.tombstones(actorA).map { it.bookmark_id })
            assertTrue(server.live(actorA).isEmpty())
            assertEquals(0, library.state.value.pending)
            staleGate?.complete(Unit); runCurrent()
            assertTrue(library.state.value.bookmarks.isEmpty(), "a read in flight before the delete cannot bring it back")
            // A lagging or replayed answer that still lists the deleted bookmark is outvoted by this account's deletion.
            server.staleRows = listOf(BookmarkRecord(id, wire(audio(1)), 15_000, false, 1, 1_000))
            library.refresh(); runCurrent()
            assertTrue(library.state.value.bookmarks.isEmpty(), "a stale page cannot bring it back")
            library.nextPage(LibrarySection.Bookmarks); runCurrent()
            assertTrue(library.state.value.bookmarks.isEmpty())
            library.close()
        }
    }

    @Test fun an_offline_delete_hides_the_bookmark_and_is_delivered_on_reconnect_without_resurrection() = withFolder { folder ->
        runTest {
            val server = BookmarkServer()
            val first = controller(server, folder.store())
            first.enterAs(actorA); runCurrent()
            val id = assertNotNull(first.createBookmark(audio(1), 15_000)); runCurrent()
            server.unavailable = true
            first.deleteBookmark(id); runCurrent()
            assertTrue(first.state.value.bookmarks.isEmpty())
            assertEquals(1, first.state.value.pending, "the delete waits, durably")
            first.close()
            val second = controller(server, folder.store())
            second.enterAs(actorA); runCurrent()
            assertTrue(second.state.value.bookmarks.isEmpty(), "still deleted after a restart")
            server.unavailable = false
            second.drain(); runCurrent(); advanceTimeBy(60_000); runCurrent()
            assertTrue(server.live(actorA).isEmpty())
            assertEquals(1, server.tombstones(actorA).size)
            assertEquals(0, second.state.value.pending)
            second.refresh(); runCurrent()
            assertTrue(second.state.value.bookmarks.isEmpty())
            second.close()
        }
    }

    @Test fun deleting_a_never_sent_bookmark_cancels_it_with_no_server_traffic() = withFolder { folder ->
        runTest {
            val server = BookmarkServer()
            val library = controller(server, folder.store())
            library.enterAs(actorA); runCurrent()
            server.calls.clear()
            val id = assertNotNull(library.createBookmark(audio(1), 15_000))
            library.deleteBookmark(id); runCurrent()
            assertTrue(library.state.value.bookmarks.isEmpty())
            assertEquals(0, library.state.value.pending)
            assertEquals(0, server.mutations, "the server never learns of a bookmark that was cancelled before sending")
            assertTrue(server.account(actorA).bookmarks.isEmpty())
            library.close()
        }
    }

    @Test fun two_devices_of_one_account_converge_and_another_account_sees_nothing() = withFolder { folder ->
        runTest {
            val server = BookmarkServer()
            val phone = controller(server, folder.store("phone.sqlite"))
            val tablet = controller(server, folder.store("tablet.sqlite"))
            val stranger = controller(server, folder.store("stranger.sqlite"))
            phone.enterAs(actorA, sessionA); tablet.enterAs(actorA, sessionB); stranger.enterAs(actorB); runCurrent()
            val id = assertNotNull(phone.createBookmark(audio(1), 15_000)); runCurrent()
            tablet.refresh(); stranger.refresh(); runCurrent()
            assertEquals(listOf(id to BookmarkSync.Synced), tablet.state.value.rows, "the same account sees it after sync")
            assertEquals(audio(1), tablet.state.value.bookmarks.single().audio)
            assertTrue(stranger.state.value.bookmarks.isEmpty(), "another account never sees it")
            tablet.deleteBookmark(id); runCurrent()
            phone.refresh(); runCurrent()
            assertTrue(phone.state.value.bookmarks.isEmpty(), "deleting on one device removes it on the other")
            assertTrue(server.live(actorA).isEmpty())
            assertEquals(1, server.tombstones(actorA).size)
            listOf(phone, tablet, stranger).forEach { it.close() }
        }
    }

    @Test fun a_create_refused_for_good_is_failed_until_the_listener_discards_it_and_never_blocks_deleting() = withFolder { folder ->
        runTest {
            val server = BookmarkServer().apply { refuseCreateWith = "not_found" }
            val library = controller(server, folder.store())
            library.enterAs(actorA); runCurrent()
            val id = assertNotNull(library.createBookmark(audio(1), 15_000)); runCurrent()
            assertEquals(listOf(id to BookmarkSync.Failed), library.state.value.rows)
            assertEquals(0, library.state.value.pending)
            assertTrue(server.account(actorA).bookmarks.isEmpty())
            val sent = server.mutations
            library.deleteBookmark(id); runCurrent()
            assertTrue(library.state.value.bookmarks.isEmpty(), "discarding a failed bookmark removes it")
            assertEquals(sent, server.mutations, "discarding a bookmark the server never accepted needs no delete request")
            assertEquals(0, library.state.value.pending)
            library.close()
        }
    }

    @Test fun a_bookmark_deleted_elsewhere_while_this_device_was_offline_is_gone_not_failed_and_not_revived() = withFolder { folder ->
        runTest {
            val server = BookmarkServer()
            val library = controller(server, folder.store())
            library.enterAs(actorA); runCurrent()
            server.unavailable = true
            val id = assertNotNull(library.createBookmark(audio(1), 15_000)); runCurrent()
            // The very bookmark was created and deleted by another device holding the same ID (a replayed outbox).
            server.create(actorA, id, wire(audio(1)), 15_000); server.delete(actorA, id)
            server.unavailable = false
            library.drain(); runCurrent(); advanceTimeBy(60_000); runCurrent()
            assertTrue(library.state.value.bookmarks.isEmpty(), "the tombstone is final")
            assertEquals(0, library.state.value.pending)
            assertTrue(library.state.value.failures.isEmpty(), "a refused resurrection is not a failure to retry")
            assertTrue(server.live(actorA).isEmpty())
            library.refresh(); runCurrent()
            assertTrue(library.state.value.bookmarks.isEmpty())
            library.close()
        }
    }

    @Test fun deleting_an_id_the_server_never_accepted_ends_gone_without_a_failure() = withFolder { folder ->
        runTest {
            val server = BookmarkServer()
            val library = controller(server, folder.store())
            library.enterAs(actorA); runCurrent()
            server.unavailable = true
            val id = assertNotNull(library.createBookmark(audio(1), 15_000)); runCurrent()
            // The create reached the wire once (sent) but the server never committed it; the listener then deletes.
            library.deleteBookmark(id); runCurrent()
            server.unavailable = false
            library.drain(); runCurrent(); advanceTimeBy(60_000); runCurrent()
            assertTrue(library.state.value.bookmarks.isEmpty())
            assertEquals(0, library.state.value.pending)
            assertTrue(library.state.value.failures.isEmpty())
            library.close()
        }
    }

    @Test fun deleting_a_bookmark_the_server_no_longer_knows_ends_gone_without_a_failure_or_a_stuck_outbox() = withFolder { folder ->
        runTest {
            val server = BookmarkServer()
            val library = controller(server, folder.store())
            library.enterAs(actorA); runCurrent()
            val id = assertNotNull(library.createBookmark(audio(1), 15_000)); runCurrent()
            server.account(actorA).bookmarks.clear() // the server has no such bookmark (404 on delete)
            library.deleteBookmark(id); runCurrent()
            assertTrue(library.state.value.bookmarks.isEmpty())
            assertEquals(0, library.state.value.pending)
            assertTrue(library.state.value.failures.isEmpty(), "a 404 on delete means it is already gone")
            assertEquals(1, server.calls.count { it.endsWith("/delete") })
            library.refresh(); runCurrent()
            assertTrue(library.state.value.bookmarks.isEmpty())
            library.close()
        }
    }

    @Test fun pages_append_in_the_servers_order_and_a_first_page_read_replaces_the_rows() = withFolder { folder ->
        runTest {
            val server = BookmarkServer().apply { pageSize = 2 }
            (1..5).forEach { server.create(actorA, uuid(7, it), wire(audio(it)), it * 1_000L) }
            val library = controller(server, folder.store())
            library.enterAs(actorA); runCurrent()
            assertEquals((5 downTo 4).map { uuid(7, it) }, library.state.value.bookmarks.map { it.id })
            library.nextPage(LibrarySection.Bookmarks); runCurrent()
            library.nextPage(LibrarySection.Bookmarks); runCurrent()
            assertEquals((5 downTo 1).map { uuid(7, it) }, library.state.value.bookmarks.map { it.id }, "newest first, each exactly once")
            assertNull(library.state.value.cursors[LibrarySection.Bookmarks])
            server.delete(actorA, uuid(7, 5))
            library.refresh(LibrarySection.Bookmarks); runCurrent()
            assertEquals(listOf(uuid(7, 4), uuid(7, 3)), library.state.value.bookmarks.map { it.id }, "a first-page read replaces the cache")
            library.close()
        }
    }

    @Test fun a_bookmark_keeps_its_exact_release_and_is_marked_unavailable_never_remapped() = withFolder { folder ->
        runTest {
            val old = audio(1, publication = 3)
            val server = BookmarkServer().apply {
                resume = com.cantos.theatre.adapters.library.wire.ResumeStatus.release_replaced
                presentation = { null }
            }
            server.create(actorA, uuid(7, 1), wire(old), 15_000)
            val library = controller(server, folder.store())
            library.enterAs(actorA); runCurrent()
            val row = library.state.value.bookmarks.single()
            assertEquals(old, row.audio)
            assertEquals(LibraryResume.ReleaseReplaced, row.resume)
            assertNull(row.presentation.valueOrNull, "no replacement title labels the old release")
            library.close()
        }
    }

    @Test fun logout_and_an_account_switch_hide_account_data_and_a_late_response_never_leaks_it() = withFolder { folder ->
        runTest {
            val server = BookmarkServer()
            server.create(actorA, uuid(7, 1), wire(audio(1)), 15_000)
            server.create(actorB, uuid(7, 2), wire(audio(2)), 25_000)
            val library = controller(server, folder.store())
            server.pageGate = CompletableDeferred()
            library.enterAs(actorA); runCurrent()          // A's first page is held open
            val held = server.pageGate
            server.pageGate = null
            library.leave()
            assertTrue(library.state.value.bookmarks.isEmpty() && library.state.value.account == null)
            library.enterAs(actorB, sessionB); runCurrent()
            held?.complete(Unit); runCurrent()
            assertEquals(actorB, library.state.value.account)
            assertEquals(listOf(uuid(7, 2)), library.state.value.bookmarks.map { it.id }, "only B's bookmark, A's late page was fenced")
            library.leave()
            assertTrue(library.state.value.bookmarks.isEmpty())
            library.close()
        }
    }

    @Test fun the_outbox_is_account_scoped_so_one_accounts_pending_bookmark_is_never_sent_as_another() = withFolder { folder ->
        runTest {
            val server = BookmarkServer().apply { unavailable = true }
            val library = controller(server, folder.store())
            library.enterAs(actorA); runCurrent()
            assertNotNull(library.createBookmark(audio(1), 15_000)); runCurrent()
            library.leave()
            library.enterAs(actorB, sessionB); runCurrent()
            assertTrue(library.state.value.bookmarks.isEmpty())
            server.unavailable = false
            library.drain(); library.refresh(); runCurrent(); advanceTimeBy(60_000); runCurrent()
            assertTrue(server.account(actorB).bookmarks.isEmpty(), "A's intent is never submitted under B")
            assertTrue(library.state.value.bookmarks.isEmpty())
            library.leave()
            library.enterAs(actorA); runCurrent(); advanceTimeBy(60_000); runCurrent()
            assertEquals(1, server.live(actorA).size, "A's own intent is still durable and delivers for A")
            library.close()
        }
    }
}
