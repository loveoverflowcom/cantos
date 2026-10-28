package com.cantos.theatre.app

import com.cantos.theatre.application.catalog.PublicCatalogEffect
import com.cantos.theatre.application.catalog.PublicCatalogEvent
import com.cantos.theatre.application.catalog.PublicCatalogIntent
import com.cantos.theatre.application.catalog.PublicCatalogState
import com.cantos.theatre.application.catalog.UnitsResolved
import com.cantos.theatre.application.catalog.WorkResolved
import com.cantos.theatre.application.catalog.WorksResolved
import com.cantos.theatre.application.catalog.reducePublicCatalog
import com.cantos.theatre.application.playback.NativeSelection
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.cursor
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unit
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.unitsPage
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.work
import com.cantos.theatre.domain.catalog.PublicCatalogSamples.worksPage
import com.cantos.theatre.domain.catalog.PublicUnitKind
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.domain.library.BookmarkEntry
import com.cantos.theatre.domain.library.BookmarkSync
import com.cantos.theatre.domain.library.ContinueEntry
import com.cantos.theatre.domain.library.HistoryEntry
import com.cantos.theatre.domain.library.LibraryLoad
import com.cantos.theatre.domain.library.LibraryPresentation
import com.cantos.theatre.domain.library.LibraryReadFailure
import com.cantos.theatre.domain.library.LibraryResume
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.library.LibrarySectionState
import com.cantos.theatre.domain.library.LibraryState
import com.cantos.theatre.domain.library.LibraryUnitInfo
import com.cantos.theatre.domain.library.LibraryWorkInfo
import com.cantos.theatre.domain.library.LibraryWorkKind
import com.cantos.theatre.domain.library.LibrarySync
import com.cantos.theatre.domain.library.SavedEntry
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngine
import com.cantos.theatre.domain.playback.PlaybackInterruption
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackSession
import com.cantos.theatre.domain.playback.PlaybackSource
import com.cantos.theatre.domain.playback.PlaybackState
import com.cantos.theatre.ports.CatalogProtocolFault
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.presentation.model.PlaybackUiStatus
import com.cantos.theatre.presentation.model.WorkListenPhase
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellDetailPhase
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellLibraryPhase
import com.cantos.theatre.presentation.model.ShellLibraryView
import com.cantos.theatre.presentation.model.ShellListPhase
import com.cantos.theatre.presentation.model.ShellResume
import com.cantos.theatre.presentation.model.ShellRowTitles
import com.cantos.theatre.presentation.model.ShellListeningFailure
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.ShellUnitKind
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.domain.publication.WorkKey
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertIs
import kotlin.test.assertSame
import kotlin.test.assertTrue

/** The old screens are a read-only view of the real owners: every fact below comes from an API value. */
class TheatreShellProjectionTest {
    private var catalog = PublicCatalogState.Initial

    private fun send(event: PublicCatalogEvent): List<PublicCatalogEffect> {
        val transition = reducePublicCatalog(catalog, event)
        catalog = transition.state
        return transition.effects
    }

    private fun token(): Long = catalog.issued

    @Test
    fun disclosed_unit_without_audio_is_shown_as_unavailable() {
        val story = work(1, kind = PublicWorkKind.Story)
        val template = unit(1)
        val pending = assertIs<com.cantos.theatre.domain.catalog.CatalogCheck.Valid<com.cantos.theatre.domain.catalog.PublicUnit>>(
            com.cantos.theatre.domain.catalog.PublicUnit.withoutAudio(template.id, template.kind, template.position.toLong(), "Chương chưa có audio", "")
        ).value
        send(PublicCatalogIntent.OpenWork(story))
        send(WorkResolved(token(), PublicCatalogRead.Ready(story)))
        send(UnitsResolved(token(), PublicCatalogRead.Ready(unitsPage(story, listOf(pending)))))
        val episode = projectTheatreShell(inputs()).catalog.work(ShellWorkId(story.id.value))!!.episodes.single()
        assertEquals("Chương chưa có audio", episode.title)
        assertEquals(0, episode.durationSeconds)
        assertNull(pending.publicationId)
        assertNull(unitRequest(story, pending), "no publication means no playback request")
        assertEquals(template.publicationId?.value, unitRequest(story, template)?.publicationId)
    }

