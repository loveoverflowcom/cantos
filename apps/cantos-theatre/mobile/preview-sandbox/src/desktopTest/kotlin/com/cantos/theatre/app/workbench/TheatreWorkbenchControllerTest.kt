package com.cantos.theatre.app.workbench

import com.cantos.theatre.adapters.preview.PreviewEpisodeIds
import com.cantos.theatre.adapters.preview.PreviewFixtures
import com.cantos.theatre.app.PreviewOwners
import com.cantos.theatre.app.PreviewSeed
import com.cantos.theatre.app.PreviewSession
import com.cantos.theatre.adapters.preview.PreviewWorkIds
import com.cantos.theatre.adapters.preview.previewLibraryStore
import com.cantos.theatre.app.previewState
import com.cantos.theatre.app.seed
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLibraryTab
import com.cantos.theatre.presentation.model.ShellListeningFailure
import com.cantos.theatre.presentation.model.ShellProgress
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
            TheatreStep.OpenSonHa, TheatreStep.PlayFirstSonHa, TheatreStep.SeekFifteen, TheatreStep.TogglePlayback, TheatreStep.TimerFifteen)))
        val a = TheatreWorkbenchController(this, recipe)
        val b = TheatreWorkbenchController(this, recipe)
        try {
            a.replay(); b.replay()
            // Eight 50 ms simulation steps and the owners' immediate work; the 15 minute timer must stay armed.
            advanceTimeBy(2_000L); runCurrent()
            assertEquals(a.snapshot().state, b.snapshot().state)
            assertEquals(a.eventLog.value, b.eventLog.value)
            assertEquals("Đèn", a.snapshot().state.query)
            assertEquals(ShellTimer.Fifteen, a.snapshot().state.timer)
            assertEquals(15_000L, a.snapshot().state.positionMs)
            assertEquals("replay complete 8 intents", a.eventLog.value.last())
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
                host.callback()(ShellEvent.SetSaved(PreviewWorkIds.CanhBuom, true))
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
        val restored = accepted(TheatreRecipe.select(TheatreFixture.Restored.id, writeLatencyMs = 500L))
        val host = TheatreWorkbenchController(this, restored)
        val old = host.mount.value
        try {
            advanceTimeBy(2_000L)
            runCurrent()
            val stale = host.callback()
            stale(ShellEvent.SetSaved(PreviewWorkIds.CanhBuom, true))
            runCurrent()
            advanceTimeBy(100L)
            host.select(TheatreRecipe.fixture(TheatreFixture.Fresh))
            val intermediate = host.mount.value
            host.select(restored)
            stale(ShellEvent.Search("obsolete"))
            stale(ShellEvent.SetSaved(PreviewWorkIds.CanhBuom, true))
            advanceTimeBy(2_000L)
            runCurrent()
            assertEquals(1, old.closeCount)
            assertEquals(1, intermediate.closeCount)
            assertEquals(0, old.owners.libraryApi.mutations, "the delayed remote write died with its mount")
            assertEquals(setOf(PreviewWorkIds.SonHa, PreviewWorkIds.DenKhua), host.snapshot().state.savedWorks)
            assertEquals("", host.snapshot().state.query)
            assertEquals(0, host.mount.value.owners.libraryApi.mutations)
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
        val snapshot = mounted.snapshot()
        host.close(); host.close()
        stale(ShellEvent.Search("obsolete"))
        mounted.owners.shell.dispatch(ShellEvent.Search("obsolete"))
        advanceUntilIdle()
        assertEquals(1, mounted.closeCount)
        assertEquals(snapshot, mounted.snapshot())
        assertEquals(0, mounted.owners.libraryApi.mutations)
        assertTrue(requireNotNull(coroutineContext[Job]).isActive)
    }

    @Test
    fun externally_injected_owner_is_not_closed_or_relabelled_by_lifecycle_controls() = runTest {
        val owners = PreviewOwners(backgroundScope, PreviewSeed(previewState(
            selectedEpisode = PreviewEpisodeIds.SonHa03, positionMs = 768_000L)), previewLibraryStore())
        val host = TheatreWorkbenchController(this, injectedOwners = owners)
        val borrowed = host.mount.value
        try {
            settle()
            val before = borrowed.snapshot()
            host.select(TheatreRecipe.fixture(TheatreFixture.Restored))
            host.resetToSelected(); host.replay(); host.recreate(); host.restore(host.snapshot())
            settle()
            assertSame(borrowed, host.mount.value)
            assertEquals(before, borrowed.snapshot())
            assertEquals(5, host.eventLog.value.size)
            assertTrue(host.eventLog.value.all { it.endsWith("owner is externally injected") })
            host.close(); host.close()
            owners.runtime.listening.playback.dispatch(PlaybackEvent.SeekRequested(42_000L))
            settle()
            assertEquals(42_000L, owners.runtime.listening.playback.state.value.session?.positionMs)
            assertEquals(1, borrowed.closeCount)
        } finally { host.close(); owners.close() }
    }

    @Test
    fun recreate_and_snapshot_round_trip_preserve_synthetic_state_with_validated_settings() = runTest {
        val settings = PreviewSettings(ThemeMode.System, Density.Comfortable, PreviewLocale.En, 1.3f,
            ConstraintFrame(320, 800), reducedMotion = true)
        val recipe = TheatreRecipe.fixture(TheatreFixture.Restored, settings)
        val host = TheatreWorkbenchController(this, recipe, systemIsDark = true)
        try {
            settle()
            host.callback()(ShellEvent.Search("Sơn Hà"))
            host.callback()(ShellEvent.SelectLibrary(ShellLibraryTab.Saved))
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
            settle()
            assertEquals(before, host.snapshot().state)
            host.resetToSelected()
            host.restore(decoded)
            settle()
            assertEquals(before, host.snapshot().state)
        } finally { host.close() }
    }

    @Test
    fun recreate_and_import_preserve_paused_selection_for_guests_and_signed_in_listeners() = runTest {
        listOf(TheatreFixture.Fresh, TheatreFixture.Restored).forEach { fixture ->
            listOf(0L, 42_000L).forEach { positionMs ->
                val host = TheatreWorkbenchController(this, TheatreRecipe.fixture(fixture))
                try {
                    settle()
                    host.callback()(ShellEvent.Navigate(ShellRoute.Work(PreviewWorkIds.SonHa)))
                    settle()
                    host.callback()(ShellEvent.OpenEpisode(PreviewEpisodeIds.SonHa03))
                    settle()
                    host.callback()(ShellEvent.ToggleSimulation)
                    host.callback()(ShellEvent.SeekTo((positionMs / 1000L).toInt()))
                    host.callback()(ShellEvent.SetSpeed(125))
                    // A selected, paused unit at zero remains a player even away from its screen.
                    host.callback()(ShellEvent.Navigate(ShellRoute.Library))
                    settle()
                    val before = host.snapshot()
                    assertEquals(PreviewEpisodeIds.SonHa03, before.state.selectedEpisode)
                    assertEquals(positionMs, before.state.positionMs)
                    assertFalse(before.state.transportRequested)
                    val payload = assertIs<TheatreSnapshotExport.Exported>(encodeTheatreSnapshot(before)).text
                    val decoded = assertIs<TheatreSnapshotResult.Accepted>(decodeTheatreSnapshot(payload)).snapshot

                    host.recreate()
                    settle()
                    assertEquals(before.state, host.snapshot().state)
                    assertEquals(before.signedIn, host.snapshot().signedIn)
                    assertTrue(host.mount.value.owners.engine.commands.none { it.startsWith("start@") })

                    host.resetToSelected()
                    host.restore(decoded)
                    settle()
                    assertEquals(before.state, host.snapshot().state)
                    assertEquals(before.signedIn, host.snapshot().signedIn)
                    assertTrue(host.mount.value.owners.engine.commands.none { it.startsWith("start@") })
                } finally { host.close() }
            }
        }
    }

    @Test
    fun malformed_unknown_and_unsupported_snapshots_leave_the_owner_untouched() = runTest {
        val host = TheatreWorkbenchController(this)
        try {
            advanceUntilIdle()
            val before = host.mount.value
            val payload = assertIs<TheatreSnapshotExport.Exported>(encodeTheatreSnapshot(host.snapshot())).text
            assertIs<TheatreSnapshotResult.UnsupportedVersion>(decodeTheatreSnapshot(payload.replace("\"version\": 2", "\"version\": 3")))
            assertIs<TheatreSnapshotResult.UnknownRecipe>(decodeTheatreSnapshot(payload.replace("theatre-fresh-v1", "unknown-recipe")))
            listOf("", "[]", "{}", "not json", payload.replace("\"widthDp\": 390", "\"widthDp\": -1"),
                payload.replace("\"seed\": 1", "\"seed\": 2"),
                payload.replace("\"positionMs\": 0", "\"positionMs\": 999999999"),
                payload.replace("\"transportRequested\": false", "\"transportRequested\": true"),
                payload.replace("\"queryId\": \"empty\"", "\"queryId\": \"/Users/private\""),
                payload.replace("\"version\": 2", "\"secret\": \"x\", \"version\": 2")
            ).forEach { assertIs<TheatreSnapshotResult.Invalid>(decodeTheatreSnapshot(it)) }
            assertSame(before, host.mount.value)
            assertIs<TheatreRecipeResult.UnknownRecipe>(TheatreRecipe.select("missing"))
            assertIs<TheatreRecipeResult.Invalid>(TheatreRecipe.select(TheatreFixture.Fresh.id, writeLatencyMs = -1L))
        } finally { host.close() }
    }

    @Test
    fun snapshots_naming_metadata_only_units_are_refused_and_leave_the_owner_untouched() = runTest {
        val host = TheatreWorkbenchController(this)
        try {
            advanceUntilIdle()
            val before = host.mount.value
            val current = host.snapshot()
            val pending = PreviewFixtures.GioMua.episode(0)
            listOf(
                current.state.copy(selectedEpisode = pending),
                current.state.copy(selectedEpisode = pending, transportRequested = true),
                current.state.copy(progress = listOf(ShellProgress(pending, 0, 0L))),
                current.state.copy(stack = listOf(ShellRoute.Discovery, ShellRoute.Work(ShellWorkId(PreviewFixtures.SongDai.work.id.value)))),
            ).forEach { state ->
                val snapshot = TheatreSnapshot(current.recipe, state, current.signedIn)
                assertIs<TheatreSnapshotExport.UnsupportedState>(encodeTheatreSnapshot(snapshot))
                host.restore(snapshot)
                advanceUntilIdle()
                assertSame(before, host.mount.value)
                assertTrue(host.eventLog.value.last().startsWith("restore unavailable"))
            }
            assertTrue(host.mount.value.owners.engine.commands.isEmpty())
            assertEquals(0, host.mount.value.owners.access.manifests)
            // Released selections keep round-tripping: recreate_and_import_preserve_paused_selection_… above.
        } finally { host.close() }
    }

    @Test
    fun offline_is_an_unreachable_catalog_and_remote_library_with_durable_pending_writes() = runTest {
        val host = TheatreWorkbenchController(this, TheatreRecipe.fixture(TheatreFixture.Offline))
        try {
            advanceUntilIdle()
            host.callback()(ShellEvent.SetSaved(PreviewWorkIds.SonHa, true))
            advanceUntilIdle()
            val owners = host.mount.value.owners
            assertEquals(com.cantos.theatre.presentation.model.ShellCatalogMode.Error, host.snapshot().state.catalogMode)
            assertEquals(0, owners.libraryApi.mutations, "the unreachable remote accepted nothing")
            assertTrue(owners.shell.library.value.pending > 0, "the change is a durable intent, visibly unsent")
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
            host.callback()(ShellEvent.SelectLibrary(ShellLibraryTab.Saved))
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
        val recipe = accepted(TheatreRecipe.select(TheatreFixture.Restored.id, writeLatencyMs = 500L))
        val host = TheatreWorkbenchController(this, recipe)
        try {
            advanceTimeBy(2_000L)
            runCurrent()
            val original = host.mount.value
            host.callback()(ShellEvent.SetSaved(PreviewWorkIds.CanhBuom, true))
            runCurrent()
            advanceTimeBy(100L)
            host.updateSettings(recipe.settings.copy(themeMode = ThemeMode.Dark, fontScale = 1.3f))
            assertSame(original, host.mount.value)
            advanceTimeBy(2_000L)
            runCurrent()
            assertEquals(setOf(PreviewWorkIds.SonHa, PreviewWorkIds.DenKhua, PreviewWorkIds.CanhBuom), host.snapshot().state.savedWorks)
            assertEquals(1, original.owners.libraryApi.mutations)
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

    /** Settles owner work without running the armed 30 minute sleep timer to completion. */
    private fun kotlinx.coroutines.test.TestScope.settle() { advanceTimeBy(1_000L); runCurrent() }
}
