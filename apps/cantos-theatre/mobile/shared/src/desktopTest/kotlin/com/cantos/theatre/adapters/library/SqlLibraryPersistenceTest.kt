package com.cantos.theatre.adapters.library

import com.cantos.theatre.application.library.NativeLibraryController
import com.cantos.theatre.adapters.library.wire.*
import com.cantos.theatre.adapters.library.wire.AudioIdentity as WireAudio
import com.cantos.theatre.domain.library.ProgressIntent
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.ports.*
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.test.*
import kotlinx.serialization.encodeToString
import java.nio.file.Files
import kotlin.test.*

@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class SqlLibraryPersistenceTest {
    private val actor = "actor-A"
    private val session = "00000000-0000-0000-0000-000000000001"
    private val work = "10000000-0000-0000-0000-000000000001"
    private val unit = "20000000-0000-0000-0000-000000000001"
    private val publication = "30000000-0000-0000-0000-000000000001"
    private val rendition = "40000000-0000-0000-0000-000000000001"
    private val audio = requireNotNull(AudioIdentity.of(work, unit, publication, rendition, "a".repeat(64), 100_000))
    private class Api : LibraryApi {
        val writes = mutableListOf<LocalLibraryIntent>()
        val receiptRequests = mutableListOf<String>()
        var receiptValue: Receipt? = null
        var continueResult = ContinuePage(emptyList(), null)
        var continueFailure: LibraryResult.Failed? = null
        var savedResult = SavedPage(emptyList(), null)
        var historyResult = HistoryPage(HistoryState(0, 0), emptyList(), null)
        var clearResponse: CompletableDeferred<LibraryResult<Receipt>>? = null
        var mutation: ((LocalLibraryIntent) -> LibraryResult<Receipt>)? = null
        var savedDeferred: CompletableDeferred<LibraryResult<SavedPage>>? = null
        var savedFailure: LibraryResult.Failed? = null
        var lateUnauthorizedOnCancel = false
        override suspend fun continuePage(auth: LibraryAuthorization, cursor: String?): LibraryResult<ContinuePage> = continueFailure ?: LibraryResult.Value(continueResult)
        override suspend fun savedPage(auth: LibraryAuthorization, cursor: String?): LibraryResult<SavedPage> = try {
            savedFailure ?: savedDeferred?.await() ?: LibraryResult.Value(savedResult)
        } catch (cancelled: kotlinx.coroutines.CancellationException) {
            if (lateUnauthorizedOnCancel) LibraryResult.Failed("unauthenticated", false) else throw cancelled
        }
        override suspend fun historyPage(auth: LibraryAuthorization, cursor: String?) = LibraryResult.Value(historyResult)
        override suspend fun saved(auth: LibraryAuthorization, work: String) = LibraryResult.Value(SavedRecord(work, false, 0, 0, 0))
        override suspend fun progress(auth: LibraryAuthorization, publication: String) = LibraryResult.Value(MutationState.Progress(null))
        override suspend fun mutate(auth: LibraryAuthorization, intent: LocalLibraryIntent): LibraryResult<Receipt> {
            writes += intent
            if (intent.path == "/library/history/clear") clearResponse?.let { return it.await() }
            return mutation?.invoke(intent) ?: LibraryResult.Failed("unavailable", true, true)
        }
        override suspend fun receipt(auth: LibraryAuthorization, operation: String): LibraryResult<Receipt> { receiptRequests += operation; return receiptValue?.let { LibraryResult.Value(it) } ?: LibraryResult.Failed("not_found", false) }
    }
    private var sequence = 10
    private fun id(): String = "90000000-0000-0000-0000-${sequence++.toString().padStart(12, '0')}"
    @Test fun sqliteRollbackHasNeitherCandidateNorOutboxAndAccountKeysStaySeparate() {
        val folder = Files.createTempDirectory("cantos-sql-contract").toFile()
        try {
            val file = folder.resolve("library.sqlite").absolutePath
            val first = desktopLibraryStore(file)
            assertFailsWith<IllegalStateException> {
                first.transaction(actor) {
                    put(LocalLibraryRecord("checkpoint", "latest", "candidate", 0))
                    enqueue(LocalLibraryIntent(id(), "progress:$publication", "PUT", "/progress/$publication", "exact-body"))
                    error("crash before commit")
                }
            }
            assertEquals(0, first.transaction(actor) { records().size + intents().size })
            first.transaction(actor) {
                put(LocalLibraryRecord("checkpoint", "latest", "rewind-7000", 0))
                enqueue(LocalLibraryIntent(id(), "progress:$publication", "PUT", "/progress/$publication", "exact-body"))
                val progressDevice = nextSequence("progress:$publication", ::id).first
                val savedDevice = nextSequence("saved:$work", ::id).first
                assertEquals(progressDevice, savedDevice, "one stable install device is shared by resource keys")
            }
            first.close()
            val reopened = desktopLibraryStore(file)
            assertEquals("rewind-7000", reopened.transaction(actor) { record("checkpoint", "latest")?.body })
            assertEquals("exact-body", reopened.transaction(actor) { intents().single().body })
            assertTrue(reopened.transaction("actor-B") { records().isEmpty() && intents().isEmpty() })
            val aOperation = reopened.transaction(actor) { intents().single().operation }
            reopened.transaction("actor-B") { enqueue(LocalLibraryIntent(aOperation, "saved:$work", "PUT", "/library/saved/$work", "B-body")) }
            assertEquals("exact-body", reopened.transaction(actor) { intents().single().body })
            assertEquals("B-body", reopened.transaction("actor-B") { intents().single().body })
            reopened.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun lostResponseAfterCommitReconcilesReceiptAfterSqliteReopenWithExactOriginalIntent() = runTest {
        val folder = Files.createTempDirectory("cantos-receipt-restart").toFile()
        try {
            val file = folder.resolve("library.sqlite").absolutePath
            val api = Api()
            val first = nativeLibraryController(backgroundScope, api, desktopLibraryStore(file), ::id) { 100_000 }
            first.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64))
            runCurrent()
            first.progressCheckpoint(audio, 7000, ProgressIntent.ExplicitSeek, true)
            first.savePlaybackSettings(audio, 1500)
            runCurrent()
            val original = api.writes.single()
            val write = assertNotNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", original.body))
            val current = ProgressRecord(write.audio, 7000, Completion.in_progress, 1, 100_001, 1, write.operation.device_id, ProgressKind.explicit_seek)
            api.receiptValue = Receipt(write.operation.operation_id, "accepted", null, MutationState.Progress(current))
            first.close()
            val second = nativeLibraryController(backgroundScope, api, desktopLibraryStore(file), ::id) { 100_010 }
            second.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64))
            runCurrent()
            assertEquals(listOf(original.operation), api.receiptRequests)
            assertEquals(1, api.writes.size, "receipt prevents duplicate network write")
            assertEquals(0, second.state.value.pending)
            assertEquals(7000, second.state.value.restoredPaused?.positionMs)
            assertEquals(audio, second.state.value.restoredPaused?.audio)
            assertEquals(1500, second.state.value.restoredPaused?.speedPermille)
            second.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun switchHidesOldAccountAndDelayedReadCannotOverwriteNewAccount() = runTest {
        val folder = Files.createTempDirectory("cantos-account-fence").toFile()
        try {
            val api = Api(); val delayed = CompletableDeferred<LibraryResult<SavedPage>>()
            api.savedDeferred = delayed
            val controller = nativeLibraryController(backgroundScope, api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath), ::id) { 1000 }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            controller.setSaved(work, true); runCurrent()
            assertEquals(true, controller.state.value.saved.single().saved)
            controller.leave()
            assertNull(controller.state.value.account); assertTrue(controller.state.value.saved.isEmpty())
            api.savedDeferred = null
            controller.enter(NativeSession("actor-B", session, Long.MAX_VALUE, null), "c".repeat(64)); runCurrent()
            delayed.complete(LibraryResult.Value(SavedPage(listOf(SavedItem(SavedRecord(work, true, 10, 10, 100))), null))); runCurrent()
            assertEquals("actor-B", controller.state.value.account)
            assertTrue(controller.state.value.saved.isEmpty())
            assertEquals(1, api.writes.size, "A's outbox was not sent as B")
            controller.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun unresolvedRetryKeepsOriginalOperationDeviceSequenceBaseAndExactBody() = runTest {
        val folder = Files.createTempDirectory("cantos-identical-retry").toFile()
        try {
            val api = Api()
            val controller = nativeLibraryController(backgroundScope, api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath), ::id) { 1000 }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            controller.setSaved(work, true); runCurrent()
            val original = api.writes.single()
            controller.refresh(); runCurrent()
            assertTrue(controller.state.value.saved.single().saved, "empty refresh preserves pending local desire")
            controller.drain(); runCurrent()
            assertEquals(listOf(original.operation), api.receiptRequests)
            assertEquals(2, api.writes.size)
            assertEquals(original.operation, api.writes.last().operation)
            assertEquals(original.body, api.writes.last().body)
            controller.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun staleDeviceSequenceIsFailedWithoutAutomaticRemintOrBodyChange() = runTest {
        val folder = Files.createTempDirectory("cantos-stale-device").toFile()
        try {
            val api = Api().apply { mutation = { LibraryResult.Failed("stale_device_sequence", false) } }
            val store = desktopLibraryStore(folder.resolve("db.sqlite").absolutePath)
            val controller = nativeLibraryController(backgroundScope, api, store, ::id) { 1000 }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            controller.setSaved(work, true); runCurrent()
            val original = api.writes.single()
            controller.drain(); runCurrent()
            assertEquals(1, api.writes.size)
            val retained = store.transaction(actor) { intents().single() }
            assertEquals(original.operation, retained.operation)
            assertEquals(original.body, retained.body)
            assertEquals("stale_device_sequence", retained.failure)
            assertEquals("stale_device_sequence", controller.state.value.failures[original.operation])
            controller.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun competingProgressPreservesLocalPausedCandidateUntilExplicitChoice() = runTest {
        val folder = Files.createTempDirectory("cantos-progress-conflict").toFile()
        try {
            val api = Api().apply {
                mutation = { intent ->
                    val write = requireNotNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", intent.body))
                    val current = ProgressRecord(write.audio, 90_000, Completion.in_progress, 4, 1000, 4, "80000000-0000-0000-0000-000000000001", ProgressKind.checkpoint)
                    LibraryResult.Value(Receipt(intent.operation, "revision_conflict", "listener_chooses", MutationState.Progress(current)))
                }
            }
            val controller = nativeLibraryController(backgroundScope, api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath), ::id) { 1000 }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            controller.progressCheckpoint(audio, 70_000, ProgressIntent.ExplicitSeek, true); runCurrent()
            val conflict = controller.state.value.conflicts.single()
            assertEquals(70_000, conflict.localPositionMs)
            assertEquals(90_000, conflict.serverPositionMs)
            assertEquals(70_000, controller.state.value.restoredPaused?.positionMs)
            assertEquals(1, api.writes.size, "conflict sends no replacement without a choice")
            api.mutation = null
            controller.resolveProgress(publication, true); runCurrent()
            val resolved = requireNotNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", api.writes.last().body))
            assertNotEquals(api.writes.first().operation, resolved.operation.operation_id)
            assertEquals(ProgressKind.resolve_here, resolved.kind)
            assertEquals(4, resolved.base_revision)
            assertEquals(70_000, resolved.position_ms)
            controller.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun checkpointCadenceIsBoundedWhileForcedLifecycleTransitionPersistsImmediately() = runTest {
        val folder = Files.createTempDirectory("cantos-checkpoint-cadence").toFile()
        try {
            var now = 1000L
            val api = Api()
            val store = desktopLibraryStore(folder.resolve("db.sqlite").absolutePath)
            val controller = nativeLibraryController(backgroundScope, api, store, ::id) { now }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            controller.progressCheckpoint(audio, 1000); runCurrent()
            repeat(29) { now += 1000; controller.progressCheckpoint(audio, now) }
            assertEquals(1, store.transaction(actor) { intents().size })
            controller.progressCheckpoint(audio, now, force = true)
            assertEquals(2, store.transaction(actor) { intents().size })
            assertEquals(now, controller.state.value.restoredPaused?.positionMs)
            controller.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun offlineProgressKeepsCapturedBaseAfterAnotherDeviceRefreshAndConflictsWithoutSeeking() = runTest {
        val folder = Files.createTempDirectory("cantos-offline-base").toFile()
        try {
            val store = desktopLibraryStore(folder.resolve("db.sqlite").absolutePath)
            val wire = WireAudio(work, unit, publication, rendition, "a".repeat(64), 100_000)
            val old = ProgressRecord(wire, 20_000, Completion.in_progress, 1, 1, 1, "80000000-0000-0000-0000-000000000001", ProgressKind.checkpoint)
            val remote = old.copy(position_ms = 90_000, revision = 2, listener_order = 2)
            store.transaction(actor) { put(LocalLibraryRecord("reconciled", "progress:$publication", LibraryCodec.json.encodeToString(old), 1)) }
            val api = Api().apply {
                continueResult = ContinuePage(listOf(ContinueItem(remote, ResumeStatus.exact_available)), null)
                mutation = { intent -> LibraryResult.Value(Receipt(intent.operation, "revision_conflict", "listener_chooses", MutationState.Progress(remote))) }
            }
            val controller = nativeLibraryController(backgroundScope, api, store, ::id) { 1000 }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64))
            controller.progressCheckpoint(audio, 70_000, ProgressIntent.ExplicitSeek, true)
            runCurrent()
            val sent = requireNotNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", api.writes.single().body))
            assertEquals(1, sent.base_revision)
            assertEquals(70_000, controller.state.value.restoredPaused?.positionMs)
            assertEquals(90_000, controller.state.value.conflicts.single().serverPositionMs)
            controller.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun explicitSavedChoiceRetiresOnlyKnownConflictAndKeepsOriginalIntentAndReceipt() = runTest {
        val folder = Files.createTempDirectory("cantos-saved-choice").toFile()
        try {
            val api = Api().apply { mutation = { intent -> LibraryResult.Value(Receipt(intent.operation, "revision_conflict", "listener_chooses", MutationState.Saved(SavedRecord(work, true, 3, 3, 1000)))) } }
            val store = desktopLibraryStore(folder.resolve("db.sqlite").absolutePath)
            val controller = nativeLibraryController(backgroundScope, api, store, ::id) { 1000 }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            controller.setSaved(work, false); runCurrent()
            val original = api.writes.single()
            assertEquals("listener_chooses", controller.state.value.failures[original.operation])
            api.mutation = { intent -> LibraryResult.Value(Receipt(intent.operation, "accepted", null, MutationState.Saved(SavedRecord(work, false, 4, 4, 1001)))) }
            controller.setSaved(work, false); runCurrent()
            assertEquals(2, api.writes.size)
            val selected = requireNotNull(LibraryCodec.decode<SavedWrite>("SavedWrite", api.writes.last().body))
            assertEquals(3, selected.base_revision)
            assertNotEquals(original.operation, selected.operation.operation_id)
            assertEquals(original.body, store.transaction(actor) { record("intent_archive", original.operation)?.body })
            assertNotNull(store.transaction(actor) { record("receipt_archive", original.operation) })
            assertTrue(store.transaction(actor) { intents().isEmpty() })
            controller.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun pendingSaveThenUnsaveWaitsForOriginalReceiptAndChainsOnlyOwnAcceptedRevision() = runTest {
        val folder = Files.createTempDirectory("cantos-saved-fifo").toFile()
        try {
            val api = Api()
            val controller = nativeLibraryController(backgroundScope, api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath), ::id) { 1000 }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            controller.setSaved(work, true); runCurrent()
            val original = api.writes.single()
            controller.setSaved(work, false); runCurrent()
            assertTrue(api.writes.all { it.operation == original.operation && it.body == original.body }, "unknown original is retried before later desire")
            assertFalse(controller.state.value.saved.single().saved)
            api.receiptValue = Receipt(original.operation, "accepted", null, MutationState.Saved(SavedRecord(work, true, 1, 1, 1000)))
            api.mutation = { intent ->
                val write = requireNotNull(LibraryCodec.decode<SavedWrite>("SavedWrite", intent.body))
                assertFalse(write.saved)
                assertEquals(1, write.base_revision, "only own accepted predecessor advances this pending base")
                LibraryResult.Value(Receipt(intent.operation, "accepted", null, MutationState.Saved(SavedRecord(work, false, 2, 2, 1001))))
            }
            controller.drain(); runCurrent()
            assertNotEquals(original.operation, api.writes.last().operation)
            assertEquals(0, controller.state.value.pending)
            controller.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun successfulLargeBacklogContinuesInBoundedBatchesWithoutDuplicateOrReorderedIntent() = runTest {
        val folder = Files.createTempDirectory("cantos-sync-batch").toFile()
        try {
            var revision = 0L
            val api = Api().apply {
                mutation = { intent ->
                    val write = requireNotNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", intent.body))
                    assertEquals(revision, write.base_revision)
                    revision += 1
                    LibraryResult.Value(Receipt(intent.operation, "accepted", null, MutationState.Progress(ProgressRecord(write.audio, write.position_ms, Completion.in_progress, revision, 1000, revision, write.operation.device_id, ProgressKind.checkpoint))))
                }
            }
            val controller = nativeLibraryController(backgroundScope, api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath), ::id) { 1000 }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64))
            repeat(25) { controller.progressCheckpoint(audio, it * 1000L, force = true) }
            runCurrent()
            assertEquals(20, api.writes.size)
            assertEquals(5, controller.state.value.pending)
            advanceTimeBy(251); runCurrent()
            assertEquals(25, api.writes.size)
            assertEquals(0, controller.state.value.pending)
            assertEquals(25, api.writes.map { it.operation }.toSet().size)
            assertEquals((0..24).map { it * 1000L }, api.writes.map { requireNotNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", it.body)).position_ms })
            controller.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun currentUnauthorizedHidesProjectionBeforeCallbackAndPreservesUnknownSentIntent() = runTest {
        val folder = Files.createTempDirectory("cantos-session-invalidated").toFile()
        try {
            val api = Api()
            val store = desktopLibraryStore(folder.resolve("db.sqlite").absolutePath)
            val invalidated = mutableListOf<String>()
            lateinit var controller: NativeLibraryController
            controller = nativeLibraryController(backgroundScope, api, store, ::id, onSessionInvalidated = { previous ->
                assertNull(controller.state.value.account)
                assertTrue(controller.state.value.saved.isEmpty())
                assertEquals(0, controller.state.value.pending)
                assertTrue(store.transaction(actor) { intents().single().sent })
                invalidated += previous
            }) { 1000 }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            controller.setSaved(work, true); runCurrent()
            val original = api.writes.single()
            api.savedFailure = LibraryResult.Failed("unauthenticated", false)
            controller.refresh(); runCurrent()
            assertEquals(listOf(session), invalidated)
            assertNull(controller.state.value.account)
            val persisted = store.transaction(actor) { intents().single() }
            assertEquals(original.operation, persisted.operation)
            assertEquals(original.body, persisted.body)
            assertNull(persisted.failure)
            api.savedFailure = null
            controller.enter(NativeSession(actor, "00000000-0000-0000-0000-000000000002", Long.MAX_VALUE, null), "c".repeat(64)); runCurrent()
            assertEquals(listOf(original.operation), api.receiptRequests)
            assertEquals(original.body, api.writes.last().body)
            controller.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun lateUnauthorizedFromCancelledOldAccountCannotInvalidateNewAccount() = runTest {
        val folder = Files.createTempDirectory("cantos-stale-auth-error").toFile()
        try {
            val api = Api().apply { savedDeferred = CompletableDeferred(); lateUnauthorizedOnCancel = true }
            val invalidated = mutableListOf<String>()
            val controller = nativeLibraryController(backgroundScope, api, desktopLibraryStore(folder.resolve("db.sqlite").absolutePath), ::id, onSessionInvalidated = { invalidated += it }) { 1000 }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            api.savedDeferred = null
            controller.enter(NativeSession("actor-B", "00000000-0000-0000-0000-000000000002", Long.MAX_VALUE, null), "c".repeat(64)); runCurrent()
            assertEquals("actor-B", controller.state.value.account)
            assertTrue(invalidated.isEmpty())
            controller.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun acknowledgedProgressRemainsInDurableContinueWhenRefreshFailsAndCompletionRemovesIt() = runTest {
        val folder = Files.createTempDirectory("cantos-continue-ssot").toFile()
        try {
            val path = folder.resolve("db.sqlite").absolutePath
            var revision = 0L
            val api = Api().apply {
                continueFailure = LibraryResult.Failed("unavailable", true)
                mutation = { intent ->
                    val write = requireNotNull(LibraryCodec.decode<ProgressWrite>("ProgressWrite", intent.body))
                    revision += 1
                    val completion = if (write.kind == ProgressKind.mark_completed) Completion.completed else Completion.in_progress
                    LibraryResult.Value(Receipt(intent.operation, "accepted", null, MutationState.Progress(ProgressRecord(write.audio, write.position_ms, completion, revision, 1000, revision, write.operation.device_id, write.kind))))
                }
            }
            val first = nativeLibraryController(backgroundScope, api, desktopLibraryStore(path), ::id) { 1000 }
            first.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            first.progressCheckpoint(audio, 42_000, force = true); runCurrent()
            val confirmed = first.state.value.continueListening.single()
            assertEquals(42_000, confirmed.positionMs)
            assertEquals(com.cantos.theatre.domain.library.LibrarySync.Confirmed, confirmed.sync)
            assertEquals(com.cantos.theatre.domain.library.LibraryResume.NeedsRefresh, confirmed.resume, "progress authority does not invent current playback eligibility")
            assertEquals("unavailable", first.state.value.failure)
            assertEquals(0, first.state.value.pending)
            first.close()
            val reopened = nativeLibraryController(backgroundScope, api, desktopLibraryStore(path), ::id) { 1001 }
            reopened.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            assertEquals(confirmed, reopened.state.value.continueListening.single())
            reopened.progressCheckpoint(audio, audio.durationMs, ProgressIntent.MarkCompleted, true); runCurrent()
            assertTrue(reopened.state.value.continueListening.isEmpty())
            assertEquals(0, reopened.state.value.pending)
            reopened.close()
        } finally { folder.deleteRecursively() }
    }
    @Test fun clearHistoryWaitsForReceiptAndRetainsConfirmedRegistersAndPendingSavedProgress() = runTest {
        val folder = Files.createTempDirectory("cantos-history-clear-isolation").toFile()
        try {
            val wire = WireAudio(work, unit, publication, rendition, "a".repeat(64), 100_000)
            val confirmedProgress = ProgressRecord(wire, 20_000, Completion.in_progress, 4, 1000, 4, "80000000-0000-0000-0000-000000000001", ProgressKind.checkpoint)
            val acknowledgedClear = CompletableDeferred<LibraryResult<Receipt>>()
            val api = Api().apply {
                savedResult = SavedPage(listOf(SavedItem(SavedRecord(work, true, 3, 3, 1000))), null)
                continueResult = ContinuePage(listOf(ContinueItem(confirmedProgress, ResumeStatus.exact_available)), null)
                historyResult = HistoryPage(HistoryState(1, 0), listOf(HistoryItem(wire, 1000, 1, confirmedProgress, ResumeStatus.exact_available)), null)
                clearResponse = acknowledgedClear
            }
            val store = desktopLibraryStore(folder.resolve("db.sqlite").absolutePath)
            val controller = nativeLibraryController(backgroundScope, api, store, ::id) { 1001 }
            controller.enter(NativeSession(actor, session, Long.MAX_VALUE, null), "b".repeat(64)); runCurrent()
            assertEquals(1, controller.state.value.history.size)
            assertTrue(controller.state.value.saved.single().saved)
            assertEquals(20_000, controller.state.value.continueListening.single().positionMs)
            controller.setSaved(work, false); runCurrent()
            controller.progressCheckpoint(audio, 30_000, ProgressIntent.ExplicitSeek, true); runCurrent()
            val pendingBefore = store.transaction(actor) { intents().associate { it.operation to it.body } }
            val retainedBefore = store.transaction(actor) {
                listOf(record("reconciled", "saved:$work"), record("reconciled", "progress:$publication"), record("checkpoint", "latest"))
            }
            assertEquals(2, pendingBefore.size)
            assertTrue(retainedBefore.all { it != null })
            controller.clearHistory(); runCurrent()
            val clearIntent = api.writes.single { it.path == "/library/history/clear" }
            val clear = requireNotNull(LibraryCodec.decode<HistoryClear>("HistoryClear", clearIntent.body))
            assertEquals(1, clear.base_revision)
            assertEquals(0, clear.history_epoch)
            assertEquals("clear_history", clear.confirmation)
            assertEquals(1, controller.state.value.history.size, "clear intent cannot erase history before acknowledgement")
            api.historyResult = HistoryPage(HistoryState(2, 1), emptyList(), null)
            // A later read must not repair an accidental clear of the other registers.
            api.continueFailure = LibraryResult.Failed("unavailable", true)
            api.savedFailure = LibraryResult.Failed("unavailable", true)
            acknowledgedClear.complete(LibraryResult.Value(Receipt(clearIntent.operation, "accepted", null, MutationState.History(HistoryState(2, 1)))))
            runCurrent()
            assertTrue(controller.state.value.history.isEmpty())
            assertFalse(controller.state.value.saved.single().saved)
            assertEquals(com.cantos.theatre.domain.library.LibrarySync.Pending, controller.state.value.saved.single().sync)
            assertEquals(30_000, controller.state.value.continueListening.single().positionMs)
            assertEquals(com.cantos.theatre.domain.library.LibrarySync.Pending, controller.state.value.continueListening.single().sync)
            assertEquals(pendingBefore, store.transaction(actor) { intents().associate { it.operation to it.body } })
            assertEquals(retainedBefore, store.transaction(actor) {
                listOf(record("reconciled", "saved:$work"), record("reconciled", "progress:$publication"), record("checkpoint", "latest"))
            })
            assertTrue(store.transaction(actor) { records().none { it.resource == "history" } })
            assertNotNull(store.transaction(actor) { record("receipt_archive", clearIntent.operation) })
            controller.close()
        } finally { folder.deleteRecursively() }
    }
}