    private fun inputs(
        navigation: NavigationState = NavigationState(),
        known: KnownCatalog = KnownCatalog().merge(catalog),
        playback: PlaybackState = PlaybackState(),
        selection: NativeSelection? = null,
        requested: UnitRequest? = null,
        accessFailed: Boolean = false,
        accessFailure: com.cantos.theatre.ports.PlaybackAccessFailure? = null,
        library: LibraryState = LibraryState(),
        locale: ShellLocale = ShellLocale.Vietnamese,
    ) = TheatreShellInputs(navigation, catalog, known, playback, selection, requested, accessFailed, library,
        ShellTheme.Light, locale, "", ShellLibraryTab.Continuing, ShellTimer.Off, accessFailure)

    private fun audio(workNumber: Int, unitNumber: Int, durationMs: Long = 61_000) = requireNotNull(AudioIdentity.of(
        work(workNumber).id.value, unit(unitNumber).id.value, unit(unitNumber).publicationId!!.value,
        "60000000-0000-4000-8000-000000000006", "b".repeat(64), durationMs))

    private fun session(audio: AudioIdentity, positionMs: Long = 5_000, intent: PlaybackIntent = PlaybackIntent.Play) = PlaybackState(
        generation = 1, session = PlaybackSession(PlaybackSource.streaming(audio), positionMs, 1000, intent,
            PlaybackEngine.Running, PlaybackInterruption.None))

    @Test
    fun work_hero_keeps_pending_play_cancellable_and_never_claims_engine_ack_from_intent() {
        val audio = audio(1, 1)
        val selected = session(audio)
        for ((engine, expected) in listOf(PlaybackEngine.Preparing to WorkListenPhase.PauseLoading,
            PlaybackEngine.Buffering to WorkListenPhase.PauseBuffering, PlaybackEngine.Running to WorkListenPhase.PausePlaying)) {
            val playback = selected.copy(session = selected.session!!.copy(engine = engine))
            assertEquals(expected, projectTheatreShell(inputs(playback = playback)).state.workListenRelease?.phase)
        }
        val newRelease = UnitRequest(audio.workId, audio.unitId, "30000000-0000-4000-8000-000000000099", "New release", "Work")
        val pending = projectTheatreShell(inputs(playback = selected, requested = newRelease)).state.workListenRelease
        assertEquals(newRelease.publicationId, pending?.publicationId)
        assertEquals(WorkListenPhase.Resolving, pending?.phase, "the same unit's old running release is not the newly requested release")
    }

    @Test
    fun an_empty_list_is_loading_then_error_then_empty_and_a_non_empty_list_is_ready_with_its_footer() {
        send(PublicCatalogIntent.Start)
        assertEquals(ShellCatalogMode.Loading, projectTheatreShell(inputs()).state.catalogMode)
        send(WorksResolved(token(), PublicCatalogRead.Transport))
        assertEquals(ShellCatalogMode.Error, projectTheatreShell(inputs()).state.catalogMode)
        send(PublicCatalogIntent.RetryWorks)
        send(WorksResolved(token(), PublicCatalogRead.Ready(worksPage(emptyList()))))
        assertEquals(ShellCatalogMode.Empty, projectTheatreShell(inputs()).state.catalogMode)

        send(PublicCatalogIntent.RefreshWorks)
        send(WorksResolved(token(), PublicCatalogRead.Ready(worksPage(listOf(work(1), work(2)), cursor(1)))))
        val ready = projectTheatreShell(inputs())
        assertEquals(ShellCatalogMode.Ready, ready.state.catalogMode)
        assertEquals(listOf(work(1).id.value, work(2).id.value), ready.catalog.works.map { it.id.value })
        assertEquals(ShellListPhase.More, ready.catalog.worksPhase)
        send(PublicCatalogIntent.LoadMoreWorks)
        assertEquals(com.cantos.theatre.presentation.model.ShellCatalogRequest.Append, projectTheatreShell(inputs()).catalog.request)
        assertEquals(ShellListPhase.Loading, projectTheatreShell(inputs()).catalog.worksPhase)
        send(WorksResolved(token(), PublicCatalogRead.Transport))
        val failed = projectTheatreShell(inputs())
        assertEquals(ShellListPhase.Failed, failed.catalog.worksPhase)
        assertEquals(2, failed.catalog.works.size, "a failed page keeps the rows already shown")
        assertEquals(com.cantos.theatre.presentation.model.ShellCatalogRequest.Idle, failed.catalog.request)
        send(PublicCatalogIntent.RefreshWorks)
        assertEquals(com.cantos.theatre.presentation.model.ShellCatalogRequest.Replacement, projectTheatreShell(inputs()).catalog.request)
        assertEquals(2, projectTheatreShell(inputs()).catalog.works.size)
    }

