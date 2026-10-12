package com.cantos.theatre.demo

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Projection and metadata tests only; playback/navigation decisions belong to their owners. */
class DemoStateTest {
    @Test
    fun diagnostic_seed_defaults_remain_usable_without_global_catalogue_data() {
        val state = DemoState()
        assertEquals(DemoRoute.Discovery, state.route)
        assertEquals(DemoEpisodeId.SonHa03, state.selectedEpisode)
        assertEquals(768, state.positionSeconds)
        assertEquals(PlaybackUiStatus.Paused, state.playbackStatus)
        assertFalse(state.simulating)
        assertFalse(state.transportRequested)
        assertFalse(canGoBack(state))
        assertEquals(DemoLocale.Vietnamese, state.locale)
        val simulatedSeed = DemoState(simulating = true)
        assertEquals(PlaybackUiStatus.Simulating, simulatedSeed.playbackStatus)
        assertTrue(simulatedSeed.simulating)
        assertTrue(simulatedSeed.transportRequested)
    }

    @Test
    fun transport_request_does_not_invent_an_observed_simulation_status() {
        val requested = DemoState(playbackStatus = PlaybackUiStatus.Loading, transportRequested = true)
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
        val stack = mutableListOf<DemoRoute>(DemoRoute.Discovery, DemoRoute.Settings)
        val saved = mutableSetOf(DemoWorkId.SonHa, DemoWorkId.DenKhua)
        val bookmarks = mutableListOf(
            DemoBookmark(DemoEpisodeId.SonHa03, 768),
            DemoBookmark(DemoEpisodeId.DenKhua01, 100),
        )
        val state = DemoState(stack = stack, savedWorks = saved, bookmarks = bookmarks)
        val copied = DemoState().copy(stack = stack, savedWorks = saved, bookmarks = bookmarks)
        stack.clear()
        saved.clear()
        bookmarks.clear()
        listOf(state, copied).forEach { projected ->
            assertEquals(listOf(DemoRoute.Discovery, DemoRoute.Settings), projected.stack)
            assertEquals(setOf(DemoWorkId.SonHa, DemoWorkId.DenKhua), projected.savedWorks)
            assertEquals(
                listOf(DemoBookmark(DemoEpisodeId.SonHa03, 768), DemoBookmark(DemoEpisodeId.DenKhua01, 100)),
                projected.bookmarks,
            )
        }
    }

    @Test
    fun projection_getters_return_detached_collections_even_when_callers_cast_and_modify_them() {
        val state = DemoState(
            stack = listOf(DemoRoute.Discovery, DemoRoute.Settings),
            savedWorks = setOf(DemoWorkId.SonHa, DemoWorkId.DenKhua),
            bookmarks = listOf(DemoBookmark(DemoEpisodeId.SonHa03, 768), DemoBookmark(DemoEpisodeId.DenKhua01, 100)),
        )
        val originalHash = state.hashCode()
        (state.stack as MutableList<DemoRoute>).clear()
        (state.savedWorks as MutableSet<DemoWorkId>).clear()
        (state.bookmarks as MutableList<DemoBookmark>).clear()
        assertEquals(listOf(DemoRoute.Discovery, DemoRoute.Settings), state.stack)
        assertEquals(DemoRoute.Settings, state.route)
        assertEquals(setOf(DemoWorkId.SonHa, DemoWorkId.DenKhua), state.savedWorks)
        assertEquals(2, state.bookmarks.size)
        assertEquals(originalHash, state.hashCode())
        assertEquals(state, state.copy())
    }

