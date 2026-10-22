package com.cantos.theatre.app

import com.cantos.theatre.adapters.library.BookmarkServer
import com.cantos.theatre.adapters.library.desktopLibraryStore
import com.cantos.theatre.adapters.library.nativeLibraryController
import com.cantos.theatre.adapters.library.wire.AudioPresentation
import com.cantos.theatre.adapters.library.wire.ResumeStatus
import com.cantos.theatre.adapters.library.wire.WorkPresentation
import com.cantos.theatre.adapters.library.wire.AudioIdentity as WireAudio
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
import com.cantos.theatre.domain.library.BookmarkSync
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.domain.playback.PlaybackPauseCause
import com.cantos.theatre.domain.playback.PlaybackSource
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.ports.*
import com.cantos.theatre.presentation.model.ShellBookmark
import com.cantos.theatre.presentation.model.ShellBookmarkSync
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellWorkId
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest

/**
 * The signed-in shell's bookmark controls over the real account, durable SQLite Library, sync adapter and player
 * owner. Only the network (a stateful bookmark-authority double) and the audio decoder are fakes: this is
 * `mock-integration-tested`, not API, device or listening evidence.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class TheatreShellBookmarkTest {
    private class Decoder : StreamingMediaEngine {
        private val observations = MutableSharedFlow<PlaybackObservation>(extraBufferCapacity = 64)
        override val events = observations.asSharedFlow()
        var starts = 0
        val loads = mutableListOf<PlaybackEngineCommand>()
        val seeks = mutableListOf<Long>()
        fun emit(value: PlaybackObservation) { check(observations.tryEmit(value)) }
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) { loads += command }
        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
        override suspend fun start(command: PlaybackEngineCommand) { starts += 1 }
        override suspend fun pause(command: PlaybackEngineCommand) = Unit
        override suspend fun seek(command: PlaybackEngineCommand) { seeks += command.positionMs }
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() = Unit
    }

    private class Fixture(scope: CoroutineScope) {
        val actor = "actor-a"
        private val grant = v1Grant('a', 'c', actor, "00000000-0000-4000-8000-000000000001", 121, 601, 901, authenticatedAt = 1, now = 1)
        private var sequence = 10
        private fun uuid() = "90000000-0000-4000-8000-${sequence++.toString().padStart(12, '0')}"
        private val folder = Files.createTempDirectory("cantos-shell-bookmarks").toFile()
        val store = desktopLibraryStore(folder.resolve("library.sqlite").absolutePath)
        val work: PublicWork = PublicCatalogSamples.work(1, title = "Ánh đèn cuối sân khấu")
        val unit = PublicCatalogSamples.unit(1, durationMs = 125_000)
        val audio = requireNotNull(AudioIdentity.of(work.id.value, unit.id.value, unit.publicationId.value,
            "60000000-0000-4000-8000-000000000006", "b".repeat(64), unit.durationMs))
        val other = requireNotNull(AudioIdentity.of("10000000-0000-4000-8000-0000000000e1", "20000000-0000-4000-8000-0000000000e1",
            "30000000-0000-4000-8000-0000000000e1", "60000000-0000-4000-8000-0000000000e1", "c".repeat(64), 90_000))
        val wire = WireAudio(audio.workId, audio.unitId, audio.publicationId, audio.renditionId, audio.sha256, audio.durationMs)
        val decoder = Decoder()
        val server = BookmarkServer().apply {
            presentation = { a ->
                if (a.publication_id == audio.publicationId) AudioPresentation(
                    WorkPresentation(a.work_id, a.publication_id, "Ánh đèn cuối sân khấu", "story", "vi-VN"), a.unit_id, "Một tiếng chuông trong sương") else null
            }
        }
        val account = NativeAccountController(scope, StubIdentityV1Api(), object : NativeCredentialJournal {
                private var record: NativeCredentialRecord? = NativeCredentialRecord.Active(grant)
                override fun read(): NativeJournalRead = record?.let(NativeJournalRead::Found) ?: NativeJournalRead.Missing
                override fun write(record: NativeCredentialRecord): CredentialFailure? { this.record = record; return null }
                override fun clear(): CredentialFailure? { record = null; return null }
            }, object : LogoutBarrier {
                override fun read() = LogoutBarrierRead.Clear
                override fun mark(): CredentialFailure? = null
                override fun clear(): CredentialFailure? = null
            }, now = { 1 })
        val library = nativeLibraryController(scope, server, store, ::uuid) { 1_000 }
        val runtime = NativeSessionRuntime(scope, { decoder }, object : PlaybackAccessPort {
            override suspend fun manifest(publicationId: String) = PlaybackAccessResult.Accepted(if (publicationId == other.publicationId) other else audio)
            override suspend fun authorize(identity: AudioIdentity) =
                PlaybackAccessResult.Accepted(PlaybackGrant(PlaybackLocation("https://media.invalid/test-only"), 100_000))
        }, account, library, ::uuid, { 1_000 })
        val catalog = PublicCatalogController(scope, object : PublicCatalogPort {
            override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery) = PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(work)))
            override suspend fun work(id: PublicWorkId) = PublicCatalogRead.Ready(work)
            override suspend fun units(work: PublicWork, cursor: CatalogCursor?) = PublicCatalogRead.Ready(PublicCatalogSamples.unitsPage(work, listOf(unit)))
        }).also { it.dispatch(PublicCatalogIntent.Start) }
        val shell = TheatreShell(scope, catalog, runtime, PreferencesController())
        fun close() { runtime.close(); folder.deleteRecursively() }
    }

    private val unitId = { f: Fixture -> ShellEpisodeId(f.unit.id.value, ShellWorkId(f.work.id.value)) }

    /** Opens the episode and drives the fake decoder to a confirmed playing position of [positionMs]. */
    private fun kotlinx.coroutines.test.TestScope.play(f: Fixture, positionMs: Long) {
        f.shell.dispatch(ShellEvent.Navigate(ShellRoute.Work(ShellWorkId(f.work.id.value)))); runCurrent()
        f.shell.dispatch(ShellEvent.OpenEpisode(unitId(f))); runCurrent()
        val load = f.decoder.loads.last()
        f.decoder.emit(PlaybackEvent.EngineReady(load.identity))
        f.decoder.emit(PlaybackEvent.EngineStarted(load.identity))
        f.decoder.emit(PlaybackEvent.EnginePosition(load.identity, positionMs))
        runCurrent()
    }

    @Test
    fun the_bookmark_control_records_the_players_confirmed_release_and_position_for_the_account() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent(); advanceUntilIdle()
            play(f, 33_000)
            assertEquals(PlaybackStatus.Playing, f.runtime.listening.playback.state.value.status)
            f.shell.dispatch(ShellEvent.Bookmark); runCurrent(); advanceUntilIdle()
            val made = f.server.live(f.actor).single()
            assertEquals(f.wire, made.audio, "the exact release the player holds")
            assertEquals(33_000, made.position_ms, "the position the player confirmed")
            val row = f.shell.library.value.bookmarks.single()
            assertEquals(BookmarkSync.Synced, row.sync)
            val projected = projectOf(f)
            assertEquals(listOf(ShellBookmarkSync.Synced), projected.state.bookmarks.map { it.sync })
            assertEquals("Một tiếng chuông trong sương", projected.state.bookmarks.single().titles?.unitTitle)
            assertEquals(33, projected.state.bookmarks.single().positionSeconds)
            assertEquals(1, f.decoder.starts, "bookmarking neither plays nor seeks")
            assertTrue(f.decoder.seeks.isEmpty())
        } finally { f.close() }
    }

    @Test
    fun changing_episode_right_after_pressing_cannot_bind_the_new_release_to_the_old_position() = runTest {
        val f = Fixture(backgroundScope)
        try {
            runCurrent(); advanceUntilIdle()
            play(f, 33_000)
            // Both messages are queued on the player owner before it runs: the capture precedes the change.
            f.shell.dispatch(ShellEvent.Bookmark)
            f.runtime.listening.playback.dispatch(PlaybackEvent.SourceSelected(PlaybackSource.streaming(f.other, "Tập khác"), 0))
            runCurrent(); advanceUntilIdle()
            val made = f.server.live(f.actor).single()
            assertEquals(f.wire, made.audio)
            assertEquals(33_000, made.position_ms)
            assertEquals(f.other, f.runtime.listening.playback.state.value.session?.source?.audio, "the change itself still happened")
        } finally { f.close() }
    }

    @Test
    fun a_guest_is_sent_to_sign_in_and_no_bookmark_exists_anywhere() = runTest {
        // No identity boundary or Library owner here: a guest never sees a saved bookmark.
        val scope = backgroundScope
        val unit = PublicCatalogSamples.unit(1, durationMs = 125_000)
        val work = PublicCatalogSamples.work(1)
        val decoder = Decoder()
        val catalog = PublicCatalogController(scope, object : PublicCatalogPort {
            override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery) = PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(work)))
            override suspend fun work(id: PublicWorkId) = PublicCatalogRead.Ready(work)
            override suspend fun units(work: PublicWork, cursor: CatalogCursor?) = PublicCatalogRead.Ready(PublicCatalogSamples.unitsPage(work, listOf(unit)))
        })
        val audio = requireNotNull(AudioIdentity.of(work.id.value, unit.id.value, unit.publicationId.value, "60000000-0000-4000-8000-000000000006", "b".repeat(64), unit.durationMs))
        val runtime = NativeSessionRuntime(scope, { decoder }, object : PlaybackAccessPort {
            override suspend fun manifest(publicationId: String) = PlaybackAccessResult.Accepted(audio)
            override suspend fun authorize(identity: AudioIdentity) = PlaybackAccessResult.Accepted(PlaybackGrant(PlaybackLocation("https://media.invalid/x"), 1_000))
        }, account = null, library = null, newUuid = { "00000000-0000-4000-8000-000000000001" }, nowMs = { 1_000 })
        val shell = TheatreShell(scope, catalog, runtime, PreferencesController())
        runCurrent()
        shell.dispatch(ShellEvent.Bookmark); runCurrent()
        assertEquals(TheatreRoute.Account, shell.navigation.value.route, "the guest is guided to an account, not shown a fake save")
        assertTrue(shell.library.value.bookmarks.isEmpty())
        runtime.close()
    }

    @Test
    fun opening_a_bookmark_restores_its_exact_release_paused_and_listing_never_plays() = runTest {
        val f = Fixture(backgroundScope)
        try {
            f.server.create(f.actor, "b0000000-0000-4000-8000-000000000001", f.wire, 40_000)
            runCurrent(); advanceUntilIdle()
            assertEquals(1, f.shell.library.value.bookmarks.size)
            assertEquals(0, f.decoder.starts, "reading the list starts nothing")
            assertNull(f.runtime.listening.playback.state.value.session)
            val shown = projectOf(f).state.bookmarks.single()
            f.shell.dispatch(ShellEvent.OpenBookmark(shown)); runCurrent()
            val session = f.runtime.listening.playback.state.value.session
            assertEquals(f.audio, session?.source?.audio)
            assertEquals(40_000L, session?.positionMs)
            assertEquals(PlaybackIntent.Pause(PlaybackPauseCause.ColdRestore), session?.intent)
            assertEquals(TheatreRoute.Player, f.shell.navigation.value.route)
            assertEquals(0, f.decoder.starts, "opening restores; only the listener presses Play")
            assertEquals("Một tiếng chuông trong sương", f.runtime.listening.selection.value?.title)
        } finally { f.close() }
    }

    @Test
    fun opening_a_bookmark_of_the_loaded_release_moves_it_and_keeps_the_listeners_own_intent() = runTest {
        val f = Fixture(backgroundScope)
        try {
            f.server.create(f.actor, "b0000000-0000-4000-8000-000000000001", f.wire, 90_000)
            runCurrent(); advanceUntilIdle()
            play(f, 10_000)
            f.shell.dispatch(ShellEvent.OpenBookmark(projectOf(f).state.bookmarks.single())); runCurrent()
            val session = f.runtime.listening.playback.state.value.session
            assertEquals(PlaybackIntent.Play, session?.intent, "still playing: the listener did not pause it")
            assertEquals(f.audio, session?.source?.audio, "the same release, through the one player owner")
            assertEquals(90_000L, f.decoder.loads.last().positionMs, "the one engine is moved to the bookmark")
            assertEquals(f.audio, f.decoder.loads.last().source.audio)
        } finally { f.close() }
    }

    @Test
    fun a_replaced_or_unavailable_release_is_explained_and_never_substituted_or_played() = runTest {
        for (status in listOf(ResumeStatus.release_replaced, ResumeStatus.access_unavailable)) {
            val f = Fixture(backgroundScope)
            try {
                f.server.resume = status
                f.server.create(f.actor, "b0000000-0000-4000-8000-000000000001", f.wire, 40_000)
                runCurrent(); advanceUntilIdle()
                val shown = projectOf(f).state.bookmarks.single()
                assertEquals(f.wire.publication_id, f.shell.library.value.bookmarks.single().audio.publicationId, "still the exact stored release")
                f.shell.dispatch(ShellEvent.OpenBookmark(shown)); runCurrent()
                assertEquals(TheatreRoute.Unavailable, f.shell.navigation.value.route, status.name)
                assertNull(f.runtime.listening.playback.state.value.session, status.name)
                assertEquals(0, f.decoder.starts)
                assertTrue(f.decoder.loads.isEmpty())
            } finally { f.close() }
        }
    }

    @Test
    fun deleting_is_synced_and_a_deleted_bookmark_stays_gone_after_refresh() = runTest {
        val f = Fixture(backgroundScope)
        try {
            val id = "b0000000-0000-4000-8000-000000000001"
            f.server.create(f.actor, id, f.wire, 40_000)
            runCurrent(); advanceUntilIdle()
            f.shell.dispatch(ShellEvent.DeleteBookmark(id)); runCurrent(); advanceUntilIdle()
            assertTrue(f.shell.library.value.bookmarks.isEmpty())
            assertEquals(listOf(id), f.server.tombstones(f.actor).map { it.bookmark_id })
            f.shell.dispatch(ShellEvent.Refresh); f.shell.dispatch(ShellEvent.Navigate(ShellRoute.Library)); f.shell.dispatch(ShellEvent.Refresh)
            runCurrent(); advanceUntilIdle()
            assertTrue(f.shell.library.value.bookmarks.isEmpty())
            assertTrue(projectOf(f).state.bookmarks.isEmpty())
        } finally { f.close() }
    }

    @Test
    fun logout_hides_the_accounts_bookmarks_and_never_writes_one_for_nobody() = runTest {
        val f = Fixture(backgroundScope)
        try {
            f.server.create(f.actor, "b0000000-0000-4000-8000-000000000001", f.wire, 40_000)
            runCurrent(); advanceUntilIdle()
            play(f, 20_000)
            assertEquals(1, f.shell.library.value.bookmarks.size)
            f.account.logout()
            f.shell.dispatch(ShellEvent.Bookmark)
            runCurrent(); advanceUntilIdle()
            assertTrue(f.shell.library.value.bookmarks.isEmpty())
            assertNull(f.shell.library.value.account)
            assertEquals(1, f.server.account(f.actor).bookmarks.size, "nothing was added after logout")
            assertEquals(0, f.server.mutations)
        } finally { f.close() }
    }

    private fun projectOf(f: Fixture) = projectTheatreShell(TheatreShellInputs(f.shell.navigation.value, f.catalog.state.value, f.shell.known.value,
        f.runtime.listening.playback.state.value, f.runtime.listening.selection.value, f.shell.requested.value, false, f.shell.library.value,
        com.cantos.theatre.presentation.model.ShellTheme.Light, com.cantos.theatre.presentation.model.ShellLocale.Vietnamese,
        "", com.cantos.theatre.presentation.model.ShellLibraryTab.Continuing, com.cantos.theatre.presentation.model.ShellTimer.Off))
}
