package com.cantos.theatre.app

import com.cantos.theatre.adapters.library.LibraryApi
import com.cantos.theatre.adapters.library.LibraryAuthorization
import com.cantos.theatre.adapters.library.LibraryResult
import com.cantos.theatre.adapters.library.desktopLibraryStore
import com.cantos.theatre.adapters.library.nativeLibraryController
import com.cantos.theatre.adapters.library.presented
import com.cantos.theatre.adapters.library.wire.AudioPresentation
import com.cantos.theatre.adapters.library.wire.Completion
import com.cantos.theatre.adapters.library.wire.ContinueItem
import com.cantos.theatre.adapters.library.wire.ContinuePage
import com.cantos.theatre.adapters.library.wire.HistoryItem
import com.cantos.theatre.adapters.library.wire.HistoryPage
import com.cantos.theatre.adapters.library.wire.PresentedContinuePage
import com.cantos.theatre.adapters.library.wire.PresentedHistoryPage
import com.cantos.theatre.adapters.library.wire.PresentedSavedPage
import com.cantos.theatre.adapters.library.wire.WorkPresentation
import com.cantos.theatre.adapters.library.wire.HistoryState
import com.cantos.theatre.adapters.library.wire.MutationState
import com.cantos.theatre.adapters.library.wire.ProgressKind
import com.cantos.theatre.adapters.library.wire.ProgressRecord
import com.cantos.theatre.adapters.library.wire.Receipt
import com.cantos.theatre.adapters.library.wire.ResumeStatus
import com.cantos.theatre.adapters.library.wire.SavedItem
import com.cantos.theatre.adapters.library.wire.SavedPage
import com.cantos.theatre.adapters.library.wire.SavedRecord
import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.StubIdentityV1Api
import com.cantos.theatre.application.identity.v1Grant
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.domain.playback.PlaybackPauseCause
import com.cantos.theatre.ports.*
import com.cantos.theatre.application.playback.NativeSelection
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellLibraryPhase
import com.cantos.theatre.presentation.model.ShellLibrarySection
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellWorkId
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest

/** The signed-in shell over the real account, SQLite Library and player owners; network and decoder are fakes. */
@OptIn(ExperimentalCoroutinesApi::class)
class TheatreShellLibraryTest {
    private class Decoder : StreamingMediaEngine {
        private val observations = MutableSharedFlow<PlaybackObservation>(extraBufferCapacity = 64)
        override val events = observations.asSharedFlow()
        var starts = 0
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = Unit
        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
        override suspend fun start(command: PlaybackEngineCommand) { starts += 1 }
        override suspend fun pause(command: PlaybackEngineCommand) = Unit
        override suspend fun seek(command: PlaybackEngineCommand) = Unit
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() = Unit
    }

    private class Fixture(scope: CoroutineScope, private val resume: ResumeStatus) {
        val actor = "actor-a"
        private val grant = v1Grant('a', 'c', actor, "00000000-0000-4000-8000-000000000001", 121, 601, 901, authenticatedAt = 1, now = 1)
        private var sequence = 10
        private fun uuid() = "90000000-0000-4000-8000-${sequence++.toString().padStart(12, '0')}"
        private val folder = Files.createTempDirectory("cantos-shell-library").toFile()
        val store = desktopLibraryStore(folder.resolve("library.sqlite").absolutePath)
        val work: PublicWork = PublicCatalogSamples.work(1, title = "Ánh đèn cuối sân khấu")
        val unit = PublicCatalogSamples.unit(1, durationMs = 125_000)
        val audio = requireNotNull(AudioIdentity.of(work.id.value, unit.id.value, unit.publicationId!!.value,
            "60000000-0000-4000-8000-000000000006", "b".repeat(64), unit.durationMs!!))
        private val wireAudio = com.cantos.theatre.adapters.library.wire.AudioIdentity(
            audio.workId, audio.unitId, audio.publicationId, audio.renditionId, audio.sha256, audio.durationMs)
        val decoder = Decoder()

