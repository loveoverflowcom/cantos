package com.cantos.theatre.app

import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.membership.MembershipScreen
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.ports.StreamingMediaEngine
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.ShellWorkId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest

/** Real catalog, runtime and shell owners; the network and decoder are explicit fakes. */
@OptIn(ExperimentalCoroutinesApi::class)
class TheatreShellTest {
    private class Decoder : StreamingMediaEngine {
        private val observations = MutableSharedFlow<PlaybackObservation>(extraBufferCapacity = 64)
        override val events = observations.asSharedFlow()
        val loads = mutableListOf<PlaybackEngineCommand>()
        fun emit(value: PlaybackObservation) { check(observations.tryEmit(value)) }
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) { loads += command }
        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
        override suspend fun start(command: PlaybackEngineCommand) = Unit
        override suspend fun pause(command: PlaybackEngineCommand) { emit(PlaybackEvent.EnginePaused(command.identity)) }
        override suspend fun seek(command: PlaybackEngineCommand) = Unit
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() = Unit
    }

    private class Fixture(scope: CoroutineScope) {
        val work: PublicWork = PublicCatalogSamples.work(1, title = "Ánh đèn cuối sân khấu")
        val unit: PublicUnit = PublicCatalogSamples.unit(1, durationMs = 125_000)
        val audio = requireNotNull(AudioIdentity.of(work.id.value, unit.id.value, unit.publicationId.value,
            "60000000-0000-4000-8000-000000000006", "b".repeat(64), unit.durationMs))
        val decoder = Decoder()
        val queries = mutableListOf<PublicCatalogQuery>()
        val catalog = PublicCatalogController(scope, object : PublicCatalogPort {
            override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery): PublicCatalogRead<com.cantos.theatre.domain.catalog.PublicWorkPage> {
                queries += query
                return PublicCatalogRead.Ready(PublicCatalogSamples.worksPage(listOf(work)))
            }
            override suspend fun work(id: PublicWorkId) = PublicCatalogRead.Ready(work)
            override suspend fun units(work: PublicWork, cursor: CatalogCursor?) =
                PublicCatalogRead.Ready(PublicCatalogSamples.unitsPage(work, listOf(unit)))
        }).also { it.dispatch(PublicCatalogIntent.Start) }
        val runtime = NativeSessionRuntime(scope, nativeFactory = { decoder },
            access = object : PlaybackAccessPort {
                override suspend fun manifest(publicationId: String) = PlaybackAccessResult.Accepted(audio)
                override suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant> =
                    PlaybackAccessResult.Accepted(PlaybackGrant(PlaybackLocation("https://media.invalid/test-only"), 1_000_000))
            }, account = null, library = null,
            newUuid = { "00000000-0000-4000-8000-000000000001" }, nowMs = { 1_000 })
        val preferences = PreferencesController()
        val shell = TheatreShell(scope, catalog, runtime, preferences, searchDebounceMs = 350, timerMinutesMs = { 60_000L })
    }

    private fun TestScope.fixture() = Fixture(backgroundScope).also { runCurrent() }
    private val workRoute = { f: Fixture -> ShellRoute.Work(ShellWorkId(f.work.id.value)) }

    @Test
    fun the_open_work_follows_the_navigation_stack_and_the_catalog_never_outlives_it() = runTest {
        val f = fixture()
        assertNull(f.catalog.state.value.detail)
        f.shell.dispatch(ShellEvent.Navigate(workRoute(f)))
        runCurrent()
        assertEquals(f.work.id, f.catalog.state.value.detail?.id)
        // The player opens above the work; the work stays the open detail beneath it.
        f.shell.navigate(NavigationEvent.OpenPlayer)
        runCurrent()
        assertEquals(f.work.id, f.catalog.state.value.detail?.id)
        f.shell.dispatch(ShellEvent.Back)
        f.shell.dispatch(ShellEvent.Back)
        runCurrent()
        assertNull(f.catalog.state.value.detail)
        assertEquals(listOf(TheatreRoute.Discovery), f.shell.navigation.value.stack)
    }

    @Test
    fun typed_search_becomes_one_debounced_server_query_only_while_the_search_route_is_open() = runTest {
        val f = fixture()
        f.shell.dispatch(ShellEvent.Navigate(ShellRoute.Search))
        f.shell.dispatch(ShellEvent.Search("sân"))
        f.shell.dispatch(ShellEvent.Search("sân khấu"))
        advanceTimeBy(349)
        assertEquals(PublicCatalogQuery.All, f.catalog.state.value.query, "typing alone sends nothing")
        advanceTimeBy(2)
        runCurrent()
        assertEquals("sân khấu", f.catalog.state.value.query.text)
        assertEquals(listOf("", "sân khấu"), f.queries.map { it.text }, "keystrokes inside the window cost one read")
        f.shell.dispatch(ShellEvent.Back)
        runCurrent()
        assertEquals(PublicCatalogQuery.All, f.catalog.state.value.query, "Discover is never a filtered list")
    }

    @Test
    fun submitting_search_reads_the_typed_text_now_without_waiting_for_the_debounce() = runTest {
        val f = fixture()
        f.shell.dispatch(ShellEvent.Navigate(ShellRoute.Search))
        f.shell.dispatch(ShellEvent.Search("sân khấu"))
        runCurrent()
        assertEquals(PublicCatalogQuery.All, f.catalog.state.value.query, "typing alone still waits")
        f.shell.dispatch(ShellEvent.SubmitSearch)
        runCurrent()
        assertEquals("sân khấu", f.catalog.state.value.query.text)
        advanceTimeBy(1_000)
        runCurrent()
        assertEquals(listOf("", "sân khấu"), f.queries.map { it.text }, "the submit costs exactly one read")
    }

    @Test
    fun an_episode_opens_real_playback_only_when_the_api_listed_it() = runTest {
        val f = fixture()
        f.shell.dispatch(ShellEvent.Navigate(workRoute(f)))
        runCurrent()
        val id = ShellEpisodeId(f.unit.id.value, ShellWorkId(f.work.id.value))
        f.shell.dispatch(ShellEvent.OpenEpisode(id))
        assertEquals(id.value, f.shell.requested.value?.unitId, "the request selects the episode until the player owns it")
        assertEquals(TheatreRoute.Player, f.shell.navigation.value.route)
        runCurrent()
        assertEquals(f.audio, f.runtime.listening.playback.state.value.session?.source?.audio)
        val load = f.decoder.loads.single()
        f.decoder.emit(PlaybackEvent.EngineReady(load.identity))
        f.decoder.emit(PlaybackEvent.EngineStarted(load.identity))
        runCurrent()
        assertEquals(PlaybackStatus.Playing, f.runtime.listening.playback.state.value.status)
        assertNull(f.shell.requested.value, "once the player holds the unit it is the only selection")

        val stranger = ShellEpisodeId("20000000-0000-4000-8000-000000000099", ShellWorkId(f.work.id.value))
        f.shell.dispatch(ShellEvent.OpenEpisode(stranger))
        runCurrent()
        assertEquals(TheatreRoute.Unavailable, f.shell.navigation.value.route)
        assertEquals(1, f.decoder.loads.size, "an unlisted unit never reaches the engine")
    }

    @Test
    fun a_guest_save_and_bookmark_ask_for_an_account_and_sample_controls_do_nothing() = runTest {
        val f = fixture()
        f.shell.dispatch(ShellEvent.SetSaved(ShellWorkId(f.work.id.value), true))
        runCurrent()
        // No identity boundary here: the guest is taken to the Account explanation, nothing is saved.
        assertEquals(TheatreRoute.Account, f.shell.navigation.value.route)
        val before = f.shell.ui.value
        f.shell.dispatch(ShellEvent.Bookmark)
        f.shell.dispatch(ShellEvent.Reset)
        f.shell.dispatch(ShellEvent.CatalogMode(com.cantos.theatre.presentation.model.ShellCatalogMode.Error))
        runCurrent()
        assertEquals(before, f.shell.ui.value)
        assertEquals(TheatreRoute.Account, f.shell.navigation.value.route)
        f.shell.dispatch(ShellEvent.Navigate(ShellRoute.MembershipPreview))
        assertEquals(MembershipScreen.Intro, f.shell.ui.value.membership.screen)
    }

    @Test
    fun the_sleep_timer_pauses_real_playback_and_resets_itself_without_outliving_the_episode() = runTest {
        val f = fixture()
        f.shell.dispatch(ShellEvent.Navigate(workRoute(f)))
        runCurrent()
        f.shell.dispatch(ShellEvent.OpenEpisode(ShellEpisodeId(f.unit.id.value, ShellWorkId(f.work.id.value))))
        runCurrent()
        val load = f.decoder.loads.single()
        f.decoder.emit(PlaybackEvent.EngineReady(load.identity))
        f.decoder.emit(PlaybackEvent.EngineStarted(load.identity))
        runCurrent()
        f.shell.dispatch(ShellEvent.SetTimer(ShellTimer.Fifteen))
        assertEquals(ShellTimer.Fifteen, f.shell.ui.value.timer)
        advanceTimeBy(59_000)
        runCurrent()
        assertEquals(PlaybackStatus.Playing, f.runtime.listening.playback.state.value.status, "not before the time")
        advanceTimeBy(2_000)
        runCurrent()
        assertEquals(PlaybackStatus.Paused, f.runtime.listening.playback.state.value.status)
        assertEquals(ShellTimer.Off, f.shell.ui.value.timer)

        f.shell.dispatch(ShellEvent.SetTimer(ShellTimer.EndOfEpisode))
        assertEquals(ShellTimer.EndOfEpisode, f.shell.ui.value.timer)
        f.shell.dispatch(ShellEvent.SetTimer(ShellTimer.Off))
        assertTrue(f.shell.ui.value.timer == ShellTimer.Off)
    }
}
