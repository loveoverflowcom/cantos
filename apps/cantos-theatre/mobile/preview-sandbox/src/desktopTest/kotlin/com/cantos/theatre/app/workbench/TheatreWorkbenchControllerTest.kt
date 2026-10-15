package com.cantos.theatre.app.workbench

import com.cantos.theatre.app.mockAppContainer
import com.cantos.theatre.demo.DemoWorkIds
import com.cantos.theatre.demo.DemoEpisodeIds
import com.cantos.theatre.app.AppContainer
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellListeningFailure
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTimer
import com.cantos.theatre.presentation.model.ShellWorkId
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.ui.Density
import com.cantos.ui.ThemeMode
import com.cantos.ui.preview.workbench.ConstraintFrame
import com.cantos.ui.preview.workbench.PreviewLocale
import com.cantos.ui.preview.workbench.PreviewSettings
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotSame
import kotlin.test.assertSame
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class TheatreWorkbenchControllerTest {
    @Test
    fun same_recipe_seed_and_script_produce_identical_complete_projection() = runTest {
        val recipe = accepted(TheatreRecipe.select(TheatreFixture.Restored.id, steps = listOf(
            TheatreStep.OpenSearch, TheatreStep.SearchNight, TheatreStep.SaveSonHa,
            TheatreStep.OpenSonHa, TheatreStep.SeekFifteen, TheatreStep.Bookmark, TheatreStep.TimerFifteen)))
        val a = TheatreWorkbenchController(this, recipe)
        val b = TheatreWorkbenchController(this, recipe)
        try {
            a.replay(); b.replay()
            advanceUntilIdle()
            assertEquals(a.snapshot().state, b.snapshot().state)
            assertEquals(a.eventLog.value, b.eventLog.value)
            assertEquals("Đèn", a.snapshot().state.query)
            assertEquals(ShellTimer.Fifteen, a.snapshot().state.timer)
            assertEquals(15_000L, a.snapshot().state.positionMs)
            assertEquals(15_000L, a.snapshot().state.bookmarks.last().positionMs)
            assertEquals("replay complete 7 intents", a.eventLog.value.last())
        } finally { a.close(); b.close() }
    }

    @Test
    fun reset_to_selected_fresh_or_restored_never_uses_the_demo_default() = runTest {
        TheatreFixture.entries.take(2).forEach { fixture ->
            val recipe = TheatreRecipe.fixture(fixture)
            val host = TheatreWorkbenchController(this, recipe)
            try {
                advanceUntilIdle()
                val expected = host.snapshot().state
                host.callback()(ShellEvent.Search("Khác"))
                host.callback()(ShellEvent.SelectLibrary(ShellLibraryTab.Saved))
                host.callback()(ShellEvent.SetSaved(DemoWorkIds.CanhBuom, true))
                advanceUntilIdle()
                val previous = host.mount.value
                host.callback()(ShellEvent.Reset)
                advanceUntilIdle()
                assertNotSame(previous, host.mount.value)
                assertEquals(1, previous.closeCount)
                assertEquals(expected, host.snapshot().state)
                assertEquals(recipe.id, host.selectedRecipe.id)
            } finally { host.close() }
        }
    }

    @Test
    fun rapid_a_b_a_and_delayed_write_cannot_update_the_new_selected_mount() = runTest {
        val fresh = accepted(TheatreRecipe.select(TheatreFixture.Fresh.id, writeLatencyMs = 500L))
        val host = TheatreWorkbenchController(this, fresh)
        val old = host.mount.value
        try {
            runCurrent()
            val stale = host.callback()
            stale(ShellEvent.SetSaved(DemoWorkIds.CanhBuom, true))
            runCurrent()
            advanceTimeBy(100L)
            host.select(TheatreRecipe.fixture(TheatreFixture.Restored))
            val intermediate = host.mount.value
            host.select(fresh)
            stale(ShellEvent.Search("obsolete"))
            stale(ShellEvent.SetSaved(DemoWorkIds.CanhBuom, true))
            advanceUntilIdle()
            assertEquals(1, old.closeCount)
            assertEquals(1, intermediate.closeCount)
            assertEquals(0L, old.database?.state?.value?.revision)
            assertEquals(emptySet(), host.snapshot().state.savedWorks)
            assertEquals("", host.snapshot().state.query)
            assertEquals(0L, host.mount.value.database?.state?.value?.revision)
            assertTrue(requireNotNull(coroutineContext[Job]).isActive)
        } finally { host.close() }
    }

    @Test
    fun disposing_during_replay_and_write_closes_once_and_refuses_stale_callbacks() = runTest {
        val recipe = accepted(TheatreRecipe.select(TheatreFixture.Fresh.id, writeLatencyMs = 500L,
            steps = listOf(TheatreStep.SaveSonHa, TheatreStep.SearchNight)))
        val host = TheatreWorkbenchController(this, recipe)
        host.replay()
        val mounted = host.mount.value
        val stale = host.callback()
        advanceTimeBy(75L)
        runCurrent()
        val snapshot = mounted.session.snapshot()
        host.close(); host.close()
        stale(ShellEvent.Search("obsolete"))
        mounted.session.dispatch(ShellEvent.Search("obsolete"))
        advanceUntilIdle()
        assertEquals(1, mounted.closeCount)
        assertEquals(snapshot, mounted.session.snapshot())
        assertEquals(0L, mounted.database?.state?.value?.revision)
        assertTrue(requireNotNull(coroutineContext[Job]).isActive)
    }

    @Test
    fun externally_injected_owner_is_not_closed_or_relabelled_by_lifecycle_controls() = runTest {
        val app = mockAppContainer(this)
        val host = TheatreWorkbenchController(this, injectedContainer = app)
        val borrowed = host.mount.value
        try {
            advanceUntilIdle()
            val before = borrowed.session.snapshot()
            host.select(TheatreRecipe.fixture(TheatreFixture.Restored))
            host.resetToSelected(); host.replay(); host.recreate(); host.restore(host.snapshot())
            advanceUntilIdle()
            assertSame(borrowed, host.mount.value)
            assertEquals(before, borrowed.session.snapshot())
            assertEquals(5, host.eventLog.value.size)
            assertTrue(host.eventLog.value.all { it.endsWith("owner is externally injected") })
            host.close(); host.close()
            app.playback.dispatch(PlaybackEvent.SeekRequested(42_000L))
            advanceUntilIdle()
            assertEquals(42_000L, app.playback.state.value.session?.positionMs)
            assertEquals(1, borrowed.closeCount)
        } finally { host.close(); app.close() }
    }

    @Test
    fun recreate_and_snapshot_round_trip_preserve_synthetic_state_with_validated_settings() = runTest {
        val settings = PreviewSettings(ThemeMode.System, Density.Comfortable, PreviewLocale.En, 1.3f,
            ConstraintFrame(320, 800), reducedMotion = true)
        val recipe = TheatreRecipe.fixture(TheatreFixture.Restored, settings)
        val host = TheatreWorkbenchController(this, recipe, systemIsDark = true)
        try {
            advanceUntilIdle()
            host.callback()(ShellEvent.Search("Sơn Hà"))
            host.callback()(ShellEvent.SelectLibrary(ShellLibraryTab.Bookmarks))
            host.callback()(ShellEvent.SetTimer(ShellTimer.Thirty))
            val before = host.snapshot().state
            val payload = assertIs<TheatreSnapshotExport.Exported>(encodeTheatreSnapshot(host.snapshot())).text
            val decoded = assertIs<TheatreSnapshotResult.Accepted>(decodeTheatreSnapshot(payload)).snapshot
            assertEquals(before, decoded.state)
            assertEquals(settings, decoded.recipe.settings)
            assertFalse(payload.contains("Coroutine"))
            assertFalse(payload.contains("/Users/"))
            assertFalse(payload.contains("credential"))
            host.recreate()
            advanceUntilIdle()
            assertEquals(before, host.snapshot().state)
            host.resetToSelected()
            host.restore(decoded)
            advanceUntilIdle()
            assertEquals(before, host.snapshot().state)
        } finally { host.close() }
    }

    @Test
    fun malformed_unknown_and_unsupported_snapshots_leave_the_owner_untouched() = runTest {
        val host = TheatreWorkbenchController(this)
        try {
            advanceUntilIdle()
            val before = host.mount.value
            val payload = assertIs<TheatreSnapshotExport.Exported>(encodeTheatreSnapshot(host.snapshot())).text
            assertIs<TheatreSnapshotResult.UnsupportedVersion>(decodeTheatreSnapshot(payload.replace("\"version\": 1", "\"version\": 2")))
            assertIs<TheatreSnapshotResult.UnknownRecipe>(decodeTheatreSnapshot(payload.replace("theatre-fresh-v1", "unknown-recipe")))
            listOf("", "[]", "{}", "not json", payload.replace("\"widthDp\": 390", "\"widthDp\": -1"),
                payload.replace("\"seed\": 1", "\"seed\": 2"),
                payload.replace("\"positionMs\": 0", "\"positionMs\": 999999999"),
                payload.replace("\"queryId\": \"empty\"", "\"queryId\": \"/Users/private\""),
                payload.replace("\"version\": 1", "\"secret\": \"x\", \"version\": 1")
            ).forEach { assertIs<TheatreSnapshotResult.Invalid>(decodeTheatreSnapshot(it)) }
            assertSame(before, host.mount.value)
            assertIs<TheatreRecipeResult.UnknownRecipe>(TheatreRecipe.select("missing"))
            assertIs<TheatreRecipeResult.Invalid>(TheatreRecipe.select(TheatreFixture.Fresh.id, writeLatencyMs = -1L))
        } finally { host.close() }
    }

    @Test
    fun offline_is_an_explicit_unavailable_catalog_and_write_simulation() = runTest {
        val host = TheatreWorkbenchController(this, TheatreRecipe.fixture(TheatreFixture.Offline))
        try {
            advanceUntilIdle()
            host.callback()(ShellEvent.SetSaved(DemoWorkIds.SonHa, true))
            advanceUntilIdle()
            assertEquals(com.cantos.theatre.presentation.model.ShellCatalogMode.Error, host.snapshot().state.catalogMode)
            assertEquals(emptySet(), host.snapshot().state.savedWorks)
            assertEquals(ShellListeningFailure.MemoryStoreUnavailable, host.snapshot().state.listeningFailure)
            assertEquals(1L, host.mount.value.database?.state?.value?.refusedWrites)
            assertIs<TheatreSnapshotExport.UnsupportedState>(encodeTheatreSnapshot(host.snapshot()))
        } finally { host.close() }
    }

    @Test
    fun appearance_and_system_theme_preserve_entered_values_navigation_and_listening() = runTest {
        val host = TheatreWorkbenchController(this, TheatreRecipe.fixture(TheatreFixture.Restored))
        try {
            advanceUntilIdle()
            host.callback()(ShellEvent.Navigate(ShellRoute.Search))
            host.callback()(ShellEvent.Search("Đèn"))
            host.callback()(ShellEvent.SelectLibrary(ShellLibraryTab.Bookmarks))
            val before = host.snapshot().state
            val originalMount = host.mount.value
            val settings = PreviewSettings(ThemeMode.System, Density.Comfortable, PreviewLocale.En, 2f,
                ConstraintFrame(320, 800), reducedMotion = true)
            host.updateSettings(settings)
            advanceUntilIdle()
            assertSame(originalMount, host.mount.value)
            assertEquals(before.query, host.snapshot().state.query)
            assertEquals(before.stack, host.snapshot().state.stack)
            assertEquals(before.libraryTab, host.snapshot().state.libraryTab)
            assertEquals(before.progress, host.snapshot().state.progress)
            host.updateSystemTheme(true)
            advanceUntilIdle()
            assertEquals(com.cantos.theatre.presentation.model.ShellTheme.Dark, host.snapshot().state.theme)
            assertEquals("Đèn", host.snapshot().state.query)
            assertEquals(settings, host.selectedRecipe.settings)
            host.callback()(ShellEvent.Search("private arbitrary secret"))
            assertIs<TheatreSnapshotExport.UnsupportedState>(encodeTheatreSnapshot(host.snapshot()))
        } finally { host.close() }
    }

    @Test
    fun appearance_changes_keep_the_same_owner_and_do_not_cancel_a_pending_write() = runTest {
        val recipe = accepted(TheatreRecipe.select(TheatreFixture.Fresh.id, writeLatencyMs = 500L))
        val host = TheatreWorkbenchController(this, recipe)
        try {
            advanceUntilIdle()
            val original = host.mount.value
            host.callback()(ShellEvent.SetSaved(DemoWorkIds.CanhBuom, true))
            runCurrent()
            advanceTimeBy(100L)
            host.updateSettings(recipe.settings.copy(themeMode = ThemeMode.Dark, fontScale = 1.3f))
            assertSame(original, host.mount.value)
            advanceUntilIdle()
            assertEquals(setOf(DemoWorkIds.CanhBuom), host.snapshot().state.savedWorks)
            assertEquals(1L, original.database?.state?.value?.revision)
            assertEquals(0, original.closeCount)
        } finally { host.close() }
    }

    @Test
    fun export_rejects_state_that_its_importer_cannot_represent() {
        val recipe = TheatreRecipe.fixture(TheatreFixture.Fresh)
        val seed = recipe.initialState(false)
        val oversizedStack = seed.copy(stack = List(33) { ShellRoute.Library })
        assertIs<TheatreSnapshotExport.UnsupportedState>(encodeTheatreSnapshot(TheatreSnapshot(recipe, oversizedStack)))
        val privateQuery = seed.copy(query = "private arbitrary secret")
        assertIs<TheatreSnapshotExport.UnsupportedState>(encodeTheatreSnapshot(TheatreSnapshot(recipe, privateQuery)))
        val transient = seed.copy(playbackStatus = com.cantos.theatre.presentation.model.PlaybackUiStatus.Buffering)
        assertIs<TheatreSnapshotExport.UnsupportedState>(encodeTheatreSnapshot(TheatreSnapshot(recipe, transient)))
    }

    @Test
    fun direct_restore_cannot_bypass_snapshot_validation_or_replace_the_current_owner() = runTest {
        val host = TheatreWorkbenchController(this)
        try {
            advanceUntilIdle()
            val original = host.mount.value
            val before = host.snapshot()
            val privateState = before.state.copy(query = "private arbitrary secret")
            host.restore(TheatreSnapshot(before.recipe, privateState))
            assertSame(original, host.mount.value)
            assertEquals(before.state, host.snapshot().state)
            val impossiblePosition = before.state.copy(positionSeconds = 999999, positionMs = 999999000L)
            host.restore(TheatreSnapshot(before.recipe, impossiblePosition))
            advanceUntilIdle()
            assertSame(original, host.mount.value)
            assertEquals(before.state, host.snapshot().state)
            assertEquals(2, host.eventLog.value.size)
            assertTrue(host.eventLog.value.all { it.startsWith("restore unavailable:") })
        } finally { host.close() }
    }

    private fun accepted(result: TheatreRecipeResult): TheatreRecipe = assertIs<TheatreRecipeResult.Accepted>(result).recipe
}
