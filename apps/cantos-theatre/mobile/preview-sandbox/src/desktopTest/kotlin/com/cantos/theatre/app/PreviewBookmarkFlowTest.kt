package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.adapters.preview.PreviewAccount
import com.cantos.theatre.adapters.preview.PreviewEpisodeIds
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.domain.library.BookmarkSync
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackIntent
import com.cantos.theatre.domain.playback.PlaybackPauseCause
import com.cantos.theatre.domain.playback.PlaybackStatus
import com.cantos.theatre.presentation.model.ShellBookmark
import com.cantos.theatre.presentation.model.ShellRoute
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * Bookmarks through the one Cantos Theatre application mounted as the Desktop Preview mounts it: the production
 * shell, SQLite outbox, sync adapter and player owner, with only the remote authority, the decoder and the
 * identity provider replaced at their ports. This proves flows and semantics on the JVM renderer; it is not API,
 * native audio or device evidence.
 */
@OptIn(ExperimentalTestApi::class)
class PreviewBookmarkFlowTest {
    private val seeded = "b0000000-0000-4000-8000-000000000001"
    private val episode = PreviewEpisodeIds.SonHa03

    private fun signedInWithOneBookmark() = PreviewSeed(
        previewState(stack = listOf(ShellRoute.Library), bookmarks = listOf(ShellBookmark(episode, 768))),
        signedIn = true,
    )

    private fun run(seed: PreviewSeed, body: ComposeUiTest.(PreviewOwners) -> Unit) = runDesktopComposeUiTest(width = 390, height = 844) {
        lateinit var owners: PreviewOwners
        setContent { Box(Modifier.requiredSize(390.dp, 844.dp)) { PreviewTestRoot(seed, onOwners = { owners = it }) } }
        body(owners)
    }

    private fun ComposeUiTest.untilTag(tag: String) = waitUntil(timeoutMillis = 5_000) { onAllNodesWithTag(tag).fetchSemanticsNodes().isNotEmpty() }
    private fun ComposeUiTest.untilNoTag(tag: String) = waitUntil(timeoutMillis = 5_000) { onAllNodesWithTag(tag).fetchSemanticsNodes().isEmpty() }
    private fun PreviewOwners.live() = libraryApi.bookmarkRecords(PreviewAccount.Linh.actor).filter { !it.deleted }

