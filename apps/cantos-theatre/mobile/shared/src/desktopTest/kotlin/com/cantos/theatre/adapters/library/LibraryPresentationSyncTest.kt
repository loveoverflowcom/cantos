package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.AudioPresentation
import com.cantos.theatre.adapters.library.wire.Completion
import com.cantos.theatre.adapters.library.wire.ContinueItem
import com.cantos.theatre.adapters.library.wire.HistoryItem
import com.cantos.theatre.adapters.library.wire.HistoryState
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.PresentedContinueItem
import com.cantos.theatre.adapters.library.wire.PresentedContinuePage
import com.cantos.theatre.adapters.library.wire.PresentedHistoryItem
import com.cantos.theatre.adapters.library.wire.PresentedHistoryPage
import com.cantos.theatre.adapters.library.wire.PresentedSavedItem
import com.cantos.theatre.adapters.library.wire.PresentedSavedPage
import com.cantos.theatre.adapters.library.wire.ProgressKind
import com.cantos.theatre.adapters.library.wire.ProgressRecord
import com.cantos.theatre.adapters.library.wire.Receipt
import com.cantos.theatre.adapters.library.wire.ResumeStatus
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.adapters.library.wire.WorkPresentation
import com.cantos.theatre.adapters.library.wire.AudioIdentity as WireAudio
import com.cantos.theatre.application.library.NativeLibraryController
import com.cantos.theatre.domain.library.LibraryLoad
import com.cantos.theatre.domain.library.LibraryPresentation
import com.cantos.theatre.domain.library.LibraryReadFailure
import com.cantos.theatre.domain.library.LibraryResume
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.library.LibraryWorkKind
import com.cantos.theatre.domain.library.valueOrNull
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.ports.LibraryLocalPort
import com.cantos.theatre.ports.LocalLibraryIntent
import com.cantos.theatre.ports.LocalLibraryRecord
import com.cantos.theatre.ports.NativeSession
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.encodeToString
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * The production sync adapter, SQLite store and codec against a scripted API: metadata from
 * `presentation=1` pages reaches Library state for Works no catalog ever listed, and every section
 * pages, fails and retries on its own. The API double is a unit-test double, not API evidence.
 */
