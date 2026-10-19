package com.cantos.theatre.presentation.model

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Projection and metadata tests only; playback/navigation decisions belong to their owners. Samples are test-local. */
class ShellStateProjectionTest {
    @Test
    fun sample_defaults_remain_usable_without_global_catalogue_data() {
        val state = sampleState()
        assertEquals(ShellRoute.Discovery, state.route)
        assertEquals(SampleEpisodeIds.SonHa03, state.selectedEpisode)
        assertEquals(768, state.positionSeconds)
        assertEquals(PlaybackUiStatus.Paused, state.playbackStatus)
        assertFalse(state.simulating)
        assertFalse(state.transportRequested)
        assertFalse(canGoBack(state))
        assertEquals(ShellLocale.Vietnamese, state.locale)
        assertEquals(listOf(ShellProgress(SampleEpisodeIds.SonHa03, 768)), state.progress)
        assertEquals(emptyList(), state.history)
        val simulatedSeed = sampleState(simulating = true)
        assertEquals(PlaybackUiStatus.Simulating, simulatedSeed.playbackStatus)
        assertTrue(simulatedSeed.simulating)
        assertTrue(simulatedSeed.transportRequested)
    }

    @Test
    fun transport_request_does_not_invent_an_observed_simulation_status() {
        val requested = sampleState(playbackStatus = PlaybackUiStatus.Loading, transportRequested = true)
        assertFalse(requested.simulating)
        assertTrue(requested.transportRequested)
        assertEquals(PlaybackUiStatus.Loading, requested.playbackStatus)
        PlaybackUiStatus.entries.forEach { status ->
            val projected = requested.copy(playbackStatus = status)
            assertEquals(status == PlaybackUiStatus.Simulating, projected.simulating)
            assertTrue(projected.transportRequested)
        }
        assertFalse(requested.copy(transportRequested = false).transportRequested)
        assertEquals(PlaybackUiStatus.Loading, requested.copy(transportRequested = false).playbackStatus)
    }

    @Test
    fun constructor_and_copy_snapshot_caller_owned_collections() {
        val stack = mutableListOf<ShellRoute>(ShellRoute.Discovery, ShellRoute.Settings)
        val saved = mutableSetOf(SampleWorkIds.SonHa, SampleWorkIds.DenKhua)
        val bookmarks = mutableListOf(
            ShellBookmark(SampleEpisodeIds.SonHa03, 768),
            ShellBookmark(SampleEpisodeIds.DenKhua01, 100),
        )
        val progress = mutableListOf(ShellProgress(SampleEpisodeIds.SonHa03, 768), ShellProgress(SampleEpisodeIds.DenKhua01, 100))
        val history = progress.toMutableList()
        val state = sampleState(stack = stack, savedWorks = saved, bookmarks = bookmarks, progress = progress, history = history)
        val copied = sampleState().copy(stack = stack, savedWorks = saved, bookmarks = bookmarks, progress = progress, history = history)
        stack.clear()
        saved.clear()
        bookmarks.clear()
        progress.clear()
        history.clear()
        listOf(state, copied).forEach { projected ->
            assertEquals(listOf(ShellRoute.Discovery, ShellRoute.Settings), projected.stack)
            assertEquals(setOf(SampleWorkIds.SonHa, SampleWorkIds.DenKhua), projected.savedWorks)
            assertEquals(listOf(ShellProgress(SampleEpisodeIds.SonHa03, 768), ShellProgress(SampleEpisodeIds.DenKhua01, 100)), projected.progress)
            assertEquals(projected.progress, projected.history)
            assertEquals(
                listOf(ShellBookmark(SampleEpisodeIds.SonHa03, 768), ShellBookmark(SampleEpisodeIds.DenKhua01, 100)),
                projected.bookmarks,
            )
        }
    }