    @Test
    fun the_library_lists_the_synced_bookmark_and_opening_it_restores_the_exact_release_paused() = run(signedInWithOneBookmark()) { owners ->
        waitUntil(timeoutMillis = 5_000) { owners.account.state.value is NativeAccountState.SignedIn && owners.shell.library.value.bookmarks.isNotEmpty() }
        untilTag("library-bookmarks")
        assertEquals(PlaybackStatus.Idle, owners.runtime.listening.playback.state.value.status, "listing bookmarks never plays")
        untilTag("library-bookmark-$seeded-open")
        onNodeWithTag("library-bookmark-$seeded-open").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.runtime.listening.playback.state.value.session != null }
        val session = assertNotNull(owners.runtime.listening.playback.state.value.session)
        assertEquals(768_000L, session.positionMs)
        assertEquals(PlaybackIntent.Pause(PlaybackPauseCause.ColdRestore), session.intent)
        assertEquals(owners.shell.library.value.bookmarks.single().audio, session.source.audio, "the stored release, through the one player")
        assertEquals(TheatreRoute.Player, owners.shell.navigation.value.route)
        assertTrue(owners.engine.commands.none { it.startsWith("start@") }, "opening restores; the listener presses Play")
    }

    @Test
    fun the_player_control_creates_a_synced_bookmark_at_the_confirmed_position_and_pressing_again_adds_nothing() = run(signedInWithOneBookmark()) { owners ->
        waitUntil(timeoutMillis = 5_000) { owners.shell.library.value.bookmarks.isNotEmpty() }
        untilTag("library-bookmark-$seeded-open")
        onNodeWithTag("library-bookmark-$seeded-open").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.runtime.listening.playback.state.value.session != null }
        // The same release position is already bookmarked: the control adds nothing.
        onNodeWithTag("bookmark").performScrollTo().performClick()
        waitForIdle()
        assertEquals(1, owners.live().size, "no duplicate at the same release position")
        owners.runtime.listening.playback.dispatch(PlaybackEvent.SeekRequested(100_000))
        waitUntil(timeoutMillis = 5_000) { owners.runtime.listening.playback.state.value.session?.positionMs == 100_000L }
        onNodeWithTag("bookmark").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.live().size == 2 }
        val made = owners.live().first { it.bookmark_id != seeded }
        assertEquals(100_000L, made.position_ms)
        assertEquals(owners.shell.library.value.bookmarks.map { it.audio }.distinct().single().publicationId, made.audio.publication_id)
        waitUntil(timeoutMillis = 5_000) { owners.shell.library.value.bookmarks.all { it.sync == BookmarkSync.Synced } && owners.shell.library.value.pending == 0 }
        untilTag("player-bookmark-${made.bookmark_id}-open")
        onNodeWithTag("bookmark").performScrollTo().performClick()
        waitForIdle()
        assertEquals(2, owners.live().size, "still two after another press at the same point")
    }

    @Test
    fun an_offline_bookmark_is_marked_and_delivered_once_on_reconnect_and_delete_is_synced() = run(signedInWithOneBookmark()) { owners ->
        waitUntil(timeoutMillis = 5_000) { owners.shell.library.value.bookmarks.isNotEmpty() }
        untilTag("library-bookmark-$seeded-open")
        onNodeWithTag("library-bookmark-$seeded-open").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.runtime.listening.playback.state.value.session != null }
        owners.libraryApi.unavailable = true
        owners.runtime.listening.playback.dispatch(PlaybackEvent.SeekRequested(200_000))
        waitUntil(timeoutMillis = 5_000) { owners.runtime.listening.playback.state.value.session?.positionMs == 200_000L }
        onNodeWithTag("bookmark").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.shell.library.value.bookmarks.any { it.sync == BookmarkSync.Offline } }
        val pending = owners.shell.library.value.bookmarks.first { it.sync == BookmarkSync.Offline }
        untilTag("player-bookmark-${pending.id}-status")
        assertEquals(1, owners.live().size, "the server holds only the seeded bookmark while offline")
        owners.libraryApi.unavailable = false
        owners.runtime.library!!.drain()
        waitUntil(timeoutMillis = 10_000) { owners.live().size == 2 && owners.shell.library.value.pending == 0 }
        untilNoTag("player-bookmark-${pending.id}-status")
        assertEquals(2, owners.live().size)
        // Delete through the row's own control: gone at once, tombstoned on the server, never back after a refresh.
        onNodeWithTag("player-bookmark-${pending.id}-delete").performScrollTo().performClick()
        untilNoTag("player-bookmark-${pending.id}-open")
        waitUntil(timeoutMillis = 5_000) { owners.live().size == 1 && owners.shell.library.value.pending == 0 }
        owners.runtime.library!!.refresh()
        waitForIdle()
        assertEquals(listOf(seeded), owners.shell.library.value.bookmarks.map { it.id })
        assertEquals(1, owners.libraryApi.bookmarkRecords(PreviewAccount.Linh.actor).count { it.deleted })
    }

    @Test
    fun a_guest_pressing_the_control_is_guided_to_sign_in_and_no_bookmark_is_saved() = run(PreviewSeed(previewState(
        stack = listOf(ShellRoute.Player), selectedEpisode = com.cantos.theatre.presentation.model.ShellEpisodeId(episode.value, com.cantos.theatre.presentation.model.ShellWorkId(com.cantos.theatre.adapters.preview.PreviewWorkIds.SonHa.value)),
        positionMs = 30_000))) { owners ->
        untilTag("bookmark")
        onNodeWithTag("bookmark").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { owners.shell.navigation.value.route == TheatreRoute.Login || owners.shell.navigation.value.route == TheatreRoute.Account }
        assertTrue(owners.shell.library.value.bookmarks.isEmpty())
        assertEquals(null, owners.shell.library.value.account)
        assertTrue(owners.libraryApi.bookmarkRecords(PreviewAccount.Linh.actor).isEmpty())
    }
}