    @Test
    fun a_listed_work_has_no_count_until_its_units_are_complete_and_uses_the_apis_kind_and_unit_noun() {
        val story = work(1, kind = PublicWorkKind.Story)
        val podcast = work(2, kind = PublicWorkKind.Podcast)
        send(PublicCatalogIntent.Start)
        send(WorksResolved(token(), PublicCatalogRead.Ready(worksPage(listOf(story, podcast)))))
        val listed = projectTheatreShell(inputs()).catalog
        assertEquals("Truyện", listed.works[0].category)
        assertEquals("Podcast", listed.works[1].category)
        assertTrue(listed.works.all { it.unitsPhase == ShellListPhase.More && it.episodes.isEmpty() },
            "an unread unit list is unknown, never zero")
        assertEquals(ShellUnitKind.Chapter, listed.works[0].unitKind)
        assertEquals(ShellUnitKind.Episode, listed.works[1].unitKind)

        send(PublicCatalogIntent.OpenWork(story))
        send(WorkResolved(token(), PublicCatalogRead.Ready(story)))
        send(UnitsResolved(token(), PublicCatalogRead.Ready(unitsPage(story, listOf(unit(1), unit(2)), cursor(3)))))
        val partial = projectTheatreShell(inputs(NavigationState(listOf(TheatreRoute.Discovery, TheatreRoute.Work(WorkKey(story.id.value)))))).catalog
        assertEquals(ShellListPhase.More, partial.work(ShellWorkId(story.id.value))?.unitsPhase)
        send(PublicCatalogIntent.LoadMoreUnits)
        send(UnitsResolved(token(), PublicCatalogRead.Ready(unitsPage(story, listOf(unit(3))))))
        val whole = projectTheatreShell(inputs()).catalog.work(ShellWorkId(story.id.value))!!
        assertEquals(ShellListPhase.Complete, whole.unitsPhase)
        assertEquals(listOf(1, 2, 3), whole.episodes.map { it.index })
        assertTrue(whole.episodes.none { it.numbered }, "a display order is never an episode number")
        assertEquals(61, whole.episodes.first().durationSeconds)
    }

    @Test
    fun the_open_detail_reports_loading_failure_and_not_found_without_inventing_units() {
        val story = work(1)
        send(PublicCatalogIntent.Start)
        send(WorksResolved(token(), PublicCatalogRead.Ready(worksPage(listOf(story)))))
        send(PublicCatalogIntent.OpenWork(story))
        val loading = projectTheatreShell(inputs()).catalog.work(ShellWorkId(story.id.value))!!
        assertEquals(ShellDetailPhase.Loading, loading.detailPhase)
        assertTrue(loading.episodes.isEmpty())
        send(WorkResolved(token(), PublicCatalogRead.NotFound))
        assertEquals(ShellDetailPhase.NotFound, projectTheatreShell(inputs()).catalog.work(ShellWorkId(story.id.value))!!.detailPhase)
        send(PublicCatalogIntent.RetryWork)
        send(WorkResolved(token(), PublicCatalogRead.Protocol(CatalogProtocolFault.Malformed)))
        assertEquals(ShellDetailPhase.Failed, projectTheatreShell(inputs()).catalog.work(ShellWorkId(story.id.value))!!.detailPhase)
    }

