package com.cantos.theatre.app

import com.cantos.theatre.adapters.library.LibraryAuthorization
import com.cantos.theatre.adapters.library.LibraryResult
import com.cantos.theatre.adapters.preview.PreviewCatalogPort
import com.cantos.theatre.adapters.preview.PreviewEpisodeIds
import com.cantos.theatre.adapters.preview.PreviewFixtures
import com.cantos.theatre.adapters.preview.PreviewLibraryApi
import com.cantos.theatre.adapters.preview.PreviewListeningSeed
import com.cantos.theatre.adapters.preview.PreviewPlaybackAccess
import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.PublicCatalogQuery
import com.cantos.theatre.ports.PlaybackAccessFailure
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PublicCatalogRead
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellWorkId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest

/**
 * A disclosed unit without an eligible release has no playback identity (#580). These tests drive the
 * preview ports and the production owners over them; they are JVM sandbox evidence, never native audio.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class MetadataOnlyPreviewTest {
    private val auth = LibraryAuthorization("linh", "session", "bearer")
    private val pending = PreviewFixtures.GioMua.units[0]
    private val released = PreviewFixtures.GioMua.units[1]
    private val unreleased = PreviewFixtures.SongDai.units[0]

    @Test
    fun only_released_units_have_audio_and_the_default_walk_stays_released_only() {
        PreviewFixtures.works.flatMap { it.units }.forEach { unit ->
            assertNotNull(PreviewFixtures.playable(unit.id.value), "default sample ${unit.id.value} is released")
        }
        (PreviewFixtures.works + PreviewFixtures.metadataWorks).forEach { work ->
            work.units.forEach { unit ->
                val audio = PreviewFixtures.audio(work, unit)
                assertEquals(unit.publicationId == null, audio == null, "audio exists exactly for a release")
                audio?.let {
                    assertEquals(unit.publicationId?.value, it.publicationId)
                    assertEquals(unit.durationMs, it.durationMs)
                }
            }
        }
        listOf(pending, unreleased, PreviewFixtures.GioMua.units[2]).forEach { unit ->
            assertNull(unit.publicationId)
            assertNull(unit.durationMs)
            assertNull(PreviewFixtures.playable(unit.id.value))
        }
        assertEquals(1_320_000L, PreviewFixtures.playable(released.id.value)?.audio?.durationMs)
        assertNull(PreviewFixtures.playable("not-a-sample-unit"))
    }

    @Test
    fun the_catalog_lists_metadata_only_units_only_under_the_explicit_opt_in() = runTest {
        val port = PreviewCatalogPort()
        val query = assertIs<CatalogCheck.Valid<PublicCatalogQuery>>(PublicCatalogQuery.of("")).value
        val defaults = assertIs<PublicCatalogRead.Ready<*>>(port.works(null, query))
        assertEquals(PreviewFixtures.works.map { it.work }, (defaults.value as com.cantos.theatre.domain.catalog.PublicWorkPage).items)
        assertIs<PublicCatalogRead.NotFound>(port.work(PreviewFixtures.GioMua.work.id))
        assertIs<PublicCatalogRead.NotFound>(port.units(PreviewFixtures.GioMua.work, null))

        port.disclosesMetadata = true
        val disclosed = assertIs<PublicCatalogRead.Ready<*>>(port.works(null, query)).value as com.cantos.theatre.domain.catalog.PublicWorkPage
        assertEquals((PreviewFixtures.works + PreviewFixtures.metadataWorks).map { it.work }, disclosed.items)
        val units = assertIs<PublicCatalogRead.Ready<*>>(port.units(PreviewFixtures.GioMua.work, null)).value
            as com.cantos.theatre.domain.catalog.PublicUnitPage
        assertEquals(listOf(null, released.publicationId, null), units.items.map { it.publicationId })
        assertEquals(listOf(null, 1_320_000L, null), units.items.map { it.durationMs })
    }

    @Test
    fun library_seeds_no_listening_record_for_metadata_only_units_in_mixed_input() = runTest {
        val api = PreviewLibraryApi(nowMs = { 1L })
        val seeds = listOf(pending, released, unreleased).map { PreviewListeningSeed(it.id.value, 1_000L) }
        api.seed("linh", saved = emptySet(), progress = seeds, history = seeds)
        api.seedBookmarks("linh", seeds)
        val publication = released.publicationId?.value

        val continuing = assertIs<LibraryResult.Value<*>>(api.continuePage(auth, null)).value
            as com.cantos.theatre.adapters.library.wire.PresentedContinuePage
        assertEquals(listOf(publication), continuing.items.map { it.progress.audio.publication_id })
        val history = assertIs<LibraryResult.Value<*>>(api.historyPage(auth, null)).value
            as com.cantos.theatre.adapters.library.wire.PresentedHistoryPage
        assertEquals(listOf(publication), history.items.map { it.audio.publication_id })
        assertEquals(listOf(publication), api.bookmarkRecords("linh").map { it.audio.publication_id })
        assertTrue(api.bookmarkRecords("linh").all { it.audio.duration_ms == 1_320_000L })
    }

    @Test
    fun presentation_binds_the_exact_stored_release_and_saved_needs_a_released_representative() = runTest {
        val api = PreviewLibraryApi(nowMs = { 1L })
        val sonHa = PreviewFixtures.SonHa
        api.seed(
            "linh",
            saved = setOf(sonHa, PreviewFixtures.GioMua, PreviewFixtures.SongDai).map { it.work.id.value }.toSet(),
            progress = listOf(PreviewListeningSeed(sonHa.units[1].id.value, 0), PreviewListeningSeed(released.id.value, 0)),
            history = emptyList(),
        )
        val continuing = assertIs<LibraryResult.Value<*>>(api.continuePage(auth, null)).value
            as com.cantos.theatre.adapters.library.wire.PresentedContinuePage
        assertEquals(2, continuing.items.size)
        continuing.items.forEach { item ->
            // A later unit never borrows the first unit's release (the exact-binding contract).
            val presentation = assertNotNull(item.presentation)
            assertEquals(item.progress.audio.publication_id, presentation.work.publication_id)
            assertEquals(item.progress.audio.unit_id, presentation.unit_id)
        }
        val saved = (assertIs<LibraryResult.Value<*>>(api.savedPage(auth, null)).value
            as com.cantos.theatre.adapters.library.wire.PresentedSavedPage).items.associateBy { it.saved.work_id }
        assertEquals(sonHa.units[0].publicationId?.value, saved.getValue(sonHa.work.id.value).presentation?.publication_id)
        // The mixed Work's first unit has no release, so its representative is the first released unit.
        assertEquals(released.publicationId?.value, saved.getValue(PreviewFixtures.GioMua.work.id.value).presentation?.publication_id)
        // No released unit: the Saved register stays, with no fabricated presentation.
        assertNull(saved.getValue(PreviewFixtures.SongDai.work.id.value).presentation)
    }

    @Test
    fun manifest_and_grant_exist_only_for_released_audio() = runTest {
        val access = PreviewPlaybackAccess(nowMs = { 1L })
        val audio = assertIs<PlaybackAccessResult.Accepted<*>>(access.manifest(assertNotNull(released.publicationId).value)).value
        assertEquals(PreviewFixtures.playable(released.id.value)?.audio, audio)
        // A metadata-only unit has no publication to ask for; any other publication is not found.
        assertEquals(PlaybackAccessResult.Refused(PlaybackAccessFailure.NotFound), access.manifest("30000000-0000-4000-8000-000000000040"))
    }

    @Test
    fun opening_a_metadata_only_unit_requests_no_manifest_and_starts_no_player() = runTest {
        val h = PreviewHarness(backgroundScope)
        try {
            h.owners.catalogPort.disclosesMetadata = true
            runCurrent()
            val work = ShellWorkId(PreviewFixtures.GioMua.work.id.value)
            h.shell.dispatch(ShellEvent.Navigate(ShellRoute.Work(work)))
            runCurrent()
            val playback = h.owners.runtime.listening.playback
            val route = h.route

            h.shell.dispatch(ShellEvent.OpenEpisode(PreviewFixtures.GioMua.episode(0)))
            runCurrent()
            assertEquals(0, h.owners.access.manifests)
            assertNull(playback.state.value.session)
            assertEquals(route, h.route)
            assertTrue(h.owners.engine.commands.isEmpty())

            h.shell.dispatch(ShellEvent.OpenEpisode(PreviewFixtures.GioMua.episode(1)))
            runCurrent()
            assertEquals(1, h.owners.access.manifests)
            assertEquals(released.publicationId?.value, playback.state.value.session?.source?.audio?.publicationId)
            assertEquals(TheatreRoute.Player, h.route)
        } finally { h.close() }
    }

    @Test
    fun a_seeded_metadata_only_selection_restores_no_player_even_when_playing_was_requested() = runTest {
        val seed = PreviewSeed(previewState(selectedEpisode = PreviewFixtures.GioMua.episode(0), transportRequested = true))
        val h = PreviewHarness(backgroundScope, seed)
        try {
            runCurrent()
            assertNull(h.owners.runtime.listening.playback.state.value.session)
            assertEquals(0, h.owners.access.manifests)
            assertTrue(h.owners.engine.commands.isEmpty())
        } finally { h.close() }
        // The released counterpart of the same seed shape still restores its paused player.
        val restored = PreviewHarness(backgroundScope, PreviewSeed(previewState(selectedEpisode = PreviewEpisodeIds.SonHa01, positionMs = 5_000L)))
        try {
            runCurrent()
            val session = assertNotNull(restored.owners.runtime.listening.playback.state.value.session)
            assertEquals(PreviewFixtures.SonHa.units[0].id.value, session.source?.audio?.unitId)
            assertNotEquals(0, restored.owners.engine.commands.size)
        } finally { restored.close() }
    }
}