    @Test
    fun projection_getters_return_detached_collections_even_when_callers_cast_and_modify_them() {
        val state = sampleState(
            stack = listOf(ShellRoute.Discovery, ShellRoute.Settings),
            savedWorks = setOf(SampleWorkIds.SonHa, SampleWorkIds.DenKhua),
            bookmarks = listOf(ShellBookmark(SampleEpisodeIds.SonHa03, 768), ShellBookmark(SampleEpisodeIds.DenKhua01, 100)),
            progress = listOf(ShellProgress(SampleEpisodeIds.SonHa03, 768), ShellProgress(SampleEpisodeIds.DenKhua01, 100)),
            history = listOf(ShellProgress(SampleEpisodeIds.SonHa03, 768), ShellProgress(SampleEpisodeIds.DenKhua01, 100)),
        )
        val originalHash = state.hashCode()
        (state.stack as MutableList<ShellRoute>).clear()
        (state.savedWorks as MutableSet<ShellWorkId>).clear()
        (state.bookmarks as MutableList<ShellBookmark>).clear()
        (state.progress as MutableList<ShellProgress>).clear()
        (state.history as MutableList<ShellProgress>).clear()
        assertEquals(listOf(ShellRoute.Discovery, ShellRoute.Settings), state.stack)
        assertEquals(ShellRoute.Settings, state.route)
        assertEquals(setOf(SampleWorkIds.SonHa, SampleWorkIds.DenKhua), state.savedWorks)
        assertEquals(2, state.bookmarks.size)
        assertEquals(2, state.progress.size)
        assertEquals(2, state.history.size)
        assertEquals(originalHash, state.hashCode())
        assertEquals(state, state.copy())
    }

    @Test
    fun projection_value_equality_includes_every_fact_without_mirroring_authority() {
        val initial = sampleState()
        val altered = listOf(
            initial.copy(stack = listOf(ShellRoute.Search)),
            initial.copy(selectedEpisode = SampleEpisodeIds.DenKhua01),
            initial.copy(positionSeconds = 100),
            initial.copy(positionMs = 768_123L),
            initial.copy(savedWorks = emptySet()),
            initial.copy(bookmarks = listOf(ShellBookmark(SampleEpisodeIds.SonHa03, 768))),
            initial.copy(progress = emptyList()),
            initial.copy(history = listOf(ShellProgress(SampleEpisodeIds.SonHa03, 768))),
            initial.copy(listeningFailure = ShellListeningFailure.MemoryStoreUnavailable),
            initial.copy(libraryTab = ShellLibraryTab.Saved),
            initial.copy(query = "Sơn Hà"),
            initial.copy(catalogMode = ShellCatalogMode.Error),
            initial.copy(theme = ShellTheme.Dark),
            initial.copy(locale = ShellLocale.English),
            initial.copy(speedPercent = 125),
            initial.copy(timer = ShellTimer.Thirty),
            initial.copy(playbackStatus = PlaybackUiStatus.Loading),
            initial.copy(transportRequested = true),
        )
        assertEquals(initial, initial.copy())
        assertEquals(initial.hashCode(), initial.copy().hashCode())
        altered.forEach { state ->
            assertFalse(initial == state)
            assertEquals(state, state.copy())
            assertEquals(state.hashCode(), state.copy().hashCode())
        }
    }

    @Test
    fun projection_validation_is_structural_and_never_reads_a_fixture_duration() {
        assertEquals(Int.MAX_VALUE, sampleState(positionSeconds = Int.MAX_VALUE).positionSeconds)
        assertEquals(Int.MAX_VALUE, ShellBookmark(SampleEpisodeIds.DenKhua01, Int.MAX_VALUE).positionSeconds)
        assertFailsWith<IllegalArgumentException> { sampleState(stack = emptyList()) }
        assertFailsWith<IllegalArgumentException> { sampleState(positionSeconds = -1) }
        assertFailsWith<IllegalArgumentException> { sampleState().copy(positionSeconds = -1) }
        assertFailsWith<IllegalArgumentException> { sampleState(speedPercent = 175) }
        assertFailsWith<IllegalArgumentException> { sampleState().copy(speedPercent = 175) }
        assertFailsWith<IllegalArgumentException> { ShellBookmark(SampleEpisodeIds.DenKhua01, -1) }
        assertFailsWith<IllegalArgumentException> { sampleState(positionSeconds = 30, positionMs = 31_123L) }
        assertFailsWith<IllegalArgumentException> { ShellBookmark(SampleEpisodeIds.DenKhua01, 30, 31_123L) }
        assertFailsWith<IllegalArgumentException> { ShellProgress(SampleEpisodeIds.DenKhua01, -1) }
        assertFailsWith<IllegalArgumentException> { ShellProgress(SampleEpisodeIds.DenKhua01, 30, 31_123L) }
    }

    @Test
    fun catalogue_constructor_snapshots_inventory_and_episode_collections() {
        val episodes = sampleEpisodes().toMutableList()
        val works = mutableListOf(sampleWork(episodes))
        val catalog = ShellCatalogView(works)
        works.clear()
        episodes.clear()
        assertEquals(1, catalog.works.size)
        assertEquals(2, catalog.work(SampleWorkIds.SonHa)?.episodes?.size)
        assertEquals("Người giữ tiếng chuông", catalog.episode(SampleEpisodeIds.SonHa03)?.title)
        assertNull(catalog.work(SampleWorkIds.CanhBuom))
        assertNull(catalog.episode(SampleEpisodeIds.CanhBuom01))
        assertEquals(emptyList(), ShellCatalogView().works)
    }