        /** A second release the catalog never listed, for Library rows outside the loaded catalog. */
        val outsideAudio = requireNotNull(AudioIdentity.of("10000000-0000-4000-8000-0000000000e1", "20000000-0000-4000-8000-0000000000e1",
            "30000000-0000-4000-8000-0000000000e1", "60000000-0000-4000-8000-0000000000e1", "c".repeat(64), 90_000))
        val outsidePresentation = AudioPresentation(WorkPresentation(outsideAudio.workId, outsideAudio.publicationId, "Chuyện ngoài danh mục", "story", "vi-VN"),
            outsideAudio.unitId, "Tập chưa từng được tải")
        val calls = mutableListOf<String>()
        var continueAnswer: (String?) -> LibraryResult<PresentedContinuePage> = { LibraryResult.Value(ContinuePage(
            listOf(ContinueItem(ProgressRecord(wireAudio, 40_000, Completion.in_progress, 3, 5, 5, "70000000-0000-4000-8000-000000000001", ProgressKind.checkpoint), resume)), null).presented()) }
        var savedAnswer: (String?) -> LibraryResult<PresentedSavedPage> = { LibraryResult.Value(SavedPage(
            listOf(SavedItem(SavedRecord(work.id.value, true, 2, 4, 4))), null).presented()) }
        var historyAnswer: (String?) -> LibraryResult<PresentedHistoryPage> = { LibraryResult.Value(HistoryPage(HistoryState(0, 0), emptyList(), null).presented()) }

        private inner class Api : LibraryApi {
            override suspend fun continuePage(auth: LibraryAuthorization, cursor: String?): LibraryResult<PresentedContinuePage> { calls += "continue:$cursor"; return continueAnswer(cursor) }
            override suspend fun savedPage(auth: LibraryAuthorization, cursor: String?): LibraryResult<PresentedSavedPage> { calls += "saved:$cursor"; return savedAnswer(cursor) }
            override suspend fun historyPage(auth: LibraryAuthorization, cursor: String?): LibraryResult<PresentedHistoryPage> { calls += "history:$cursor"; return historyAnswer(cursor) }
            override suspend fun bookmarkPage(auth: LibraryAuthorization, cursor: String?) = LibraryResult.Value(com.cantos.theatre.adapters.library.wire.BookmarkPage(emptyList(), null))
            override suspend fun saved(auth: LibraryAuthorization, work: String) = LibraryResult.Value(SavedRecord(work, false, 0, 0, 0))
            override suspend fun progress(auth: LibraryAuthorization, publication: String) = LibraryResult.Value(MutationState.Progress(null))
            override suspend fun mutate(auth: LibraryAuthorization, intent: LocalLibraryIntent): LibraryResult<Receipt> = LibraryResult.Failed("unavailable", true, true)
            override suspend fun receipt(auth: LibraryAuthorization, operation: String): LibraryResult<Receipt> = LibraryResult.Failed("not_found", false)
        }