    @Test
    fun library_rows_are_named_only_from_known_api_values_with_neutral_wording_otherwise() {
        send(PublicCatalogIntent.Start)
        send(WorksResolved(token(), PublicCatalogRead.Ready(worksPage(listOf(work(1))))))
        val known = audio(1, 1)
        val stranger = audio(9, 9, durationMs = 90_000)
        val library = LibraryState(
            account = "actor",
            saved = listOf(SavedEntry(work(1).id.value, true, 1, LibrarySync.Confirmed), SavedEntry(work(9).id.value, true, 1, LibrarySync.Pending),
                SavedEntry(work(5).id.value, false, 2, LibrarySync.Confirmed)),
            continueListening = listOf(
                ContinueEntry(stranger, 30_000, 1, LibraryResume.ExactAvailable, LibrarySync.Confirmed),
                ContinueEntry(known, 12_500, 1, LibraryResume.NeedsRefresh, LibrarySync.Confirmed),
            ),
            history = listOf(HistoryEntry(known, 1, LibraryResume.ExactAvailable)),
        )
        val view = projectTheatreShell(inputs(library = library))
        assertEquals(setOf(ShellWorkId(work(1).id.value), ShellWorkId(work(9).id.value)), view.state.savedWorks,
            "an unsaved row is not saved")
        // The browsing list never grows by what Library merely references.
        assertEquals(listOf(work(1).id.value), view.catalog.works.map { it.id.value })
        assertEquals(setOf(work(1).id.value, work(9).id.value), view.catalog.knownWorks.map { it.id.value }.toSet())
        // The API lists newest first; the old presentation expects progress oldest first.
        assertEquals(listOf(known.unitId, stranger.unitId), view.state.progress.map { it.episodeId.value })
        assertEquals(12, view.state.progress.first().positionSeconds)
        assertEquals(12, view.state.history.single().positionSeconds, "history reuses its work's continue position")
        val strangerWork = view.catalog.work(ShellWorkId(work(9).id.value))!!
        assertEquals("Tác phẩm", strangerWork.title, "an unseen work is named neutrally, never invented")
        val strangerEpisode = view.catalog.episode(ShellEpisodeId(stranger.unitId, ShellWorkId(stranger.workId)))!!
        assertEquals(90, strangerEpisode.durationSeconds)
        assertTrue(strangerWork.episodes.isNotEmpty(), "the saved row can still resume its identity")
    }

    @Test
    fun an_open_detail_list_is_exactly_what_the_api_walked_even_when_the_player_names_another_unit() {
        val story = work(1)
        send(PublicCatalogIntent.Start)
        send(WorksResolved(token(), PublicCatalogRead.Ready(worksPage(listOf(story)))))
        send(PublicCatalogIntent.OpenWork(story))
        send(WorkResolved(token(), PublicCatalogRead.Ready(story)))
        send(UnitsResolved(token(), PublicCatalogRead.Ready(unitsPage(story, listOf(unit(1))))))
        val playing = audio(1, 7)
        val view = projectTheatreShell(inputs(playback = session(playing)))
        assertEquals(listOf(unit(1).id.value), view.catalog.work(ShellWorkId(story.id.value))!!.episodes.map { it.id.value })
        assertEquals(ShellEpisodeId(playing.unitId, ShellWorkId(story.id.value)), view.state.selectedEpisode)
        assertEquals(61, view.catalog.episode(view.state.selectedEpisode)?.durationSeconds, "the playing unit still resolves")
    }

    @Test
    fun playback_projects_the_players_truth_and_a_pending_request_selects_its_episode() {
        send(PublicCatalogIntent.Start)
        send(WorksResolved(token(), PublicCatalogRead.Ready(worksPage(listOf(work(1))))))
        val first = audio(1, 1)
        val playing = projectTheatreShell(inputs(playback = session(first, positionMs = 7_900)))
        assertEquals(PlaybackUiStatus.Playing, playing.state.playbackStatus)
        assertTrue(playing.state.transportRequested)
        assertEquals(7, playing.state.positionSeconds)
        assertEquals(7_900L, playing.state.positionMs)
        assertEquals(100, playing.state.speedPercent)
        assertEquals(WorkListenPhase.PausePlaying, playing.state.workListenRelease?.phase)
        val paused = projectTheatreShell(inputs(playback = session(first, intent = PlaybackIntent.Pause(com.cantos.theatre.domain.playback.PlaybackPauseCause.User))))
        assertEquals(PlaybackUiStatus.Paused, paused.state.playbackStatus)
        assertEquals(false, paused.state.transportRequested)
        assertEquals(WorkListenPhase.Resume, paused.state.workListenRelease?.phase)

        val request = UnitRequest(work(1).id.value, unit(2).id.value, unit(2).publicationId!!.value, "Chương hai", "Tên truyện")
        val pending = projectTheatreShell(inputs(playback = session(first), requested = request))
        assertEquals(unit(2).id.value, pending.state.selectedEpisode.value)
        assertEquals(PlaybackUiStatus.Loading, pending.state.playbackStatus)
        assertEquals(0, pending.state.positionSeconds, "the old unit's position is not the requested one's")
        assertNull(pending.state.listeningFailure)
        assertEquals(WorkListenPhase.Resolving, pending.state.workListenRelease?.phase)
        val refused = projectTheatreShell(inputs(requested = request, accessFailed = true))
        assertEquals(PlaybackUiStatus.Failed, refused.state.playbackStatus)
        assertEquals(ShellListeningFailure.ReleaseUnavailable, refused.state.listeningFailure)
        assertEquals(WorkListenPhase.ResolveFailed, refused.state.workListenRelease?.phase)
        assertEquals("Chương hai", refused.catalog.episode(refused.state.selectedEpisode)?.title)
        for (failure in listOf(com.cantos.theatre.ports.PlaybackAccessFailure.NotFound,
            com.cantos.theatre.ports.PlaybackAccessFailure.IdentityChanged)) {
            val unavailable = projectTheatreShell(inputs(requested = request, accessFailed = true, accessFailure = failure))
            assertEquals(WorkListenPhase.Unavailable, unavailable.state.workListenRelease?.phase)
        }
    }

