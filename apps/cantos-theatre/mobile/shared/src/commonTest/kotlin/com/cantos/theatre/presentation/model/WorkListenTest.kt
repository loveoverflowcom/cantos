package com.cantos.theatre.presentation.model

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class WorkListenTest {
    private val workId = ShellWorkId("work")
    private val unit = ShellEpisode(ShellEpisodeId("first", workId), workId, 9, "Tiếng chuông trong sương", 120,
        publicationId = "release-a")
    private fun work(vararg units: ShellEpisode, phase: ShellListPhase = ShellListPhase.Complete) =
        ShellWork(workId, "Người gom tiếng chuông", "Truyện", "", units.toList(), unitsPhase = phase)

    @Test
    fun every_source_bound_action_maps_to_the_existing_owner_and_pending_play_can_be_paused() {
        for ((phase, event) in listOf(
            WorkListenPhase.Ready to ShellEvent.OpenEpisode(unit.id),
            WorkListenPhase.Resolving to null,
            WorkListenPhase.PauseLoading to ShellEvent.ToggleSimulation,
            WorkListenPhase.PauseBuffering to ShellEvent.ToggleSimulation,
            WorkListenPhase.PausePlaying to ShellEvent.ToggleSimulation,
            WorkListenPhase.Resume to ShellEvent.ToggleSimulation,
            WorkListenPhase.ResolveFailed to ShellEvent.RetryListening,
            WorkListenPhase.TransportFailed to ShellEvent.ToggleSimulation,
            WorkListenPhase.Unavailable to null,
        )) {
            val actual = workListenProjection(work(unit), WorkListenRelease(unit.id, "release-a", phase))
            assertEquals(phase, actual.phase)
            assertEquals(event, actual.event())
            if (phase != WorkListenPhase.PausePlaying) {
                kotlin.test.assertNotEquals(CopyKey.PlaybackPlaying, actual.status)
            }
        }
    }

    @Test
    fun another_release_of_the_same_unit_never_offers_pause_resume_or_an_engine_status() {
        for (phase in listOf(WorkListenPhase.PausePlaying, WorkListenPhase.Resume, WorkListenPhase.ResolveFailed)) {
            val projected = workListenProjection(work(unit), WorkListenRelease(unit.id, "old-release", phase))
            assertEquals(WorkListenPhase.Ready, projected.phase)
            assertEquals(ShellEvent.OpenEpisode(unit.id), projected.event())
            assertNull(projected.status)
        }
    }

    @Test
    fun unloaded_and_failed_pages_are_not_empty_and_playable_order_is_not_sorted_by_number() {
        for ((phase, expected) in listOf(ShellListPhase.Loading to WorkListenPhase.ListLoading,
            ShellListPhase.More to WorkListenPhase.NoTarget, ShellListPhase.Failed to WorkListenPhase.ListFailed,
            ShellListPhase.Complete to WorkListenPhase.Unavailable)) {
            val actual = workListenProjection(work(phase = phase), null)
            assertEquals(expected, actual.phase)
            assertNull(actual.event())
        }
        val unavailable = unit.copy(durationSeconds = 0)
        val later = unit.copy(id = ShellEpisodeId("later", workId), index = 1, publicationId = "release-b")
        assertEquals(unit, workListenProjection(work(unavailable, unit, later), null).target)
    }

}