        private val account = NativeAccountController(scope, StubIdentityV1Api(), object : NativeCredentialJournal {
                private var record: NativeCredentialRecord? = NativeCredentialRecord.Active(grant)
                override fun read(): NativeJournalRead = record?.let(NativeJournalRead::Found) ?: NativeJournalRead.Missing
                override fun write(record: NativeCredentialRecord): CredentialFailure? { this.record = record; return null }
                override fun clear(): CredentialFailure? { record = null; return null }
            }, object : LogoutBarrier {
                override fun read() = LogoutBarrierRead.Clear
                override fun mark(): CredentialFailure? = null
                override fun clear(): CredentialFailure? = null
            }, now = { 1 })
        val library = nativeLibraryController(scope, Api(), store, ::uuid) { 1_000 }
        val runtime = NativeSessionRuntime(scope, { decoder }, object : PlaybackAccessPort {
            override suspend fun manifest(publicationId: String) = PlaybackAccessResult.Accepted(if (publicationId == outsideAudio.publicationId) outsideAudio else audio)
            override suspend fun authorize(identity: AudioIdentity) =
                PlaybackAccessResult.Accepted(PlaybackGrant(PlaybackLocation("https://media.invalid/test-only"), 100_000))
        }, account, library, ::uuid, { 1_000 })
        val catalog = PublicCatalogController(scope, object : PublicCatalogPort {
            override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery) =
                PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(work)))
            override suspend fun work(id: PublicWorkId) = PublicCatalogRead.Ready(work)
            override suspend fun units(work: PublicWork, cursor: CatalogCursor?) =
                PublicCatalogRead.Ready(PublicCatalogSamples.unitsPage(work, listOf(unit)))
        }).also { it.dispatch(PublicCatalogIntent.Start) }
        val shell = TheatreShell(scope, catalog, runtime, PreferencesController())
        fun intents() = store.transaction(actor) { intents() }
        fun close() { runtime.close(); folder.deleteRecursively() }
    }

    private val unitId = { f: Fixture -> ShellEpisodeId(f.unit.id.value, ShellWorkId(f.work.id.value)) }

    @Test
    fun an_exact_continue_entry_reopens_paused_at_its_saved_position_without_starting_the_engine() = runTest {
        val f = Fixture(backgroundScope, ResumeStatus.exact_available)
        try {
            runCurrent(); advanceUntilIdle()
            assertEquals(f.actor, f.shell.library.value.account)
            val projected = projectTheatreShell(TheatreShellInputs(NavigationState(), f.catalog.state.value, f.shell.known.value,
                f.runtime.listening.playback.state.value, null, null, false, f.shell.library.value,
                com.cantos.theatre.presentation.model.ShellTheme.Light, com.cantos.theatre.presentation.model.ShellLocale.Vietnamese,
                "", com.cantos.theatre.presentation.model.ShellLibraryTab.Continuing, com.cantos.theatre.presentation.model.ShellTimer.Off))
            assertEquals(40, projected.state.progress.single().positionSeconds)
            assertEquals(setOf(ShellWorkId(f.work.id.value)), projected.state.savedWorks)

            f.shell.dispatch(ShellEvent.ResumeEpisode(unitId(f)))
            runCurrent()
            val session = f.runtime.listening.playback.state.value.session
            assertEquals(40_000L, session?.positionMs)
            assertEquals(PlaybackIntent.Pause(PlaybackPauseCause.ColdRestore), session?.intent)
            assertEquals(TheatreRoute.Player, f.shell.navigation.value.route)
            assertEquals(0, f.decoder.starts, "restoring never plays")
        } finally { f.close() }
    }

    @Test
    fun a_stale_or_unavailable_release_never_opens_the_player() = runTest {
        for (status in listOf(ResumeStatus.release_replaced, ResumeStatus.access_unavailable)) {
            val f = Fixture(backgroundScope, status)
            try {
                runCurrent(); advanceUntilIdle()
                f.shell.dispatch(ShellEvent.ResumeEpisode(unitId(f)))
                runCurrent()
                assertEquals(TheatreRoute.Unavailable, f.shell.navigation.value.route, status.name)
                assertEquals(null, f.runtime.listening.playback.state.value.session, status.name)
            } finally { f.close() }
        }
    }

    @Test
    fun saving_a_work_writes_to_the_durable_local_library_for_the_signed_in_account() = runTest {
        val f = Fixture(backgroundScope, ResumeStatus.exact_available)
        try {
            runCurrent(); advanceUntilIdle()
            f.shell.dispatch(ShellEvent.Navigate(ShellRoute.Library))
            f.shell.dispatch(ShellEvent.SetSaved(ShellWorkId(f.work.id.value), false))
            runCurrent()
            assertTrue(f.intents().any { it.path == "/library/saved/${f.work.id.value}" && it.body.contains("false") },
                "the change is a durable intent before any network answer")
            assertEquals(TheatreRoute.Library, f.shell.navigation.value.route, "a signed-in save does not leave the screen")
        } finally { f.close() }
    }

    private fun project(f: Fixture) = projectTheatreShell(TheatreShellInputs(f.shell.navigation.value, f.catalog.state.value, f.shell.known.value,
        f.runtime.listening.playback.state.value, f.runtime.listening.selection.value, f.shell.requested.value, false, f.shell.library.value,
        com.cantos.theatre.presentation.model.ShellTheme.Light, com.cantos.theatre.presentation.model.ShellLocale.Vietnamese,
        "", com.cantos.theatre.presentation.model.ShellLibraryTab.Continuing, com.cantos.theatre.presentation.model.ShellTimer.Off))

    @Test
    fun a_section_footer_event_reads_only_its_own_section_and_retry_repeats_the_failed_page() = runTest {
        val f = Fixture(backgroundScope, ResumeStatus.exact_available)
        try {
            val next = "d".repeat(64)
            var attempts = 0
            f.savedAnswer = { cursor ->
                when {
                    cursor == null -> LibraryResult.Value(SavedPage(listOf(SavedItem(SavedRecord(f.work.id.value, true, 2, 9, 9))), next).presented())
                    ++attempts == 1 -> LibraryResult.Failed("unavailable", true)
                    else -> LibraryResult.Value(SavedPage(listOf(SavedItem(SavedRecord("10000000-0000-4000-8000-0000000000e1", true, 1, 5, 5))), null).presented())
                }
            }
            f.historyAnswer = { LibraryResult.Failed("unavailable", true) }
            runCurrent(); advanceUntilIdle()
            assertEquals(ShellLibraryPhase.More, project(f).catalog.library.saved)
            assertEquals(ShellLibraryPhase.Failed, project(f).catalog.library.history, "an unread, failed section is an error, not empty")
            f.calls.clear()

            f.shell.dispatch(ShellEvent.LoadMoreLibrary(ShellLibrarySection.Saved)); runCurrent()
            assertEquals(listOf("saved:$next"), f.calls, "the Saved footer reads the Saved cursor and nothing else")
            assertEquals(ShellLibraryPhase.MoreFailed, project(f).catalog.library.saved)
            assertEquals(1, f.shell.library.value.saved.size, "the failed page keeps the rows already shown")

            f.calls.clear()
            f.shell.dispatch(ShellEvent.RetryLibrary(ShellLibrarySection.Saved)); runCurrent()
            assertEquals(listOf("saved:$next"), f.calls, "retry of a failed next page repeats that page, not the first page")
            assertEquals(ShellLibraryPhase.Complete, project(f).catalog.library.saved)
            assertEquals(2, f.shell.library.value.saved.size)

            f.calls.clear()
            f.historyAnswer = { LibraryResult.Value(HistoryPage(HistoryState(1, 0), emptyList(), null).presented()) }
            f.shell.dispatch(ShellEvent.RetryLibrary(ShellLibrarySection.History)); runCurrent()
            assertEquals(listOf("history:null"), f.calls, "retrying a failed first page restarts only that section")
            assertEquals(ShellLibraryPhase.Complete, project(f).catalog.library.history)
        } finally { f.close() }
    }

    @Test
    fun resuming_a_row_the_api_named_restores_its_exact_release_with_its_titles_and_does_not_start_audio() = runTest {
        val f = Fixture(backgroundScope, ResumeStatus.exact_available)
        try {
            f.continueAnswer = {
                LibraryResult.Value(ContinuePage(listOf(ContinueItem(ProgressRecord(f.outsideAudio.let { a ->
                    com.cantos.theatre.adapters.library.wire.AudioIdentity(a.workId, a.unitId, a.publicationId, a.renditionId, a.sha256, a.durationMs) },
                    40_000, Completion.in_progress, 3, 5, 5, "70000000-0000-4000-8000-000000000001", ProgressKind.checkpoint), ResumeStatus.exact_available)), null)
                    .presented { f.outsidePresentation })
            }
            runCurrent(); advanceUntilIdle()
            val row = project(f).state.progress.single()
            assertEquals("Tập chưa từng được tải", row.titles?.unitTitle)
            assertEquals("Chuyện ngoài danh mục", row.titles?.workTitle)

            f.shell.dispatch(ShellEvent.ResumeEpisode(ShellEpisodeId(f.outsideAudio.unitId, ShellWorkId(f.outsideAudio.workId)))); runCurrent()
            val session = f.runtime.listening.playback.state.value.session
            assertEquals(f.outsideAudio, session?.source?.audio, "exact publication, rendition and hash")
            assertEquals(40_000L, session?.positionMs)
            assertEquals(PlaybackIntent.Pause(PlaybackPauseCause.ColdRestore), session?.intent)
            assertEquals(NativeSelection("Tập chưa từng được tải", "Chuyện ngoài danh mục"), f.runtime.listening.selection.value)
            assertEquals(TheatreRoute.Player, f.shell.navigation.value.route)
            assertEquals(0, f.decoder.starts)
        } finally { f.close() }
    }

    @Test
    fun history_reopens_the_exact_release_of_a_work_the_catalog_never_listed_and_a_replaced_one_stays_closed() = runTest {
        val f = Fixture(backgroundScope, ResumeStatus.exact_available)
        try {
            val outside = f.outsideAudio
            val wire = com.cantos.theatre.adapters.library.wire.AudioIdentity(outside.workId, outside.unitId, outside.publicationId, outside.renditionId, outside.sha256, outside.durationMs)
            f.historyAnswer = { LibraryResult.Value(HistoryPage(HistoryState(1, 0), listOf(HistoryItem(wire, 1_000, 3, null, ResumeStatus.exact_available)), null)
                .presented { f.outsidePresentation }) }
            runCurrent(); advanceUntilIdle()
            assertEquals(null, f.shell.known.value.unit(outside.unitId), "the catalog never listed it")
            f.shell.dispatch(ShellEvent.ResumeHistoryEpisode(ShellEpisodeId(outside.unitId, ShellWorkId(outside.workId)))); runCurrent()
            assertEquals(TheatreRoute.Player, f.shell.navigation.value.route)
            assertEquals(outside, f.runtime.listening.playback.state.value.session?.source?.audio)
            assertEquals(NativeSelection("Tập chưa từng được tải", "Chuyện ngoài danh mục"), f.runtime.listening.selection.value)

            val replaced = Fixture(backgroundScope, ResumeStatus.exact_available)
            try {
                replaced.historyAnswer = { LibraryResult.Value(HistoryPage(HistoryState(1, 0), listOf(HistoryItem(wire, 1_000, 3, null, ResumeStatus.release_replaced)), null).presented()) }
                runCurrent(); advanceUntilIdle()
                replaced.shell.dispatch(ShellEvent.ResumeHistoryEpisode(ShellEpisodeId(outside.unitId, ShellWorkId(outside.workId)))); runCurrent()
                assertEquals(TheatreRoute.Unavailable, replaced.shell.navigation.value.route)
                assertEquals(null, replaced.runtime.listening.playback.state.value.session)
            } finally { replaced.close() }
        } finally { f.close() }
    }

    @Test
    fun history_never_resumes_a_continue_entry_of_another_release_of_the_same_unit() = runTest {
        val f = Fixture(backgroundScope, ResumeStatus.exact_available)
        try {
            val older = com.cantos.theatre.adapters.library.wire.AudioIdentity(f.audio.workId, f.audio.unitId,
                "30000000-0000-4000-8000-0000000000f1", "60000000-0000-4000-8000-0000000000f1", "d".repeat(64), 80_000)
            val info = AudioPresentation(WorkPresentation(f.audio.workId, older.publication_id, "Bản cũ", "story", "vi-VN"), f.audio.unitId, "Tập bản cũ")
            f.historyAnswer = { LibraryResult.Value(HistoryPage(HistoryState(1, 0), listOf(HistoryItem(older, 1_000, 3, null, ResumeStatus.exact_available)), null).presented { info }) }
            runCurrent(); advanceUntilIdle()
            assertEquals(f.audio.publicationId, f.shell.library.value.continueListening.single().audio.publicationId, "Continue holds the newer release")
            f.shell.dispatch(ShellEvent.ResumeHistoryEpisode(unitId(f))); runCurrent()
            assertTrue(f.runtime.listening.playback.state.value.session?.intent != PlaybackIntent.Pause(PlaybackPauseCause.ColdRestore),
                "the other release's saved position is not restored under this row")
        } finally { f.close() }
    }
}
