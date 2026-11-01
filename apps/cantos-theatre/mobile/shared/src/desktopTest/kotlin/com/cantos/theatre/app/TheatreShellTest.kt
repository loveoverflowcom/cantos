package com.cantos.theatre.app

import com.cantos.theatre.application.catalog.PublicCatalogController
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.preferences.PreferencesController
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.application.identity.StubIdentityV1Api
import com.cantos.theatre.application.identity.v1Grant
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.domain.catalog.PublicCatalogSamples
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.membership.MembershipScreen
import com.cantos.theatre.domain.publication.WorkKey
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
import com.cantos.theatre.presentation.model.ShellSearchKind
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.ShellWorkId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext
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

    private class Fixture(scope: CoroutineScope, account: NativeAccountController? = null) {
        var manifestGate: CompletableDeferred<Unit>? = null
        var manifestCalls = 0
        val work: PublicWork = PublicCatalogSamples.work(1, title = "Ánh đèn cuối sân khấu")
        val unit: PublicUnit = PublicCatalogSamples.unit(1, durationMs = 125_000)
        val audio = requireNotNull(AudioIdentity.of(work.id.value, unit.id.value, unit.publicationId!!.value,
            "60000000-0000-4000-8000-000000000006", "b".repeat(64), unit.durationMs!!))
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
                override suspend fun manifest(publicationId: String): PlaybackAccessResult<AudioIdentity> {
                    manifestCalls++
                    manifestGate?.await()
                    return PlaybackAccessResult.Accepted(audio)
                }
                override suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant> =
                    PlaybackAccessResult.Accepted(PlaybackGrant(PlaybackLocation("https://media.invalid/test-only"), 1_000_000))
            }, account = account, library = null,
            newUuid = { "00000000-0000-4000-8000-000000000001" }, nowMs = { 1_000 })
        val preferences = PreferencesController()
        val shell = TheatreShell(scope, catalog, runtime, preferences, searchDebounceMs = 350, timerMinutesMs = { 60_000L })
    }

    private fun TestScope.fixture() = Fixture(backgroundScope).also { runCurrent() }
    private val workRoute = { f: Fixture -> ShellRoute.Work(ShellWorkId(f.work.id.value)) }

    /** Synthetic identity/journal only; all session and navigation decisions are production owners. */
    private class Identity(scope: CoroutineScope, saved: Boolean = false) : StubIdentityV1Api() {
        var grant = v1Grant('a', 'b', "actor-a", "00000000-0000-4000-8000-000000000001", 121, 601, 901, 1, 1)
        var record: NativeCredentialRecord? = if (saved) NativeCredentialRecord.Active(grant) else null
        var marked = false
        var loginCalls = 0
        var loginGate: CompletableDeferred<Unit>? = null
        var sessionGate: CompletableDeferred<Unit>? = null
        var cleanupGate: CompletableDeferred<Unit>? = null
        var sessionFailure: IdentityV1Failure? = null
        val account = NativeAccountController(scope, this, object : NativeCredentialJournal {
            override fun read() = record?.let { NativeJournalRead.Found(it) } ?: NativeJournalRead.Missing
            override fun write(record: NativeCredentialRecord): com.cantos.theatre.ports.CredentialFailure? { this@Identity.record = record; return null }
            override fun clear(): com.cantos.theatre.ports.CredentialFailure? { record = null; return null }
        }, object : LogoutBarrier {
            override fun read() = if (marked) LogoutBarrierRead.Pending else LogoutBarrierRead.Clear
            override fun mark(): com.cantos.theatre.ports.CredentialFailure? { marked = true; return null }
            override fun clear(): com.cantos.theatre.ports.CredentialFailure? { marked = false; return null }
        }, now = { 1 })

        override suspend fun login(email: RegistrationEmail, password: String): IdentityV1Result<NativeGrant> {
            loginCalls += 1
            // Simulates a remote commit whose response arrives despite local cancellation.
            withContext(NonCancellable) { loginGate?.await() }
            return IdentityV1Result.Accepted(grant)
        }
        override suspend fun session(previous: NativeGrant): IdentityV1Result<NativeSession> {
            sessionGate?.await()
            return sessionFailure?.let { IdentityV1Result.Refused(it) } ?: IdentityV1Result.Accepted(previous.view)
        }
        override suspend fun revokeFamily(previous: NativeGrant): IdentityV1Result<Unit> {
            cleanupGate?.await()
            return IdentityV1Result.Accepted(Unit)
        }
    }

    // cantos.presentation.guest-navigation.run.unresolved-session / run.public-deep-link
    @Test fun restoring_preserves_public_and_private_intents_without_enabling_auth_early() = runTest {
        val identity = Identity(backgroundScope, saved = true)
        val gate = CompletableDeferred<Unit>().also { identity.sessionGate = it }
        val f = Fixture(backgroundScope, identity.account)
        runCurrent()
        assertEquals(NativeAccountState.Pending, identity.account.state.value)
        assertFalse(f.shell.openLogin())
        assertFalse(f.shell.openRegister())
        f.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Library))
        assertEquals(TheatreRoute.Library, f.shell.navigation.value.route)
        f.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Work(WorkKey(f.work.id.value))))
        runCurrent()
        assertEquals(f.work.id, f.catalog.state.value.detail?.id, "public data does not await session lookup")
        gate.complete(Unit)
        runCurrent()
        assertIs<NativeAccountState.SignedIn>(identity.account.state.value)
        assertEquals(listOf(TheatreRoute.Library, TheatreRoute.Work(WorkKey(f.work.id.value))), f.shell.navigation.value.stack)
    }

    // cantos.presentation.guest-navigation.run.auth-return / run.auth-cancel-late
    @Test fun cancelled_login_response_cannot_reopen_private_return_and_a_new_intent_can_complete() = runTest {
        val identity = Identity(backgroundScope)
        val f = Fixture(backgroundScope, identity.account)
        runCurrent()
        f.shell.dispatch(ShellEvent.Navigate(ShellRoute.Library))
        assertEquals(listOf(TheatreRoute.Landing), f.shell.navigation.value.stack)
        assertTrue(f.shell.openLogin())
        val email = (RegistrationEmail.parse("listener@example.test") as RegistrationResult.Accepted).value
        val late = CompletableDeferred<Unit>().also { identity.loginGate = it }
        assertTrue(f.shell.login(email, "synthetic-password"))
        assertFalse(f.shell.login(email, "synthetic-password"))
        runCurrent()
        f.shell.dispatch(ShellEvent.Back)
        runCurrent()
        late.complete(Unit)
        runCurrent()
        assertEquals(NativeAccountState.SignedOut, identity.account.state.value)
        assertEquals(NavigationState(), f.shell.navigation.value)
        assertEquals(1, identity.loginCalls)
        f.shell.dispatch(ShellEvent.Navigate(ShellRoute.History))
        assertTrue(f.shell.openRegister())
        assertEquals(TheatreRoute.Register, f.shell.navigation.value.route)
        f.shell.dispatch(ShellEvent.Back)
        assertEquals(TheatreRoute.Login, f.shell.navigation.value.route)
        assertTrue(f.shell.login(email, "synthetic-password"))
        runCurrent()
        assertEquals(TheatreRoute.History, f.shell.navigation.value.route)
        assertTrue(f.shell.navigation.value.stack.none { it.isAuth || it == TheatreRoute.Landing })
    }

    // cantos.presentation.guest-navigation.run.logout-expiry / run.unresolved-session
    @Test fun owner_initiated_account_switch_and_logout_cannot_retain_the_old_private_return() = runTest {
        val identity = Identity(backgroundScope, saved = true)
        val f = Fixture(backgroundScope, identity.account)
        runCurrent()
        f.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Account))
        identity.grant = v1Grant('c', 'd', "actor-b", "00000000-0000-4000-8000-000000000002", 121, 601, 901, 1, 1)
        val email = (RegistrationEmail.parse("listener@example.test") as RegistrationResult.Accepted).value
        identity.account.login(email, "synthetic-password")
        runCurrent()
        assertEquals("actor-b", (identity.account.state.value as NativeAccountState.SignedIn).actor)
        assertEquals(NavigationState(listOf(TheatreRoute.Discovery)), f.shell.navigation.value)
        f.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Library))
        identity.account.logout()
        runCurrent()
        assertEquals(NativeAccountState.SignedOut, identity.account.state.value)
        assertEquals(NavigationState(), f.shell.navigation.value)
        assertNull(f.shell.navigation.value.returnTo)
    }

    // cantos.presentation.guest-navigation.run.logout-expiry / run.unresolved-session
    @Test fun outage_retains_recovery_but_confirmed_loss_and_logout_remove_the_private_stack() = runTest {
        val identity = Identity(backgroundScope, saved = true)
        val f = Fixture(backgroundScope, identity.account)
        runCurrent()
        assertEquals(TheatreRoute.Discovery, f.shell.navigation.value.route)
        // This fixture has no durable Library adapter; exercise the production route/session seam.
        f.shell.navigate(NavigationEvent.Navigate(TheatreRoute.Library))
        identity.sessionFailure = IdentityV1Failure.ServiceUnavailable
        identity.account.revalidate(identity.grant.view.session)
        runCurrent()
        assertEquals(NativeAccountFailure.Api, (identity.account.state.value as NativeAccountState.Failed).reason)
        assertEquals(TheatreRoute.Library, f.shell.navigation.value.route, "outage is unresolved, never confirmed logout")
        assertNull(identity.account.confirmedLoss)
        identity.sessionFailure = null
        assertEquals(RestoreOutcome.Requested, f.shell.restoreSession())
        runCurrent()
        assertIs<NativeAccountState.SignedIn>(identity.account.state.value)
        identity.sessionFailure = IdentityV1Failure.Unauthorized
        identity.account.revalidate(identity.grant.view.session)
        runCurrent()
        assertEquals(NativeAccountFailure.Expired, (identity.account.state.value as NativeAccountState.Failed).reason)
        assertEquals(NavigationState(), f.shell.navigation.value)
        identity.sessionFailure = null
        assertTrue(f.shell.openLogin())
        val email = (RegistrationEmail.parse("listener@example.test") as RegistrationResult.Accepted).value
        assertTrue(f.shell.login(email, "synthetic-password"))
        runCurrent()
        f.shell.dispatch(ShellEvent.Navigate(ShellRoute.Account))
        val cleanup = CompletableDeferred<Unit>().also { identity.cleanupGate = it }
        f.shell.logout()
        runCurrent()
        assertEquals(NativeAccountState.CleanupRequired, identity.account.state.value)
        assertFalse(f.shell.openLogin())
        assertFalse(f.shell.openRegister())
        assertEquals(NavigationState(), f.shell.navigation.value)
        f.shell.dispatch(ShellEvent.Back)
        cleanup.complete(Unit)
        runCurrent()
        assertEquals(NativeAccountState.SignedOut, identity.account.state.value)
        assertEquals(NavigationState(), f.shell.navigation.value)
    }

    @Test
    fun repeat_open_during_resolve_keeps_one_exact_release_request() = runTest {
        val f = fixture()
        f.shell.dispatch(ShellEvent.Navigate(workRoute(f)))
        runCurrent()
        val olderRelease = requireNotNull(AudioIdentity.of(f.audio.workId, f.audio.unitId,
            "80000000-0000-4000-8000-000000000099", f.audio.renditionId, f.audio.sha256, f.audio.durationMs))
        f.runtime.listening.restore(olderRelease, 500)
        runCurrent()
        f.runtime.listening.playback.dispatch(PlaybackEvent.PlayRequested)
        runCurrent()
        f.manifestGate = CompletableDeferred()
        val intent = ShellEvent.OpenEpisode(ShellEpisodeId(f.unit.id.value, ShellWorkId(f.work.id.value)))
        f.shell.dispatch(intent)
        runCurrent()
        assertEquals(1, f.manifestCalls)
        assertEquals(f.audio.publicationId, f.shell.requested.value?.publicationId,
            "pausing an older publication of the same unit cannot clear the pending release")
        f.shell.dispatch(intent)
        runCurrent()
        assertEquals(1, f.manifestCalls, "a pending exact-release resolve is not canceled and restarted by repeated input")
        f.manifestGate!!.complete(Unit)
        runCurrent()
        assertEquals(f.audio, f.runtime.listening.playback.state.value.session?.source?.audio)
        assertEquals(PlaybackStatus.Buffering, f.runtime.listening.playback.state.value.status)
    }

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
        assertEquals(listOf(TheatreRoute.Landing), f.shell.navigation.value.stack)
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
    fun a_kind_chip_reads_now_with_the_typed_text_and_all_clears_only_the_kind() = runTest {
        val f = fixture()
        f.shell.dispatch(ShellEvent.Navigate(ShellRoute.Search))
        f.shell.dispatch(ShellEvent.Search("sân khấu"))
        f.shell.dispatch(ShellEvent.SelectSearchKind(ShellSearchKind.Podcast))
        runCurrent()
        assertEquals(PublicWorkKind.Podcast, f.catalog.state.value.query.kind)
        assertEquals("sân khấu", f.catalog.state.value.query.text, "the chip keeps the text")
        f.shell.dispatch(ShellEvent.SelectSearchKind(ShellSearchKind.Novel))
        runCurrent()
        assertEquals(PublicWorkKind.Novel, f.catalog.state.value.query.kind)
        f.shell.dispatch(ShellEvent.SelectSearchKind(ShellSearchKind.All))
        runCurrent()
        assertNull(f.catalog.state.value.query.kind)
        assertEquals("sân khấu", f.catalog.state.value.query.text)
        advanceTimeBy(1_000)
        runCurrent()
        assertEquals(
            listOf("" to null, "sân khấu" to PublicWorkKind.Podcast, "sân khấu" to PublicWorkKind.Novel, "sân khấu" to null),
            f.queries.map { it.text to it.kind }, "each chip costs exactly one read and typing in between is not repeated")
    }

    @Test
    fun a_kind_filter_alone_is_a_server_query_and_never_outlives_the_search_route() = runTest {
        val f = fixture()
        f.shell.dispatch(ShellEvent.Navigate(ShellRoute.Search))
        f.shell.dispatch(ShellEvent.SelectSearchKind(ShellSearchKind.Story))
        runCurrent()
        assertEquals(PublicCatalogQuery.of("", PublicWorkKind.Story).let { (it as CatalogCheck.Valid).value }, f.catalog.state.value.query)
        f.shell.dispatch(ShellEvent.Back)
        runCurrent()
        assertEquals(PublicCatalogQuery.All, f.catalog.state.value.query, "Discover is never a filtered list")
        assertEquals(ShellSearchKind.Story, f.shell.ui.value.searchKind, "the chip is restored when Search reopens")
        f.shell.dispatch(ShellEvent.Navigate(ShellRoute.Search))
        runCurrent()
        assertEquals(PublicWorkKind.Story, f.catalog.state.value.query.kind)
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
        // No identity boundary: Landing explains unavailable login, and nothing is saved.
        assertEquals(TheatreRoute.Landing, f.shell.navigation.value.route)
        val before = f.shell.ui.value
        f.shell.dispatch(ShellEvent.Bookmark)
        f.shell.dispatch(ShellEvent.Reset)
        f.shell.dispatch(ShellEvent.CatalogMode(com.cantos.theatre.presentation.model.ShellCatalogMode.Error))
        runCurrent()
        assertEquals(before, f.shell.ui.value)
        assertEquals(TheatreRoute.Landing, f.shell.navigation.value.route)
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