    @Test
    fun catalogue_getters_and_work_lookup_never_expose_the_stored_inventory() {
        val catalog = ShellCatalogView(listOf(sampleWork(sampleEpisodes())))
        val originalHash = catalog.hashCode()
        val escaped = catalog.works
        (escaped.single().episodes as MutableList<ShellEpisode>).clear()
        (escaped as MutableList<ShellWork>).clear()
        val work = catalog.work(SampleWorkIds.SonHa)
        assertTrue(work != null)
        (work.episodes as MutableList<ShellEpisode>).clear()
        assertEquals(1, catalog.works.size)
        assertEquals(2, catalog.work(SampleWorkIds.SonHa)?.episodes?.size)
        assertEquals("Người giữ tiếng chuông", catalog.episode(SampleEpisodeIds.SonHa03)?.title)
        assertEquals(originalHash, catalog.hashCode())
        assertEquals(catalog, ShellCatalogView(listOf(sampleWork(sampleEpisodes()))))
    }

    @Test
    fun search_uses_only_supplied_metadata_with_vietnamese_case_and_direct_diacritic_matching() {
        val catalog = ShellCatalogView(listOf(sampleWork(sampleEpisodes())))
        assertEquals(listOf(SampleWorkIds.SonHa), filteredWorks(sampleState(query = "  SƠN HÀ  "), catalog).map { it.id })
        assertEquals(listOf(SampleWorkIds.SonHa), filteredWorks(sampleState(query = "NGƯỜI GIỮ TIẾNG CHUÔNG"), catalog).map { it.id })
        assertEquals(emptyList(), filteredWorks(sampleState(query = "son ha"), catalog))
        assertEquals(emptyList(), filteredWorks(sampleState(query = "Đèn Khuya"), catalog))
        assertEquals(catalog.works, filteredWorks(sampleState(query = " \n "), catalog))
    }

    @Test
    fun unavailable_catalogue_scenarios_hide_results_without_deleting_supplied_metadata() {
        val catalog = ShellCatalogView(listOf(sampleWork(sampleEpisodes())))
        listOf(ShellCatalogMode.Loading, ShellCatalogMode.Empty, ShellCatalogMode.Error).forEach { mode ->
            assertEquals(emptyList(), filteredWorks(sampleState(catalogMode = mode), catalog))
            assertEquals("Sơn Hà Ký", catalog.work(SampleWorkIds.SonHa)?.title)
        }
        assertEquals(emptyList(), filteredWorks(sampleState(), ShellCatalogView()))
    }

    @Test
    fun the_five_presentation_speed_choices_are_detached_and_match_the_contract_set() {
        assertEquals(listOf(75, 100, 125, 150, 200), SupportedUiSpeedPercents)
        val escaped = SupportedUiSpeedPercents as MutableList<Int>
        escaped[0] = 175
        assertEquals(listOf(75, 100, 125, 150, 200), SupportedUiSpeedPercents)
        listOf(75, 100, 125, 150, 200).forEach { speed ->
            assertEquals(speed, sampleState(speedPercent = speed).speedPercent)
        }
    }

    @Test
    fun continue_projection_has_the_latest_point_per_work_without_inventing_history() {
        val unknown = ShellProgress(SampleEpisodeIds.DenKhua01, 40)
        val state = sampleState(progress = listOf(
            ShellProgress(SampleEpisodeIds.SonHa02, 50),
            unknown,
            ShellProgress(SampleEpisodeIds.SonHa03, 768, 768_123L),
        ))
        assertEquals(listOf(ShellProgress(SampleEpisodeIds.SonHa03, 768, 768_123L), unknown), continuingProgress(state))
        assertEquals(emptyList(), state.history)
        assertEquals(emptyList(), continuingProgress(state.copy(progress = emptyList())))
    }

    private fun sampleWork(episodes: List<ShellEpisode>): ShellWork = ShellWork(
        id = SampleWorkIds.SonHa,
        title = "Sơn Hà Ký",
        category = "Tiên hiệp",
        description = "Metadata supplied by the test catalog.",
        episodes = episodes,
    )

    private fun sampleEpisodes(): List<ShellEpisode> = listOf(
        ShellEpisode(SampleEpisodeIds.SonHa02, SampleWorkIds.SonHa, 2, "Con đường qua núi", 1640),
        ShellEpisode(SampleEpisodeIds.SonHa03, SampleWorkIds.SonHa, 3, "Người giữ tiếng chuông", 1722),
    )
}