    @Test
    fun projection_value_equality_includes_every_fact_without_mirroring_authority() {
        val initial = DemoState()
        val altered = listOf(
            initial.copy(stack = listOf(DemoRoute.Search)),
            initial.copy(selectedEpisode = DemoEpisodeId.DenKhua01),
            initial.copy(positionSeconds = 100),
            initial.copy(positionMs = 768_123L),
            initial.copy(savedWorks = emptySet()),
            initial.copy(bookmarks = listOf(DemoBookmark(DemoEpisodeId.SonHa03, 768))),
            initial.copy(libraryTab = DemoLibraryTab.Saved),
            initial.copy(query = "Sơn Hà"),
            initial.copy(catalogMode = DemoCatalogMode.Error),
            initial.copy(theme = DemoTheme.Dark),
            initial.copy(locale = DemoLocale.English),
            initial.copy(speedPercent = 125),
            initial.copy(timer = DemoTimer.Thirty),
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
        assertEquals(Int.MAX_VALUE, DemoState(positionSeconds = Int.MAX_VALUE).positionSeconds)
        assertEquals(Int.MAX_VALUE, DemoBookmark(DemoEpisodeId.DenKhua01, Int.MAX_VALUE).positionSeconds)
        assertFailsWith<IllegalArgumentException> { DemoState(stack = emptyList()) }
        assertFailsWith<IllegalArgumentException> { DemoState(positionSeconds = -1) }
        assertFailsWith<IllegalArgumentException> { DemoState().copy(positionSeconds = -1) }
        assertFailsWith<IllegalArgumentException> { DemoState(speedPercent = 175) }
        assertFailsWith<IllegalArgumentException> { DemoState().copy(speedPercent = 175) }
        assertFailsWith<IllegalArgumentException> { DemoBookmark(DemoEpisodeId.DenKhua01, -1) }
        assertFailsWith<IllegalArgumentException> { DemoState(positionSeconds = 30, positionMs = 31_123L) }
        assertFailsWith<IllegalArgumentException> { DemoBookmark(DemoEpisodeId.DenKhua01, 30, 31_123L) }
    }

    @Test
    fun catalogue_constructor_snapshots_inventory_and_episode_collections() {
        val episodes = sampleEpisodes().toMutableList()
        val works = mutableListOf(sampleWork(episodes))
        val catalog = DemoCatalogView(works)
        works.clear()
        episodes.clear()
        assertEquals(1, catalog.works.size)
        assertEquals(2, catalog.work(DemoWorkId.SonHa)?.episodes?.size)
        assertEquals("Người giữ tiếng chuông", catalog.episode(DemoEpisodeId.SonHa03)?.title)
        assertNull(catalog.work(DemoWorkId.CanhBuom))
        assertNull(catalog.episode(DemoEpisodeId.CanhBuom01))
        assertEquals(emptyList(), DemoCatalogView().works)
    }

    @Test
    fun catalogue_getters_and_work_lookup_never_expose_the_stored_inventory() {
        val catalog = DemoCatalogView(listOf(sampleWork(sampleEpisodes())))
        val originalHash = catalog.hashCode()
        val escaped = catalog.works
        (escaped.single().episodes as MutableList<DemoEpisode>).clear()
        (escaped as MutableList<DemoWork>).clear()
        val work = catalog.work(DemoWorkId.SonHa)
        assertTrue(work != null)
        (work.episodes as MutableList<DemoEpisode>).clear()
        assertEquals(1, catalog.works.size)
        assertEquals(2, catalog.work(DemoWorkId.SonHa)?.episodes?.size)
        assertEquals("Người giữ tiếng chuông", catalog.episode(DemoEpisodeId.SonHa03)?.title)
        assertEquals(originalHash, catalog.hashCode())
        assertEquals(catalog, DemoCatalogView(listOf(sampleWork(sampleEpisodes()))))
    }

    @Test
    fun search_uses_only_supplied_metadata_with_vietnamese_case_and_direct_diacritic_matching() {
        val catalog = DemoCatalogView(listOf(sampleWork(sampleEpisodes())))
        assertEquals(listOf(DemoWorkId.SonHa), filteredWorks(DemoState(query = "  SƠN HÀ  "), catalog).map { it.id })
        assertEquals(listOf(DemoWorkId.SonHa), filteredWorks(DemoState(query = "NGƯỜI GIỮ TIẾNG CHUÔNG"), catalog).map { it.id })
        assertEquals(emptyList(), filteredWorks(DemoState(query = "son ha"), catalog))
        assertEquals(emptyList(), filteredWorks(DemoState(query = "Đèn Khuya"), catalog))
        assertEquals(catalog.works, filteredWorks(DemoState(query = " \n "), catalog))
    }

    @Test
    fun unavailable_catalogue_scenarios_hide_results_without_deleting_supplied_metadata() {
        val catalog = DemoCatalogView(listOf(sampleWork(sampleEpisodes())))
        listOf(DemoCatalogMode.Loading, DemoCatalogMode.Empty, DemoCatalogMode.Error).forEach { mode ->
            assertEquals(emptyList(), filteredWorks(DemoState(catalogMode = mode), catalog))
            assertEquals("Sơn Hà Ký", catalog.work(DemoWorkId.SonHa)?.title)
        }
        assertEquals(emptyList(), filteredWorks(DemoState(), DemoCatalogView()))
    }

    @Test
    fun the_five_presentation_speed_choices_are_detached_and_match_the_contract_set() {
        assertEquals(listOf(75, 100, 125, 150, 200), SupportedUiSpeedPercents)
        val escaped = SupportedUiSpeedPercents as MutableList<Int>
        escaped[0] = 175
        assertEquals(listOf(75, 100, 125, 150, 200), SupportedUiSpeedPercents)
        listOf(75, 100, 125, 150, 200).forEach { speed ->
            assertEquals(speed, DemoState(speedPercent = speed).speedPercent)
        }
    }

    private fun sampleWork(episodes: List<DemoEpisode>): DemoWork = DemoWork(
        id = DemoWorkId.SonHa,
        title = "Sơn Hà Ký",
        category = "Tiên hiệp",
        description = "Metadata supplied by the test catalog.",
        episodes = episodes,
    )

    private fun sampleEpisodes(): List<DemoEpisode> = listOf(
        DemoEpisode(DemoEpisodeId.SonHa02, DemoWorkId.SonHa, 2, "Con đường qua núi", 1640),
        DemoEpisode(DemoEpisodeId.SonHa03, DemoWorkId.SonHa, 3, "Người giữ tiếng chuông", 1722),
    )
}