@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class LibraryPresentationSyncTest {
    private val actorA = "actor-A"
    private val sessionA = "00000000-0000-0000-0000-000000000001"
    private fun uuid(group: Int, n: Int) = "${group}0000000-0000-4000-8000-${n.toString().padStart(12, '0')}"
    private fun wireAudio(n: Int, publication: Int = n) =
        WireAudio(uuid(1, n), uuid(2, n), uuid(3, publication), uuid(4, publication), "a".repeat(64), 600_000)
    private fun progress(n: Int, order: Long, publication: Int = n, position: Long = 15_000) = ProgressRecord(
        wireAudio(n, publication), position, Completion.in_progress, 1, 1000, order, uuid(5, 1), ProgressKind.checkpoint)
    private fun presentation(n: Int, publication: Int = n, workTitle: String = "Truyện số $n", unitTitle: String = "Tập số $n") =
        AudioPresentation(WorkPresentation(uuid(1, n), uuid(3, publication), workTitle, "story", "vi-VN"), uuid(2, n), unitTitle)
    private fun continueItem(n: Int, order: Long, presented: Boolean = true, resume: ResumeStatus = ResumeStatus.exact_available) =
        PresentedContinueItem(progress(n, order), resume, if (presented) presentation(n) else null)
    private fun savedItem(n: Int, order: Long, presented: Boolean = true) =
        PresentedSavedItem(SavedRecord(uuid(1, n), true, 1, order, 1000), if (presented) WorkPresentation(uuid(1, n), uuid(3, n), "Truyện số $n", "podcast", "vi-VN") else null)
    private fun historyItem(n: Int, order: Long, presented: Boolean = true) =
        PresentedHistoryItem(wireAudio(n), 1000, order, null, ResumeStatus.exact_available, if (presented) presentation(n) else null)

    private class Api : LibraryApi {
        val calls = mutableListOf<String>()
        val continuePages = HashMap<String?, LibraryResult<PresentedContinuePage>>()
        val savedPages = HashMap<String?, LibraryResult<PresentedSavedPage>>()
        val historyPages = HashMap<String?, LibraryResult<PresentedHistoryPage>>()
        var savedGate: CompletableDeferred<Unit>? = null
        override suspend fun continuePage(auth: LibraryAuthorization, cursor: String?): LibraryResult<PresentedContinuePage> {
            calls += "continue:$cursor"
            return continuePages[cursor] ?: LibraryResult.Value(PresentedContinuePage(emptyList(), null))
        }
        override suspend fun savedPage(auth: LibraryAuthorization, cursor: String?): LibraryResult<PresentedSavedPage> {
            calls += "saved:$cursor"
            val answer = savedPages[cursor] ?: LibraryResult.Value(PresentedSavedPage(emptyList(), null))
            savedGate?.await() // the answer is fixed when the request is made, like a server's
            return answer
        }
        override suspend fun historyPage(auth: LibraryAuthorization, cursor: String?): LibraryResult<PresentedHistoryPage> {
            calls += "history:$cursor"
            return historyPages[cursor] ?: LibraryResult.Value(PresentedHistoryPage(HistoryState(0, 0), emptyList(), null))
        }
        val bookmarkPages = HashMap<String?, LibraryResult<com.cantos.theatre.adapters.library.wire.BookmarkPage>>()
        override suspend fun bookmarkPage(auth: LibraryAuthorization, cursor: String?): LibraryResult<com.cantos.theatre.adapters.library.wire.BookmarkPage> =
            bookmarkPages[cursor] ?: LibraryResult.Value(com.cantos.theatre.adapters.library.wire.BookmarkPage(emptyList(), null))
        override suspend fun saved(auth: LibraryAuthorization, work: String) = LibraryResult.Value(SavedRecord(work, false, 0, 0, 0))
        override suspend fun progress(auth: LibraryAuthorization, publication: String) = LibraryResult.Value(MutationState.Progress(null))
        override suspend fun mutate(auth: LibraryAuthorization, intent: LocalLibraryIntent): LibraryResult<Receipt> = LibraryResult.Failed("unavailable", true, true)
        override suspend fun receipt(auth: LibraryAuthorization, operation: String): LibraryResult<Receipt> = LibraryResult.Failed("not_found", false)
    }

    private var sequence = 10
    private fun id(): String = "90000000-0000-0000-0000-${sequence++.toString().padStart(12, '0')}"
    private fun TestScope.controller(api: LibraryApi, store: LibraryLocalPort): NativeLibraryController =
        nativeLibraryController(backgroundScope, api, store, ::id) { 1000 }
    private fun NativeLibraryController.enterAs(actor: String, session: String = sessionA) { enter(NativeSession(actor, session, Long.MAX_VALUE), "b".repeat(64)) }
    private fun workIds(state: LibraryState) = state.continueListening.map { it.audio.workId }

    @Test fun metadataFromTheApiNamesWorksNoCatalogEverListed() = runTest {
        val folder = Files.createTempDirectory("cantos-presented-names").toFile()
        try {
            val api = Api().apply {
                continuePages[null] = LibraryResult.Value(PresentedContinuePage(listOf(continueItem(1, 9)), null))
                savedPages[null] = LibraryResult.Value(PresentedSavedPage(listOf(savedItem(2, 8)), null))
                historyPages[null] = LibraryResult.Value(PresentedHistoryPage(HistoryState(1, 0), listOf(historyItem(3, 7)), null))
            }
            val library = controller(api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath))
            library.enterAs(actorA); runCurrent()
            val state = library.state.value
            val row = state.continueListening.single()
            assertEquals("Truyện số 1", row.presentation.valueOrNull?.work?.title)
            assertEquals("Tập số 1", row.presentation.valueOrNull?.unitTitle)
            assertEquals(uuid(3, 1), row.presentation.valueOrNull?.work?.publicationId)
            assertEquals(wireAudio(1).publication_id, row.audio.publicationId, "metadata rides the exact stored audio")
            assertEquals(15_000, row.positionMs)
            val saved = state.saved.single()
            assertEquals(LibraryWorkKind.Podcast, saved.presentation.valueOrNull?.kind)
            assertEquals("Truyện số 2", saved.presentation.valueOrNull?.title)
            assertEquals("Tập số 3", state.history.single().presentation.valueOrNull?.unitTitle)
            assertEquals(setOf("continue:null", "saved:null", "history:null"), api.calls.toSet())
            assertEquals(3, api.calls.size, "one read per section, no per-Work fetch")
            library.close()
        } finally { folder.deleteRecursively() }
    }

    @Test fun twoPagesPerSectionPageIndependentlyInTheApisOrderWithoutDuplicates() = runTest {
        val folder = Files.createTempDirectory("cantos-presented-pages").toFile()
        try {
            val c1 = "c".repeat(64); val s1 = "d".repeat(64); val h1 = "e".repeat(64)
            val api = Api().apply {
                // Newest first by listener order, deliberately not alphabetical by Work id.
                continuePages[null] = LibraryResult.Value(PresentedContinuePage(listOf(continueItem(9, 100), continueItem(1, 90)), c1))
                continuePages[c1] = LibraryResult.Value(PresentedContinuePage(listOf(continueItem(5, 80), continueItem(1, 90)), null))
                savedPages[null] = LibraryResult.Value(PresentedSavedPage(listOf(savedItem(8, 100), savedItem(2, 90)), s1))
                savedPages[s1] = LibraryResult.Value(PresentedSavedPage(listOf(savedItem(4, 80)), null))
                historyPages[null] = LibraryResult.Value(PresentedHistoryPage(HistoryState(2, 0), listOf(historyItem(7, 100), historyItem(3, 90)), h1))
                historyPages[h1] = LibraryResult.Value(PresentedHistoryPage(HistoryState(2, 0), listOf(historyItem(6, 80)), null))
            }
            val library = controller(api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath))
            library.enterAs(actorA); runCurrent()
            assertEquals(mapOf(LibrarySection.Continue to c1, LibrarySection.Saved to s1, LibrarySection.History to h1), library.state.value.cursors.filterValues { it != null })
            api.calls.clear()

            library.nextPage(LibrarySection.Saved); runCurrent()
            assertEquals(listOf("saved:$s1"), api.calls, "only the Saved section reads")
            assertEquals(listOf(uuid(1, 8), uuid(1, 2), uuid(1, 4)), library.state.value.saved.map { it.workId })
            assertEquals(listOf(uuid(1, 9), uuid(1, 1)), workIds(library.state.value), "other sections are untouched")
            assertNull(library.state.value.cursors[LibrarySection.Saved])
            assertEquals(c1, library.state.value.cursors[LibrarySection.Continue])

            library.nextPage(LibrarySection.Continue); runCurrent()
            library.nextPage(LibrarySection.History); runCurrent()
            assertEquals(listOf(uuid(1, 9), uuid(1, 1), uuid(1, 5)), workIds(library.state.value), "Work 1 appears once")
            assertEquals(listOf(uuid(1, 7), uuid(1, 3), uuid(1, 6)), library.state.value.history.map { it.audio.workId })
            assertTrue(library.state.value.cursors.values.all { it == null })
            assertTrue(LibrarySection.entries.all { library.state.value.section(it).loaded && library.state.value.section(it).failure == null })
            assertTrue(library.state.value.continueListening.all { it.presentation is LibraryPresentation.Available })
            library.close()
        } finally { folder.deleteRecursively() }
    }

    @Test fun aFailedNextPageKeepsItsRowsAndOtherSectionsAndRetryDoesNotDuplicate() = runTest {
        val folder = Files.createTempDirectory("cantos-presented-retry").toFile()
        try {
            val s1 = "d".repeat(64)
            val api = Api().apply {
                continuePages[null] = LibraryResult.Value(PresentedContinuePage(listOf(continueItem(1, 100)), null))
                savedPages[null] = LibraryResult.Value(PresentedSavedPage(listOf(savedItem(2, 100), savedItem(3, 90)), s1))
                savedPages[s1] = LibraryResult.Failed("unavailable", true)
            }
            val library = controller(api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath))
            library.enterAs(actorA); runCurrent()
            library.nextPage(LibrarySection.Saved); runCurrent()
            val failed = library.state.value
            assertEquals(LibraryReadFailure("unavailable", retryable = true, nextPage = true), failed.section(LibrarySection.Saved).failure)
            assertEquals(LibraryLoad.Idle, failed.section(LibrarySection.Saved).loading)
            assertEquals(listOf(uuid(1, 2), uuid(1, 3)), failed.saved.map { it.workId }, "rows already shown are kept")
            assertEquals(s1, failed.cursors[LibrarySection.Saved], "the cursor survives so retry asks for the same page")
            assertNull(failed.section(LibrarySection.Continue).failure)
            assertNull(failed.section(LibrarySection.History).failure)
            assertEquals(listOf(uuid(1, 1)), workIds(failed))

            api.savedPages[s1] = LibraryResult.Value(PresentedSavedPage(listOf(savedItem(4, 80), savedItem(3, 90)), null))
            library.nextPage(LibrarySection.Saved); runCurrent()
            val recovered = library.state.value
            assertNull(recovered.section(LibrarySection.Saved).failure)
            assertEquals(listOf(uuid(1, 2), uuid(1, 3), uuid(1, 4)), recovered.saved.map { it.workId }, "retry adds the page once")
            library.close()
        } finally { folder.deleteRecursively() }
    }

    @Test fun aSectionReadsOnceAtATimeAndAFailedFirstPageIsTheSectionsOwn() = runTest {
        val folder = Files.createTempDirectory("cantos-presented-first").toFile()
        try {
            val s1 = "d".repeat(64)
            val gate = CompletableDeferred<Unit>()
            val api = Api().apply {
                continuePages[null] = LibraryResult.Value(PresentedContinuePage(listOf(continueItem(1, 100)), null))
                savedPages[null] = LibraryResult.Value(PresentedSavedPage(listOf(savedItem(2, 100)), s1))
                savedPages[s1] = LibraryResult.Value(PresentedSavedPage(listOf(savedItem(3, 90)), null))
                historyPages[null] = LibraryResult.Failed("unavailable", true)
            }
            val library = controller(api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath))
            library.enterAs(actorA); runCurrent()
            val history = library.state.value.section(LibrarySection.History)
            assertEquals(LibraryReadFailure("unavailable", true, nextPage = false), history.failure)
            assertFalse(history.loaded, "a failed first page is not an empty list")
            assertTrue(library.state.value.section(LibrarySection.Continue).loaded)
            assertTrue(library.state.value.history.isEmpty())

            api.calls.clear()
            api.savedGate = gate
            library.nextPage(LibrarySection.Saved); runCurrent()
            assertEquals(LibraryLoad.NextPage, library.state.value.section(LibrarySection.Saved).loading)
            assertEquals(LibraryLoad.Idle, library.state.value.section(LibrarySection.Continue).loading)
            assertTrue(library.state.value.refreshing)
            library.nextPage(LibrarySection.Saved); runCurrent()
            assertEquals(listOf("saved:$s1"), api.calls, "a second tap while reading is the same request")
            gate.complete(Unit); runCurrent()
            assertEquals(LibraryLoad.Idle, library.state.value.section(LibrarySection.Saved).loading)
            assertFalse(library.state.value.refreshing)

            api.historyPages[null] = LibraryResult.Value(PresentedHistoryPage(HistoryState(1, 0), listOf(historyItem(7, 100)), null))
            api.calls.clear()
            library.refresh(LibrarySection.History); runCurrent()
            assertEquals(listOf("history:null"), api.calls, "retrying one section restarts only that section")
            assertTrue(library.state.value.section(LibrarySection.History).loaded)
            assertNull(library.state.value.section(LibrarySection.History).failure)
            library.close()
        } finally { folder.deleteRecursively() }
    }

    @Test fun nullPresentationKeepsTheExactRecordAndNeverBorrowsAnotherReleasesTitle() = runTest {
        val folder = Files.createTempDirectory("cantos-presented-null").toFile()
        try {
            val api = Api().apply {
                continuePages[null] = LibraryResult.Value(PresentedContinuePage(listOf(
                    continueItem(1, 100), continueItem(2, 90, presented = false, resume = ResumeStatus.release_replaced)), null))
                savedPages[null] = LibraryResult.Value(PresentedSavedPage(listOf(savedItem(3, 100, presented = false)), null))
            }
            val library = controller(api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath))
            library.enterAs(actorA); runCurrent()
            val (available, replaced) = library.state.value.continueListening
            assertIs<LibraryPresentation.Available<*>>(available.presentation)
            assertEquals(LibraryPresentation.Unavailable, replaced.presentation)
            assertEquals(LibraryResume.ReleaseReplaced, replaced.resume)
            assertEquals(wireAudio(2).publication_id, replaced.audio.publicationId)
            assertEquals(15_000, replaced.positionMs, "the record and its progress are kept")
            assertEquals(LibraryPresentation.Unavailable, library.state.value.saved.single().presentation)
            assertEquals(true, library.state.value.saved.single().saved)

            // The listener starts a newer release of Work 1: local progress for the new publication
            // must not inherit the old release's title.
            val newer = requireNotNull(AudioIdentity.of(uuid(1, 1), uuid(2, 1), uuid(3, 99), uuid(4, 99), "b".repeat(64), 700_000))
            library.progressCheckpoint(newer, 5_000, force = true); runCurrent()
            val moved = library.state.value.continueListening.first { it.audio.workId == uuid(1, 1) }
            assertEquals(newer, moved.audio)
            assertEquals(LibraryPresentation.Unread, moved.presentation)
            assertEquals(LibraryResume.NeedsRefresh, moved.resume)
            assertEquals(5_000, moved.positionMs)
            library.close()
        } finally { folder.deleteRecursively() }
    }

    @Test fun restartKeepsTheSameRowsMetadataAndOlderRowsClaimNothing() = runTest {
        val folder = Files.createTempDirectory("cantos-presented-restart").toFile()
        try {
            val path = folder.resolve("db.sqlite").absolutePath
            val api = Api().apply {
                continuePages[null] = LibraryResult.Value(PresentedContinuePage(listOf(continueItem(1, 100), continueItem(2, 90, presented = false)), null))
                savedPages[null] = LibraryResult.Value(PresentedSavedPage(listOf(savedItem(3, 100)), null))
                historyPages[null] = LibraryResult.Value(PresentedHistoryPage(HistoryState(1, 0), listOf(historyItem(4, 100)), null))
            }
            val first = controller(api, desktopLibraryStore(path))
            first.enterAs(actorA); runCurrent()
            val before = first.state.value
            first.close()

            // Restart with the network down: the stored wire bodies project to the same metadata.
            val down = Api().apply {
                continuePages[null] = LibraryResult.Failed("unavailable", true)
                savedPages[null] = LibraryResult.Failed("unavailable", true)
                historyPages[null] = LibraryResult.Failed("unavailable", true)
                bookmarkPages[null] = LibraryResult.Failed("unavailable", true)
            }
            val store = desktopLibraryStore(path)
            val reopened = controller(down, store)
            reopened.enterAs(actorA); runCurrent()
            assertEquals(before.continueListening, reopened.state.value.continueListening)
            assertEquals(before.saved, reopened.state.value.saved)
            assertEquals(before.history, reopened.state.value.history)
            assertTrue(LibrarySection.entries.all { reopened.state.value.section(it).failure?.retryable == true && !reopened.state.value.section(it).loaded },
                "stale rows are shown beside an honest failed read")
            reopened.leave()

            // A row stored before presentation reads has no metadata record: Unread, never null/Unavailable.
            store.transaction(actorA) {
                val legacy = ContinueItem(progress(8, 50), ResumeStatus.exact_available)
                put(LocalLibraryRecord("continue", legacy.progress.audio.work_id, LibraryCodec.json.encodeToString(legacy), legacy.progress.revision))
                val legacyHistory = HistoryItem(wireAudio(9), 1, 40, null, ResumeStatus.exact_available)
                put(LocalLibraryRecord("history", legacyHistory.audio.work_id, LibraryCodec.json.encodeToString(legacyHistory), legacyHistory.listener_order))
            }
            reopened.enterAs(actorA); runCurrent()
            assertEquals(LibraryPresentation.Unread, reopened.state.value.continueListening.first { it.audio.workId == uuid(1, 8) }.presentation)
            assertEquals(LibraryPresentation.Unread, reopened.state.value.history.first { it.audio.workId == uuid(1, 9) }.presentation)
            assertIs<LibraryPresentation.Available<*>>(reopened.state.value.continueListening.first { it.audio.workId == uuid(1, 1) }.presentation, "rows the API did name keep their label")
            reopened.close()
        } finally { folder.deleteRecursively() }
    }

    @Test fun rowsBeyondTheFirstPageKeepTheirOwnLabelWhenOnlyTheFirstPageIsReadAgain() = runTest {
        val folder = Files.createTempDirectory("cantos-presented-walk").toFile()
        try {
            val c1 = "c".repeat(64)
            val api = Api().apply {
                continuePages[null] = LibraryResult.Value(PresentedContinuePage(listOf(continueItem(1, 100), continueItem(2, 90)), c1))
                continuePages[c1] = LibraryResult.Value(PresentedContinuePage(listOf(continueItem(3, 80)), null))
            }
            val library = controller(api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath))
            library.enterAs(actorA); runCurrent()
            library.nextPage(LibrarySection.Continue); runCurrent()
            library.refresh(LibrarySection.Continue); runCurrent()
            val rows = library.state.value.continueListening
            assertEquals(listOf(uuid(1, 1), uuid(1, 2), uuid(1, 3)), rows.map { it.audio.workId })
            assertEquals(listOf("Tập số 1", "Tập số 2", "Tập số 3"), rows.map { it.presentation.valueOrNull?.unitTitle },
                "the third row was read on an earlier page but still names its own release")
            library.close()
        } finally { folder.deleteRecursively() }
    }

    @Test fun aLateResponseOfAccountACannotReachAccountBOrItsMetadata() = runTest {
        val folder = Files.createTempDirectory("cantos-presented-ab").toFile()
        try {
            val gate = CompletableDeferred<Unit>()
            val api = Api().apply {
                savedPages[null] = LibraryResult.Value(PresentedSavedPage(listOf(savedItem(1, 100)), null))
                savedGate = gate
            }
            val store = desktopLibraryStore(folder.resolve("db.sqlite").absolutePath)
            val library = controller(api, store)
            library.enterAs(actorA); runCurrent()
            assertEquals(LibraryLoad.Refreshing, library.state.value.section(LibrarySection.Saved).loading)
            library.leave()
            assertNull(library.state.value.account)
            assertTrue(library.state.value.sections.isEmpty(), "logout clears every section state")
            api.savedPages[null] = LibraryResult.Value(PresentedSavedPage(listOf(savedItem(2, 100)), null))
            library.enterAs("actor-B", "00000000-0000-0000-0000-000000000002"); runCurrent()
            gate.complete(Unit); runCurrent()
            assertEquals("actor-B", library.state.value.account)
            assertEquals(listOf(uuid(1, 2)), library.state.value.saved.map { it.workId }, "only B's own read lands")
            assertEquals("Truyện số 2", library.state.value.saved.single().presentation.valueOrNull?.title)
            assertTrue(store.transaction(actorA) { records().none { it.resource.startsWith("saved") } }, "A's late page is not stored")
            assertNotEquals(null, store.transaction("actor-B") { record("saved_presentation", uuid(1, 2)) })
            library.close()
        } finally { folder.deleteRecursively() }
    }

    @Test fun metadataNoStoredRowNamesAnymoreIsDroppedWhenItsSectionIsReadAgain() = runTest {
        val folder = Files.createTempDirectory("cantos-presented-prune").toFile()
        try {
            val store = desktopLibraryStore(folder.resolve("db.sqlite").absolutePath)
            val api = Api().apply {
                historyPages[null] = LibraryResult.Value(PresentedHistoryPage(HistoryState(1, 0), listOf(historyItem(7, 100)), null))
            }
            val library = controller(api, store)
            library.enterAs(actorA); runCurrent()
            assertTrue(store.transaction(actorA) { record("unit_presentation", uuid(3, 7)) } != null)
            // The history is cleared elsewhere; nothing names Work 7's release any more.
            api.historyPages[null] = LibraryResult.Value(PresentedHistoryPage(HistoryState(2, 1), emptyList(), null))
            library.refresh(LibrarySection.History); runCurrent()
            assertNull(store.transaction(actorA) { record("unit_presentation", uuid(3, 7)) })
            library.close()
        } finally { folder.deleteRecursively() }
    }
}