    @Test
    fun navigation_and_preferences_pass_through_and_a_cold_start_selects_nothing() {
        val nav = NavigationState(listOf(TheatreRoute.Discovery, TheatreRoute.Work(WorkKey(work(1).id.value)), TheatreRoute.Player))
        val view = projectTheatreShell(inputs(navigation = nav, locale = ShellLocale.English))
        assertEquals(listOf(ShellRoute.Discovery, ShellRoute.Work(ShellWorkId(work(1).id.value)), ShellRoute.Player), view.state.stack)
        assertEquals(ShellLocale.English, view.state.locale)
        assertEquals(NoSelection, view.state.selectedEpisode)
        assertNull(view.catalog.episode(view.state.selectedEpisode), "no mini-player appears for no selection")
        assertEquals(PlaybackUiStatus.Idle, view.state.playbackStatus)
    }

    @Test
    fun known_catalog_replaces_a_changed_release_keeps_unchanged_state_identical_and_stays_bounded() {
        val story = work(1)
        send(PublicCatalogIntent.Start)
        send(WorksResolved(token(), PublicCatalogRead.Ready(worksPage(listOf(story)))))
        send(PublicCatalogIntent.OpenWork(story))
        send(WorkResolved(token(), PublicCatalogRead.Ready(story)))
        send(UnitsResolved(token(), PublicCatalogRead.Ready(unitsPage(story, listOf(unit(1))))))
        val known = KnownCatalog().merge(catalog)
        assertSame(known, known.merge(catalog), "nothing new is nothing emitted")
        assertEquals(unit(1), known.unit(unit(1).id.value)?.unit)
        assertEquals(listOf(unit(1)), known.unitsOf(story.id.value))
        val replaced = unit(1, publication = 2)
        send(PublicCatalogIntent.RefreshWork)
        send(WorkResolved(token(), PublicCatalogRead.Ready(story)))
        send(UnitsResolved(token(), PublicCatalogRead.Ready(unitsPage(story, listOf(replaced)))))
        assertEquals(replaced, known.merge(catalog).unit(unit(1).id.value)?.unit, "a new release replaces the unit in place")

        var bounded = KnownCatalog()
        for (batch in 0 until 25) {
            val works = (1..40).map { work(batch * 40 + it + 100) }
            send(PublicCatalogIntent.RefreshWorks)
            send(WorksResolved(token(), PublicCatalogRead.Ready(worksPage(works.take(40)))))
            bounded = bounded.merge(catalog)
        }
        assertEquals(null, bounded.work(work(101).id.value), "the oldest works are dropped first once the bound is reached")
        assertEquals(work(1_100).id, bounded.work(work(1_100).id.value)?.id)
    }

    // --- Library presentation (#297) ------------------------------------------------------------

    private fun unitInfo(workNumber: Int, unitNumber: Int, workTitle: String, unitTitle: String, kind: LibraryWorkKind = LibraryWorkKind.Story) =
        LibraryUnitInfo(LibraryWorkInfo(work(workNumber).id.value, unit(unitNumber).publicationId!!.value, workTitle, kind, "vi-VN"), unit(unitNumber).id.value, unitTitle)

