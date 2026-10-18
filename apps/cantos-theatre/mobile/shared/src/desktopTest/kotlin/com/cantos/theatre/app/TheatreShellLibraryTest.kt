package com.cantos.theatre.app

import com.cantos.theatre.adapters.library.LibraryApi
import com.cantos.theatre.adapters.library.LibraryAuthorization
import com.cantos.theatre.adapters.library.LibraryResult
import com.cantos.theatre.adapters.library.desktopLibraryStore
import com.cantos.theatre.adapters.library.nativeLibraryController
import com.cantos.theatre.adapters.library.wire.Completion
import com.cantos.theatre.adapters.library.wire.ContinueItem
import com.cantos.theatre.adapters.library.wire.ContinuePage
import com.cantos.theatre.adapters.library.wire.HistoryPage
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
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.domain.playback.PlaybackPauseCause
import com.cantos.theatre.ports.*
import com.cantos.theatre.presentation.model.ShellEpisodeId
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
        val actor = "actor-A"
        private val bearer = "a".repeat(64)
        private var sequence = 10
        private fun uuid() = "90000000-0000-4000-8000-${sequence++.toString().padStart(12, '0')}"
        private val folder = Files.createTempDirectory("cantos-shell-library").toFile()
        val store = desktopLibraryStore(folder.resolve("library.sqlite").absolutePath)
        val work: PublicWork = PublicCatalogSamples.work(1, title = "Ánh đèn cuối sân khấu")
        val unit = PublicCatalogSamples.unit(1, durationMs = 125_000)
        val audio = requireNotNull(AudioIdentity.of(work.id.value, unit.id.value, unit.publicationId.value,
            "60000000-0000-4000-8000-000000000006", "b".repeat(64), unit.durationMs))
        private val wireAudio = com.cantos.theatre.adapters.library.wire.AudioIdentity(
            audio.workId, audio.unitId, audio.publicationId, audio.renditionId, audio.sha256, audio.durationMs)
        val decoder = Decoder()

        private inner class Api : LibraryApi {
            override suspend fun continuePage(auth: LibraryAuthorization, cursor: String?) = LibraryResult.Value(ContinuePage(
                listOf(ContinueItem(ProgressRecord(wireAudio, 40_000, Completion.in_progress, 3, 5, 5, "70000000-0000-4000-8000-000000000001", ProgressKind.checkpoint), resume)), null))
            override suspend fun savedPage(auth: LibraryAuthorization, cursor: String?) = LibraryResult.Value(SavedPage(
                listOf(SavedItem(SavedRecord(work.id.value, true, 2, 4, 4))), null))
            override suspend fun historyPage(auth: LibraryAuthorization, cursor: String?) = LibraryResult.Value(HistoryPage(HistoryState(0, 0), emptyList(), null))
            override suspend fun saved(auth: LibraryAuthorization, work: String) = LibraryResult.Value(SavedRecord(work, false, 0, 0, 0))
            override suspend fun progress(auth: LibraryAuthorization, publication: String) = LibraryResult.Value(MutationState.Progress(null))
            override suspend fun mutate(auth: LibraryAuthorization, intent: LocalLibraryIntent): LibraryResult<Receipt> = LibraryResult.Failed("unavailable", true, true)
            override suspend fun receipt(auth: LibraryAuthorization, operation: String): LibraryResult<Receipt> = LibraryResult.Failed("not_found", false)
        }

        private val account = NativeAccountController(scope,
            requireNotNull(NativeClientConfiguration.create("https://api.invalid", "https://identity.invalid/oauth2/openid/cantos-theatre-native")),
            object : NativeIdentityApi {
                override suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow> = NativeApiResult.Refused(NativeApiFailure.Unavailable)
                override suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeSession> = NativeApiResult.Refused(NativeApiFailure.Unavailable)
                override suspend fun session(credential: String): NativeApiResult<NativeSession> =
                    NativeApiResult.Accepted(NativeSession(actor, "00000000-0000-4000-8000-000000000001", Long.MAX_VALUE, null))
                override suspend fun revoke(credential: String) = NativeApiResult.Accepted(Unit)
                override suspend fun saved(credential: String, session: String): NativeApiResult<Int> = error("unused")
            }, object : IdentityCrypto {
                override fun secret(): LoginSecret? = null
                override fun challenge(verifier: LoginSecret): String? = null
            }, object : SystemBrowser {
                override fun open(url: String) = false
                override fun cancel() = Unit
            }, object : CredentialStore {
                override fun read(): CredentialRead = CredentialRead.Found(bearer)
                override fun write(value: String): CredentialFailure? = null
                override fun clear(): CredentialFailure? = null
            }, object : LogoutBarrier {
                override fun read() = LogoutBarrierRead.Clear
                override fun mark(): CredentialFailure? = null
                override fun clear(): CredentialFailure? = null
            }, now = { 1 })
        val library = nativeLibraryController(scope, Api(), store, ::uuid) { 1_000 }
        val runtime = NativeSessionRuntime(scope, { decoder }, object : PlaybackAccessPort {
            override suspend fun manifest(publicationId: String) = PlaybackAccessResult.Accepted(audio)
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
}
