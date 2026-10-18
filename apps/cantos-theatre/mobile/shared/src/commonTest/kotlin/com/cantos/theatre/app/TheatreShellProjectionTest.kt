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
import com.cantos.theatre.domain.library.ContinueEntry
import com.cantos.theatre.domain.library.HistoryEntry
import com.cantos.theatre.domain.library.LibraryResume
import com.cantos.theatre.domain.library.LibraryState
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
import com.cantos.theatre.presentation.model.ShellCatalogMode
import com.cantos.theatre.presentation.model.ShellDetailPhase
import com.cantos.theatre.presentation.model.ShellEpisodeId
import com.cantos.theatre.presentation.model.ShellListPhase
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

    private fun inputs(
        navigation: NavigationState = NavigationState(),
        known: KnownCatalog = KnownCatalog().merge(catalog),
        playback: PlaybackState = PlaybackState(),
        selection: NativeSelection? = null,
        requested: UnitRequest? = null,
        accessFailed: Boolean = false,
        library: LibraryState = LibraryState(),
        locale: ShellLocale = ShellLocale.Vietnamese,
    ) = TheatreShellInputs(navigation, catalog, known, playback, selection, requested, accessFailed, library,
        ShellTheme.Light, locale, "", ShellLibraryTab.Continuing, ShellTimer.Off)

    private fun audio(workNumber: Int, unitNumber: Int, durationMs: Long = 61_000) = requireNotNull(AudioIdentity.of(
        work(workNumber).id.value, unit(unitNumber).id.value, unit(unitNumber).publicationId.value,
        "60000000-0000-4000-8000-000000000006", "b".repeat(64), durationMs))

    private fun session(audio: AudioIdentity, positionMs: Long = 5_000, intent: PlaybackIntent = PlaybackIntent.Play) = PlaybackState(
        generation = 1, session = PlaybackSession(PlaybackSource.streaming(audio), positionMs, 1000, intent,
            PlaybackEngine.Running, PlaybackInterruption.None))

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
        assertEquals(ShellListPhase.Loading, projectTheatreShell(inputs()).catalog.worksPhase)
        send(WorksResolved(token(), PublicCatalogRead.Transport))
        val failed = projectTheatreShell(inputs())
        assertEquals(ShellListPhase.Failed, failed.catalog.worksPhase)
        assertEquals(2, failed.catalog.works.size, "a failed page keeps the rows already shown")
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
        val paused = projectTheatreShell(inputs(playback = session(first, intent = PlaybackIntent.Pause(com.cantos.theatre.domain.playback.PlaybackPauseCause.User))))
        assertEquals(PlaybackUiStatus.Paused, paused.state.playbackStatus)
        assertEquals(false, paused.state.transportRequested)

        val request = UnitRequest(work(1).id.value, unit(2).id.value, unit(2).publicationId.value, "Chương hai", "Tên truyện")
        val pending = projectTheatreShell(inputs(playback = session(first), requested = request))
        assertEquals(unit(2).id.value, pending.state.selectedEpisode.value)
        assertEquals(PlaybackUiStatus.Loading, pending.state.playbackStatus)
        assertEquals(0, pending.state.positionSeconds, "the old unit's position is not the requested one's")
        assertNull(pending.state.listeningFailure)
        val refused = projectTheatreShell(inputs(requested = request, accessFailed = true))
        assertEquals(PlaybackUiStatus.Failed, refused.state.playbackStatus)
        assertEquals(ShellListeningFailure.ReleaseUnavailable, refused.state.listeningFailure)
        assertEquals("Chương hai", refused.catalog.episode(refused.state.selectedEpisode)?.title)
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
}