    @Test
    fun a_library_row_for_a_work_the_catalog_never_listed_is_named_by_the_apis_presentation() {
        val info = unitInfo(7, 7, "Ánh đèn cuối sân khấu — Một lời hẹn chưa nói", "Một tiếng chuông trong sương")
        val library = LibraryState(account = "actor-a",
            saved = listOf(SavedEntry(work(8).id.value, true, 2, LibrarySync.Confirmed,
                LibraryPresentation.Available(LibraryWorkInfo(work(8).id.value, unit(8).publicationId!!.value, "Sơn Hà Ký", LibraryWorkKind.Podcast, "vi-VN")))),
            continueListening = listOf(ContinueEntry(audio(7, 7, 125_000), 40_000, 3, LibraryResume.ExactAvailable, LibrarySync.Confirmed, LibraryPresentation.Available(info))),
            history = listOf(HistoryEntry(audio(7, 7, 125_000), 1, LibraryResume.ExactAvailable, LibraryPresentation.Available(info))))
        val view = projectTheatreShell(inputs(library = library))
        val row = view.state.progress.single()
        assertEquals(ShellRowTitles("Ánh đèn cuối sân khấu — Một lời hẹn chưa nói", "Một tiếng chuông trong sương"), row.titles)
        assertEquals(125, row.durationSeconds)
        assertEquals(40, row.positionSeconds)
        assertEquals(ShellResume.Available, row.resume)
        assertEquals(row.titles, view.state.history.single().titles)
        assertEquals("Một tiếng chuông trong sương", view.catalog.episode(row.episodeId)?.title)
        val saved = view.catalog.work(ShellWorkId(work(8).id.value))
        assertEquals("Sơn Hà Ký", saved?.title)
        assertEquals("Podcast", saved?.category)
        assertEquals(ShellUnitKind.Episode, saved?.unitKind)
        assertTrue(saved?.unavailable == false)
        assertTrue(view.catalog.works.isEmpty(), "referenced Works never join the browsing list")
    }

    @Test
    fun a_row_never_borrows_the_title_or_duration_of_another_release_of_the_same_unit() {
        val story = work(1)
        send(PublicCatalogIntent.Start)
        send(WorksResolved(token(), PublicCatalogRead.Ready(worksPage(listOf(story)))))
        send(PublicCatalogIntent.OpenWork(story))
        send(WorkResolved(token(), PublicCatalogRead.Ready(story)))
        send(UnitsResolved(token(), PublicCatalogRead.Ready(unitsPage(story, listOf(unit(1))))))
        val catalogUnit = unit(1)
        // The listener's stored audio is an older release of the same unit with another duration.
        val old = requireNotNull(AudioIdentity.of(story.id.value, catalogUnit.id.value, "30000000-0000-4000-8000-0000000000aa",
            "60000000-0000-4000-8000-0000000000aa", "c".repeat(64), 90_000))
        assertTrue(old.publicationId != catalogUnit.publicationId!!.value)
        val replaced = LibraryState(account = "actor-a",
            continueListening = listOf(ContinueEntry(old, 30_000, 1, LibraryResume.ReleaseReplaced, LibrarySync.Confirmed, LibraryPresentation.Unavailable)))
        val row = projectTheatreShell(inputs(library = replaced)).state.progress.single()
        assertEquals(ShellResume.ReleaseReplaced, row.resume)
        assertEquals(90, row.durationSeconds, "the exact stored release's duration, not the catalog's 61 seconds")
        assertEquals(30, row.positionSeconds)
        assertTrue(row.titles?.unitTitle != catalogUnit.title, "the newer release's unit title must not label the older audio")
        assertEquals("Tác phẩm", row.titles?.workTitle, "the API said it cannot name this release: no catalog title of a newer release fills in")

        // The same release may be named by the catalog when the API gave no metadata for it.
        val same = LibraryState(account = "actor-a",
            continueListening = listOf(ContinueEntry(audio(1, 1), 30_000, 1, LibraryResume.NeedsRefresh, LibrarySync.Confirmed, LibraryPresentation.Unread)))
        val named = projectTheatreShell(inputs(library = same)).state.progress.single()
        assertEquals(catalogUnit.title, named.titles?.unitTitle)
        assertEquals(story.title, named.titles?.workTitle, "an unread row may use the catalog's name for the same release")
        assertEquals(ShellResume.NeedsRefresh, named.resume)

        // The API's presentation wins over the catalog for its own row.
        val presented = LibraryState(account = "actor-a", continueListening = listOf(ContinueEntry(audio(1, 1), 30_000, 1, LibraryResume.ExactAvailable, LibrarySync.Confirmed,
            LibraryPresentation.Available(unitInfo(1, 1, "Tên theo bản phát hành", "Tập theo bản phát hành")))))
        assertEquals(ShellRowTitles("Tên theo bản phát hành", "Tập theo bản phát hành"), projectTheatreShell(inputs(library = presented)).state.progress.single().titles)
    }

