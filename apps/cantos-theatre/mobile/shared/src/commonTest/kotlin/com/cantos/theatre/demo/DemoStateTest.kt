package com.cantos.theatre.demo

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class DemoStateTest {
    @Test
    fun fixture_ids_are_unique_and_every_episode_belongs_to_its_work() {
        val episodes = DemoFixtures.works.flatMap { it.episodes }
        assertEquals(DemoWorkId.entries.toSet(), DemoFixtures.works.map { it.id }.toSet())
        assertEquals(DemoEpisodeId.entries.toSet(), episodes.map { it.id }.toSet())
        assertEquals(DemoEpisodeId.entries.size, episodes.size)
        DemoFixtures.works.forEach { work ->
            assertEquals(work.episodes.indices.map { it + 1 }, work.episodes.map { it.index })
            work.episodes.forEach { episode ->
                assertEquals(work.id, episode.workId)
                assertTrue(episode.durationSeconds > 0)
                assertEquals(episode, DemoFixtures.episode(episode.id))
            }
            assertEquals(work, DemoFixtures.work(work.id))
        }
    }

    @Test
    fun initial_state_is_a_paused_memory_only_position_from_the_reference() {
        val state = DemoState()
        assertEquals(DemoRoute.Discovery, state.route)
        assertEquals(DemoEpisodeId.SonHa03, state.selectedEpisode)
        assertEquals(768, state.positionSeconds)
        assertEquals(1722, DemoFixtures.episode(state.selectedEpisode).durationSeconds)
        assertEquals("Người giữ tiếng chuông", DemoFixtures.episode(state.selectedEpisode).title)
        assertFalse(state.simulating)
        assertFalse(canGoBack(state))
        assertEquals(DemoLocale.Vietnamese, state.locale)
    }

    @Test
    fun repeated_routes_do_not_accumulate_and_two_back_events_return_to_root() {
        var state = reduceDemo(DemoState(), DemoEvent.Navigate(DemoRoute.Work(DemoWorkId.SonHa)))
        repeat(10) { state = reduceDemo(state, DemoEvent.Navigate(DemoRoute.Work(DemoWorkId.SonHa))) }
        state = reduceDemo(state, DemoEvent.Navigate(DemoRoute.Player))
        repeat(10) { state = reduceDemo(state, DemoEvent.Navigate(DemoRoute.Player)) }
        assertEquals(
            listOf(DemoRoute.Discovery, DemoRoute.Work(DemoWorkId.SonHa), DemoRoute.Player),
            state.stack,
        )
        state = reduceDemo(state, DemoEvent.Back)
        assertEquals(DemoRoute.Work(DemoWorkId.SonHa), state.route)
        state = reduceDemo(state, DemoEvent.Back)
        assertEquals(DemoState(), state)
        assertEquals(state, reduceDemo(state, DemoEvent.Back))
    }

    @Test
    fun selecting_a_root_replaces_the_stack_and_preserves_the_demo_session() {
        val state = DemoState(
            stack = listOf(DemoRoute.Discovery, DemoRoute.Work(DemoWorkId.SonHa), DemoRoute.Player),
            simulating = true,
        )
        listOf(DemoRoute.Discovery, DemoRoute.Search, DemoRoute.Library).forEach { route ->
            val selected = reduceDemo(state, DemoEvent.Navigate(route))
            assertEquals(listOf(route), selected.stack)
            assertEquals(state.selectedEpisode, selected.selectedEpisode)
            assertEquals(768, selected.positionSeconds)
            assertTrue(selected.simulating)
        }
    }

    @Test
    fun settings_back_returns_to_search_with_its_query_or_player_with_its_position() {
        val search = DemoState(stack = listOf(DemoRoute.Search), query = "Sơn Hà")
        val searchSettings = reduceDemo(search, DemoEvent.Navigate(DemoRoute.Settings))
        assertEquals(listOf(DemoRoute.Search, DemoRoute.Settings), searchSettings.stack)
        assertEquals(search, reduceDemo(searchSettings, DemoEvent.Back))

        val player = DemoState(
            stack = listOf(DemoRoute.Library, DemoRoute.Player),
            positionSeconds = 900,
            simulating = true,
        )
        val playerSettings = reduceDemo(player, DemoEvent.Navigate(DemoRoute.Settings))
        assertEquals(listOf(DemoRoute.Library, DemoRoute.Player, DemoRoute.Settings), playerSettings.stack)
        assertEquals(player, reduceDemo(playerSettings, DemoEvent.Back))
        assertEquals(playerSettings, reduceDemo(playerSettings, DemoEvent.Navigate(DemoRoute.Settings)))
    }

    @Test
    fun reopening_current_episode_preserves_position_and_repeated_press_does_not_grow_stack() {
        var state = reduceDemo(DemoState(), DemoEvent.OpenEpisode(DemoEpisodeId.SonHa03))
        repeat(10) { state = reduceDemo(state, DemoEvent.OpenEpisode(DemoEpisodeId.SonHa03)) }
        assertEquals(listOf(DemoRoute.Discovery, DemoRoute.Player), state.stack)
        assertEquals(768, state.positionSeconds)
        assertTrue(state.simulating)
    }

    @Test
    fun selecting_another_episode_resets_only_its_position_and_starts_demo_simulation() {
        val state = DemoState(timer = DemoTimer.Thirty, speedPercent = 150)
        val selected = reduceDemo(state, DemoEvent.OpenEpisode(DemoEpisodeId.DenKhua01))
        assertEquals(DemoEpisodeId.DenKhua01, selected.selectedEpisode)
        assertEquals(0, selected.positionSeconds)
        assertTrue(selected.simulating)
        assertEquals(DemoTimer.Thirty, selected.timer)
        assertEquals(150, selected.speedPercent)
    }

    @Test
    fun queue_selection_closes_queue_and_reuses_the_existing_player() {
        val state = DemoState(
            stack = listOf(DemoRoute.Library, DemoRoute.Player, DemoRoute.Queue),
        )
        val selected = reduceDemo(state, DemoEvent.OpenEpisode(DemoEpisodeId.SonHa03))
        assertEquals(listOf(DemoRoute.Library, DemoRoute.Player), selected.stack)
        assertEquals(768, selected.positionSeconds)
        assertEquals(listOf(DemoRoute.Library), reduceDemo(selected, DemoEvent.Back).stack)
        val queuedWithoutPlayer = DemoState(stack = listOf(DemoRoute.Library, DemoRoute.Queue))
        assertEquals(
            listOf(DemoRoute.Library, DemoRoute.Player),
            reduceDemo(queuedWithoutPlayer, DemoEvent.OpenEpisode(DemoEpisodeId.DenKhua01)).stack,
        )
    }

    @Test
    fun seeking_clamps_negative_oversized_and_overflowing_deltas_to_the_episode() {
        val state = DemoState()
        assertEquals(0, reduceDemo(state, DemoEvent.Seek(-1000)).positionSeconds)
        assertEquals(1722, reduceDemo(state, DemoEvent.Seek(10000)).positionSeconds)
        assertEquals(0, reduceDemo(state, DemoEvent.Seek(Int.MIN_VALUE)).positionSeconds)
        assertEquals(1722, reduceDemo(state, DemoEvent.Seek(Int.MAX_VALUE)).positionSeconds)
        assertEquals(0, reduceDemo(state, DemoEvent.SeekTo(Int.MIN_VALUE)).positionSeconds)
        assertEquals(1722, reduceDemo(state, DemoEvent.SeekTo(Int.MAX_VALUE)).positionSeconds)
        assertEquals(753, reduceDemo(state, DemoEvent.Seek(-15)).positionSeconds)
        assertEquals(123, reduceDemo(state, DemoEvent.SeekTo(123)).positionSeconds)
    }

    @Test
    fun seeking_to_end_does_not_change_simulation_or_invent_completion() {
        val state = DemoState(simulating = true)
        val sought = reduceDemo(state, DemoEvent.SeekTo(1722))
        assertEquals(state.copy(positionSeconds = 1722), sought)
    }

    @Test
    fun toggling_simulation_never_advances_the_position() {
        val state = DemoState()
        val active = reduceDemo(state, DemoEvent.ToggleSimulation)
        assertEquals(state.copy(simulating = true), active)
        assertEquals(state, reduceDemo(active, DemoEvent.ToggleSimulation))
    }

    @Test
    fun bookmarks_are_idempotent_and_keep_their_original_episode_and_position() {
        val initial = DemoState()
        var state = reduceDemo(initial, DemoEvent.Bookmark)
        repeat(10) { state = reduceDemo(state, DemoEvent.Bookmark) }
        assertEquals(listOf(DemoBookmark(DemoEpisodeId.SonHa03, 768)), state.bookmarks)
        state = reduceDemo(state, DemoEvent.SeekTo(900))
        state = reduceDemo(state, DemoEvent.Bookmark)
        state = reduceDemo(state, DemoEvent.OpenEpisode(DemoEpisodeId.DenKhua01))
        state = reduceDemo(state, DemoEvent.Bookmark)
        assertEquals(
            listOf(
                DemoBookmark(DemoEpisodeId.SonHa03, 768),
                DemoBookmark(DemoEpisodeId.SonHa03, 900),
                DemoBookmark(DemoEpisodeId.DenKhua01, 0),
            ),
            state.bookmarks,
        )
        assertEquals(emptyList(), initial.bookmarks)
    }

    @Test
    fun saved_work_toggles_and_library_tabs_leave_other_values_unchanged() {
        val initial = DemoState()
        val saved = reduceDemo(initial, DemoEvent.SaveWork(DemoWorkId.DenKhua))
        assertEquals(setOf(DemoWorkId.SonHa, DemoWorkId.DenKhua), saved.savedWorks)
        assertEquals(initial, reduceDemo(saved, DemoEvent.SaveWork(DemoWorkId.DenKhua)))
        assertEquals(emptySet(), reduceDemo(initial, DemoEvent.SaveWork(DemoWorkId.SonHa)).savedWorks)
        DemoLibraryTab.entries.forEach { tab ->
            assertEquals(initial.copy(libraryTab = tab), reduceDemo(initial, DemoEvent.SelectLibrary(tab)))
        }
    }

    @Test
    fun search_matches_vietnamese_case_and_preserves_diacritic_distinctions() {
        val initial = DemoState()
        fun ids(query: String) = filteredWorks(reduceDemo(initial, DemoEvent.Search(query))).map { it.id }
        assertEquals(listOf(DemoWorkId.SonHa), ids("  SƠN HÀ  "))
        assertEquals(listOf(DemoWorkId.DenKhua), ids("ĐÈN KHUYA"))
        assertEquals(listOf(DemoWorkId.CanhBuom), ids("cánh bướm"))
        assertEquals(listOf(DemoWorkId.SonHa), ids("NGƯỜI GIỮ TIẾNG CHUÔNG"))
        assertEquals(emptyList(), ids("son ha"))
        assertEquals(emptyList(), ids("không có trong thư viện"))
        assertEquals(DemoWorkId.entries.toList(), ids(" \n "))
        assertEquals("", initial.query)
    }

    @Test
    fun retry_restores_catalogue_without_erasing_search_or_session() {
        val initial = DemoState(query = "Sơn Hà", simulating = true)
        listOf(DemoCatalogMode.Loading, DemoCatalogMode.Empty, DemoCatalogMode.Error).forEach { mode ->
            val unavailable = reduceDemo(initial, DemoEvent.CatalogMode(mode))
            assertEquals(emptyList(), filteredWorks(unavailable))
            assertEquals(initial, reduceDemo(unavailable, DemoEvent.Retry))
        }
    }

    @Test
    fun supported_speed_choices_are_exact_and_invalid_speed_is_a_no_op() {
        val initial = DemoState()
        listOf(75, 100, 125, 150, 175, 200).forEach { speed ->
            assertEquals(initial.copy(speedPercent = speed), reduceDemo(initial, DemoEvent.SetSpeed(speed)))
        }
        listOf(Int.MIN_VALUE, -1, 0, 74, 80, 201, Int.MAX_VALUE).forEach { speed ->
            assertEquals(initial, reduceDemo(initial, DemoEvent.SetSpeed(speed)))
        }
    }

    @Test
    fun theme_locale_and_timer_are_memory_only_choices() {
        val initial = DemoState()
        assertEquals(
            initial.copy(theme = DemoTheme.Dark),
            reduceDemo(initial, DemoEvent.SetTheme(DemoTheme.Dark)),
        )
        assertEquals(
            initial.copy(locale = DemoLocale.English),
            reduceDemo(initial, DemoEvent.SetLocale(DemoLocale.English)),
        )
        DemoTimer.entries.forEach { timer ->
            assertEquals(initial.copy(timer = timer), reduceDemo(initial, DemoEvent.SetTimer(timer)))
        }
    }

    @Test
    fun reset_restores_every_value_to_the_exact_default_state() {
        val altered = DemoState(
            stack = listOf(DemoRoute.Settings, DemoRoute.Queue),
            selectedEpisode = DemoEpisodeId.DenKhua01,
            positionSeconds = 200,
            simulating = true,
            savedWorks = setOf(DemoWorkId.CanhBuom),
            bookmarks = listOf(DemoBookmark(DemoEpisodeId.DenKhua01, 100)),
            libraryTab = DemoLibraryTab.Bookmarks,
            query = "đèn",
            catalogMode = DemoCatalogMode.Error,
            theme = DemoTheme.Dark,
            locale = DemoLocale.English,
            speedPercent = 200,
            timer = DemoTimer.EndOfEpisode,
        )
        assertEquals(DemoState(), reduceDemo(altered, DemoEvent.Reset))
    }

    @Test
    fun construction_snapshots_caller_owned_navigation_saved_work_and_bookmark_collections() {
        val stack = mutableListOf<DemoRoute>(DemoRoute.Discovery)
        val saved = mutableSetOf(DemoWorkId.SonHa)
        val bookmarks = mutableListOf(DemoBookmark(DemoEpisodeId.SonHa03, 768))
        val state = DemoState(stack = stack, savedWorks = saved, bookmarks = bookmarks)
        stack.clear()
        saved.clear()
        bookmarks.clear()
        assertEquals(DemoRoute.Discovery, state.route)
        assertEquals(listOf(DemoRoute.Discovery), state.stack)
        assertEquals(setOf(DemoWorkId.SonHa), state.savedWorks)
        assertEquals(listOf(DemoBookmark(DemoEpisodeId.SonHa03, 768)), state.bookmarks)
    }

    @Test
    fun copy_snapshots_caller_owned_navigation_saved_work_and_bookmark_collections() {
        val initial = DemoState()
        val stack = mutableListOf<DemoRoute>(DemoRoute.Search, DemoRoute.Settings)
        val saved = mutableSetOf(DemoWorkId.DenKhua)
        val bookmarks = mutableListOf(DemoBookmark(DemoEpisodeId.DenKhua01, 100))
        val copied = initial.copy(stack = stack, savedWorks = saved, bookmarks = bookmarks)
        stack.clear()
        saved.clear()
        bookmarks.clear()
        assertEquals(DemoRoute.Settings, copied.route)
        assertEquals(listOf(DemoRoute.Search, DemoRoute.Settings), copied.stack)
        assertEquals(setOf(DemoWorkId.DenKhua), copied.savedWorks)
        assertEquals(listOf(DemoBookmark(DemoEpisodeId.DenKhua01, 100)), copied.bookmarks)
        assertEquals(DemoState(), initial)
    }

    @Test
    fun no_op_reducer_preserves_owned_collection_snapshots() {
        val stack = mutableListOf<DemoRoute>(DemoRoute.Discovery)
        val saved = mutableSetOf(DemoWorkId.SonHa)
        val bookmarks = mutableListOf(DemoBookmark(DemoEpisodeId.SonHa03, 768))
        val input = DemoState(stack = stack, savedWorks = saved, bookmarks = bookmarks)
        val reduced = reduceDemo(input, DemoEvent.SetSpeed(0))
        stack += DemoRoute.Settings
        saved.clear()
        bookmarks.clear()
        assertEquals(listOf(DemoRoute.Discovery), reduced.stack)
        assertEquals(setOf(DemoWorkId.SonHa), reduced.savedWorks)
        assertEquals(listOf(DemoBookmark(DemoEpisodeId.SonHa03, 768)), reduced.bookmarks)
    }

    @Test
    fun value_equality_covers_every_field_and_copied_values_share_a_hash() {
        val initial = DemoState()
        assertEquals(initial, initial.copy())
        assertEquals(initial.hashCode(), initial.copy().hashCode())
        val altered = listOf(
            initial.copy(stack = listOf(DemoRoute.Search)),
            initial.copy(selectedEpisode = DemoEpisodeId.DenKhua01),
            initial.copy(positionSeconds = 100),
            initial.copy(simulating = true),
            initial.copy(savedWorks = emptySet()),
            initial.copy(bookmarks = listOf(DemoBookmark(DemoEpisodeId.SonHa03, 768))),
            initial.copy(libraryTab = DemoLibraryTab.Saved),
            initial.copy(query = "Sơn Hà"),
            initial.copy(catalogMode = DemoCatalogMode.Error),
            initial.copy(theme = DemoTheme.Dark),
            initial.copy(locale = DemoLocale.English),
            initial.copy(speedPercent = 125),
            initial.copy(timer = DemoTimer.Thirty),
        )
        altered.forEach { state ->
            assertFalse(initial == state)
            assertEquals(state, state.copy())
            assertEquals(state.hashCode(), state.copy().hashCode())
        }
    }

    @Test
    fun construction_and_copy_reject_invalid_position_speed_and_empty_navigation() {
        assertFailsWith<IllegalArgumentException> { DemoState(stack = emptyList()) }
        assertFailsWith<IllegalArgumentException> { DemoState(positionSeconds = -1) }
        assertFailsWith<IllegalArgumentException> { DemoState(positionSeconds = 1723) }
        assertFailsWith<IllegalArgumentException> { DemoState(speedPercent = 80) }
        assertFailsWith<IllegalArgumentException> { DemoState().copy(positionSeconds = 1723) }
        assertFailsWith<IllegalArgumentException> { DemoState().copy(stack = emptyList()) }
        assertFailsWith<IllegalArgumentException> { DemoState().copy(speedPercent = 80) }
        assertFailsWith<IllegalArgumentException> { DemoBookmark(DemoEpisodeId.DenKhua01, 1261) }
    }
}
