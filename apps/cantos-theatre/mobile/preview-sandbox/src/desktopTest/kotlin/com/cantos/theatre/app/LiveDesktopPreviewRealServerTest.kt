package com.cantos.theatre.app

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.library.desktopLibraryStore
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.domain.library.BookmarkSync
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellWorkId
import java.util.concurrent.Executors
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assume
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * Explicit lane of `cmp_live_desktop_preview_against_real_identity_and_http` (theatre_business_journey): the
 * opt-in live Desktop Preview composition, [LiveEnrollmentOwners], over the **real** Cantos Server and the real
 * Identity process on loopback. The shell logs in, reads the real public catalog, saves a work, bookmarks the
 * seeded episode and logs out; the Rust side then reads the Server rows. Audio stays unavailable by design.
 * Skipped, never passed, when the harness environment is absent.
 */
class LiveDesktopPreviewRealServerTest {
    private fun await(what: String, predicate: () -> Boolean) = runBlocking {
        val deadline = System.nanoTime() + 40_000_000_000
        while (!predicate()) {
            check(System.nanoTime() < deadline) { "timed out: $what" }
            delay(25)
        }
    }

    @Test fun live_preview_logs_in_reads_the_real_catalog_saves_bookmarks_and_logs_out() {
        val text = System.getenv("CANTOS_AUTHORIZED_API_ORIGIN")
        Assume.assumeTrue("explicit real Identity journey only", text != null)
        val origin = ApiOrigin.parse(requireNotNull(text))
        assertTrue(origin.toString().startsWith("http://127.0.0.1:"))
        val raw = Json.parseToJsonElement(requireNotNull(System.getenv("CANTOS_LIVE_AUDIO"))).jsonObject
        fun field(key: String) = raw.getValue(key).jsonPrimitive.content
        val audio = assertNotNull(AudioIdentity.of(field("work_id"), field("unit_id"), field("publication_id"),
            field("rendition_id"), field("sha256"), field("duration_ms").toLong()))
        val dispatcher = Executors.newSingleThreadExecutor().asCoroutineDispatcher()
        val scope = CoroutineScope(SupervisorJob() + dispatcher)
        fun <T> on(block: () -> T): T = runBlocking { withContext(dispatcher) { block() } }
        val live = on { LiveEnrollmentOwners(scope, origin, store = desktopLibraryStore(":memory:")) }
        try {
            await("the real public catalog listed the seeded work") {
                live.shell.catalog.state.value.works.items.any { it.id.value == audio.workId }
            }
            val email = assertIs<RegistrationResult.Accepted<RegistrationEmail>>(RegistrationEmail.parse("owner@example.com")).value
            // A guest's save is refused into sign-in, not sent: no durable guest Library exists.
            on { live.shell.dispatch(ShellEvent.SetSaved(ShellWorkId(audio.workId), true)) }
            assertEquals(0, on { live.library.state.value.pending })
            on { assertTrue(live.shell.openLogin()); assertTrue(live.shell.login(email, "a correct horse battery")) }
            await("real login admitted and every real Library section loaded") {
                live.account.verifiedSession.value != null && live.library.state.value.account != null &&
                    LibrarySection.entries.all { live.library.state.value.section(it).loaded }
            }
            val authority = assertNotNull(on { live.account.verifiedSession.value })
            assertEquals(authority.session.actor, on { live.library.state.value.account })
            on { live.shell.dispatch(ShellEvent.SetSaved(ShellWorkId(audio.workId), true)) }
            await("the real Server accepted the save") {
                live.library.state.value.pending == 0 && live.library.state.value.saved.any { it.workId == audio.workId && it.saved }
            }
            val mark = on { assertNotNull(live.library.createBookmark(audio, 30)) }
            await("the real Server accepted the bookmark") {
                live.library.state.value.pending == 0 &&
                    live.library.state.value.bookmarks.any { it.id == mark && it.sync == BookmarkSync.Synced }
            }
            on { live.shell.logout() }
            await("logout fenced the account and its Library") {
                live.account.verifiedSession.value == null && live.library.state.value.account == null
            }
            assertNull(on { live.account.verifiedSession.value })
            println("LIVE-DESKTOP-PREVIEW-OK")
        } finally {
            on { live.close() }
            scope.cancel(); dispatcher.close()
        }
    }
}