    @Test
    fun a_saved_work_the_api_gave_no_metadata_for_is_kept_and_marked_undescribed_never_named_by_a_sample() {
        val library = LibraryState(account = "actor-a", saved = listOf(
            SavedEntry(work(5).id.value, true, 1, LibrarySync.Confirmed, LibraryPresentation.Unavailable),
            SavedEntry(work(6).id.value, true, 1, LibrarySync.Confirmed, LibraryPresentation.Unread)))
        val view = projectTheatreShell(inputs(library = library))
        assertEquals(listOf(ShellWorkId(work(5).id.value), ShellWorkId(work(6).id.value)), view.state.savedWorks.toList(), "the API's saved order")
        view.state.savedWorks.forEach { id ->
            val shown = view.catalog.work(id)
            assertEquals("Tác phẩm", shown?.title)
        }
        assertTrue(view.catalog.work(ShellWorkId(work(5).id.value))?.unavailable == true, "the API's explicit null says so")
        assertTrue(view.catalog.work(ShellWorkId(work(6).id.value))?.unavailable == false, "an unread record claims nothing")
    }

    @Test
    fun each_library_section_reports_its_own_phase() {
        fun phases(library: LibraryState) = projectTheatreShell(inputs(library = library)).catalog.library
        assertEquals(ShellLibraryView(), phases(LibraryState()), "a guest has no Library to load")
        val fresh = LibraryState(account = "actor-a")
        assertEquals(ShellLibraryView(ShellLibraryPhase.Loading, ShellLibraryPhase.Loading, ShellLibraryPhase.Loading, ShellLibraryPhase.Loading), phases(fresh), "not read yet is loading, not empty")
        val loaded = LibrarySectionState(loaded = true)
        assertEquals(ShellLibraryView(), phases(fresh.copy(sections = LibrarySection.entries.associateWith { loaded })), "read and empty is empty")
        val mixed = fresh.copy(
            sections = mapOf(
                LibrarySection.Continue to loaded.copy(loading = LibraryLoad.NextPage),
                LibrarySection.Saved to loaded.copy(failure = LibraryReadFailure("unavailable", true, nextPage = true)),
                LibrarySection.History to LibrarySectionState(failure = LibraryReadFailure("unavailable", true, nextPage = false)),
                LibrarySection.Bookmarks to loaded.copy(loading = LibraryLoad.Refreshing),
            ),
            cursors = mapOf(LibrarySection.Continue to "c".repeat(64), LibrarySection.Saved to "d".repeat(64)),
        )
        assertEquals(ShellLibraryView(ShellLibraryPhase.LoadingMore, ShellLibraryPhase.MoreFailed, ShellLibraryPhase.Failed, ShellLibraryPhase.Loading), phases(mixed))
        val stale = mixed.copy(
            history = listOf(HistoryEntry(audio(1, 1), 1, LibraryResume.ExactAvailable)),
            bookmarks = listOf(BookmarkEntry("b0000000-0000-4000-8000-000000000001", audio(1, 1), 1_000, LibraryResume.ExactAvailable, BookmarkSync.Synced)),
            sections = mixed.sections + (LibrarySection.Continue to loaded) + (LibrarySection.Saved to loaded) +
                (LibrarySection.Bookmarks to loaded.copy(failure = LibraryReadFailure("unavailable", true, nextPage = false))),
        )
        assertEquals(ShellLibraryView(ShellLibraryPhase.More, ShellLibraryPhase.More, ShellLibraryPhase.Stale, ShellLibraryPhase.Stale), phases(stale),
            "a failed refresh over rows already shown is stale, a section with more pages offers them")
        val refreshing = fresh.copy(sections = mapOf(LibrarySection.Saved to loaded.copy(loading = LibraryLoad.Refreshing)))
        assertEquals(ShellLibraryPhase.Loading, phases(refreshing).saved)
    }

    private val bookmarkId = "b0000000-0000-4000-8000-000000000001"
    private fun info(workNumber: Int, unitNumber: Int, unitTitle: String) = LibraryUnitInfo(
        LibraryWorkInfo(work(workNumber).id.value, unit(unitNumber).publicationId!!.value, "Ánh đèn cuối sân khấu", LibraryWorkKind.Story, "vi-VN"),
        unit(unitNumber).id.value, unitTitle)

    @Test
    fun a_bookmark_projects_its_exact_release_position_titles_and_the_states_that_are_true_of_it() {
        val exact = audio(1, 1)
        val library = LibraryState(account = "actor-a", bookmarks = listOf(
            BookmarkEntry(bookmarkId, exact, 61_000, LibraryResume.ExactAvailable, BookmarkSync.Synced,
                LibraryPresentation.Available(info(1, 1, "Một tiếng chuông trong sương"))),
            BookmarkEntry("b0000000-0000-4000-8000-000000000002", audio(2, 2), 1_500, LibraryResume.ReleaseReplaced, BookmarkSync.Offline),
            BookmarkEntry("b0000000-0000-4000-8000-000000000003", audio(3, 3), 0, LibraryResume.NeedsRefresh, BookmarkSync.Failed),
        ))
        val shown = projectTheatreShell(inputs(library = library)).state.bookmarks
        assertEquals(listOf(bookmarkId, "b0000000-0000-4000-8000-000000000002", "b0000000-0000-4000-8000-000000000003"), shown.map { it.id }, "the Library's order is kept")
        val first = shown[0]
        assertEquals(ShellEpisodeId(exact.unitId, ShellWorkId(exact.workId)), first.episodeId)
        assertEquals(61, first.positionSeconds)
        assertEquals(61_000L, first.positionMs, "exact milliseconds survive projection")
        assertEquals(61, first.durationSeconds, "the duration is the exact stored release's")
        assertEquals("Một tiếng chuông trong sương", first.titles?.unitTitle)
        assertEquals(com.cantos.theatre.presentation.model.ShellBookmarkSync.Synced, first.sync)
        assertEquals(ShellResume.Available, first.resume)
        assertEquals(ShellResume.ReleaseReplaced, shown[1].resume)
        assertEquals(com.cantos.theatre.presentation.model.ShellBookmarkSync.Offline, shown[1].sync)
        assertEquals(1, shown[1].positionSeconds)
        assertEquals(1_500L, shown[1].positionMs)
        assertEquals(ShellResume.NeedsRefresh, shown[2].resume)
        assertEquals(com.cantos.theatre.presentation.model.ShellBookmarkSync.Failed, shown[2].sync)
    }

    @Test
    fun a_bookmark_of_an_unavailable_release_borrows_no_title_from_a_replacement_release() {
        send(PublicCatalogIntent.Start)
        send(WorksResolved(token(), PublicCatalogRead.Ready(worksPage(listOf(work(1))))))
        val oldRelease = requireNotNull(AudioIdentity.of(work(1).id.value, unit(1).id.value, "30000000-0000-4000-8000-0000000000aa",
            "60000000-0000-4000-8000-0000000000aa", "d".repeat(64), 61_000))
        val library = LibraryState(account = "actor-a", bookmarks = listOf(
            BookmarkEntry(bookmarkId, oldRelease, 5_000, LibraryResume.ReleaseReplaced, BookmarkSync.Synced, LibraryPresentation.Unavailable)))
        val projected = projectTheatreShell(inputs(library = library))
        val row = projected.state.bookmarks.single()
        assertEquals(ShellResume.ReleaseReplaced, row.resume)
        // The API said it cannot name this release: the catalog's title for the NEW release must not label it.
        assertEquals(ShellRowTitles("Tác phẩm", "Tác phẩm"), row.titles, "neutral wording, never the replacement's title")
        assertTrue(unit(1).title != row.titles?.unitTitle)
    }

    @Test
    fun a_guest_has_no_bookmarks_even_if_stale_state_names_some() {
        val stale = LibraryState(account = null, bookmarks = listOf(BookmarkEntry(bookmarkId, audio(1, 1), 1, LibraryResume.ExactAvailable, BookmarkSync.Synced)))
        // The shell only ever exposes a Library for the verified account; the projection still shows a guest no loading phase.
        assertEquals(ShellLibraryView(), projectTheatreShell(inputs(library = stale)).catalog.library)
    }
}
