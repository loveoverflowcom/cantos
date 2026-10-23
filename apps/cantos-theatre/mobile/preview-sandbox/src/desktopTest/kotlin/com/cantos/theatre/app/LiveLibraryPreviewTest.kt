package com.cantos.theatre.app

import com.cantos.theatre.adapters.library.desktopLibraryStore
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.domain.library.LibrarySection
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CoroutineExceptionHandler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.junit.After
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * The opt-in live Desktop Preview's own composition over **real loopback HTTP**: [PreviewNativeHttp]
 * (the JDK transport), Identity v1 login, [com.cantos.theatre.adapters.http.NativeAuthorizedHttpPort],
 * the production Library client, sync and SQLDelight store, and the real [TheatreShell].
 *
 * The server below is a protocol fixture that stamps its own responses. It is mocked server evidence,
 * not the Cantos Server: it proves how the preview wires and transports the production boundary and how the
 * fences react to real HTTP responses. The real Server and Identity lane is the explicit Rust harness.
 */
class LiveLibraryPreviewTest {
    private val actor = "listener_a"
    private val family = "11111111-2222-4333-8444-555555555555"
    private val work = "10000000-0000-4000-8000-000000000001"
    private val access = "ci1_n_" + "a".repeat(64)
    private val refresh = "ci1_r_" + "b".repeat(64)

    private val executor = Executors.newSingleThreadExecutor()
    private val dispatcher = executor.asCoroutineDispatcher()
    /** Anything a transport coroutine lets escape lands here instead of on the global handler. */
    private val uncaught = CopyOnWriteArrayList<Throwable>()
    private val scope = CoroutineScope(SupervisorJob() + dispatcher + CoroutineExceptionHandler { _, failure -> uncaught += failure })
    private val fixtures = mutableListOf<LoopbackServer>()
    private val owners = mutableListOf<LiveEnrollmentOwners>()

    @After fun tearDown() {
        owners.forEach { runCatching { on { it.close() } } }
        fixtures.forEach { it.stop() }
        scope.cancel()
        executor.shutdownNow()
    }

    private fun <T> on(block: () -> T): T = runBlocking { withContext(dispatcher) { block() } }

    private fun await(what: String, predicate: () -> Boolean) = runBlocking {
        val deadline = System.nanoTime() + 20_000_000_000
        while (!on(predicate)) {
            check(System.nanoTime() < deadline) { "timed out: $what" }
            delay(20)
        }
    }

    private fun start(): Pair<LoopbackServer, LiveEnrollmentOwners> {
        val fixture = LoopbackServer(actor, family, work, access, refresh).also(fixtures::add)
        val live = on { LiveEnrollmentOwners(scope, fixture.origin, store = desktopLibraryStore(":memory:")) }.also(owners::add)
        return fixture to live
    }

    private fun LiveEnrollmentOwners.signIn(fixture: LoopbackServer? = null) {
        val email = assertIs<RegistrationResult.Accepted<RegistrationEmail>>(RegistrationEmail.parse("listener@example.test")).value
        on { assertTrue(shell.openLogin()); assertTrue(shell.login(email, "synthetic-test-password")) }
        try {
            await("admitted session and four loaded Library sections") {
                account.verifiedSession.value != null && library.state.value.account == actor &&
                    LibrarySection.entries.all { library.state.value.section(it).loaded }
            }
        } catch (timeout: IllegalStateException) {
            val diagnostic = on { "account=${account.state.value} verified=${account.verifiedSession.value != null} libraryAccount=${library.state.value.account}" }
            throw IllegalStateException("$diagnostic requests=${fixture?.seen?.map { it.method + " " + it.route }}", timeout)
        }
    }

    private fun LiveEnrollmentOwners.fenced() = account.verifiedSession.value == null && library.state.value.account == null

    @Test fun login_admits_the_session_and_the_production_library_reads_every_section_over_real_http() {
        val (fixture, live) = start()
        live.signIn(fixture)
        val authority = on { assertNotNull(live.account.verifiedSession.value) }
        assertEquals(actor, authority.session.actor)
        val reads = fixture.seen.filter { it.route.startsWith("/library/") }
        assertEquals(setOf("continue", "saved", "history", "bookmarks"), reads.map { it.route.removePrefix("/library/").substringBefore('?') }.toSet())
        assertTrue(reads.all { it.method == "GET" && it.authorization == "Bearer $access" && it.session == family })
        assertEquals(emptyList(), fixture.seen.filter { !it.cookie.isNullOrEmpty() }.map { "${it.method} ${it.route}" }, "native requests carry no ambient cookie (the catalog sends only its deliberate empty guard)")
    }

    @Test fun the_live_window_requests_the_public_catalog_without_signing_in() {
        val (fixture, _) = start()
        await("the catalog was requested") { fixture.seen.any { it.route.startsWith("/works") } }
        assertTrue(fixture.seen.filter { it.route.startsWith("/works") }.all { it.authorization == null && it.session == null })
    }

    @Test fun a_save_is_sent_once_with_its_stamp_and_settles_from_the_stamped_receipt() {
        val (fixture, live) = start()
        live.signIn(fixture)
        on { live.library.setSaved(work, true) }
        await("durable Saved delivered") { live.library.state.value.pending == 0 && fixture.committed.size == 1 }
        val write = fixture.seen.single { it.method == "PUT" }
        assertEquals("/library/saved/$work", write.route)
        assertEquals("Bearer $access", write.authorization)
        assertEquals(family, write.session)
    }

    @Test fun a_lost_committed_response_is_reconciled_by_its_receipt_before_any_replay() {
        val (fixture, live) = start()
        live.signIn(fixture)
        var dropped = false
        // The server commits the write, then the connection dies before the response is delivered.
        fixture.override = { request ->
            if (request.method == "PUT" && !dropped) { dropped = true; LoopbackReply(0, "", null, dropAfterCommit = true) } else null
        }
        on { live.library.setSaved(work, true) }
        await("the write settled from its committed receipt") { live.library.state.value.pending == 0 && fixture.committed.size == 1 }
        assertEquals(1, fixture.seen.count { it.method == "PUT" }, "the committed write is never replayed")
        assertTrue(fixture.seen.any { it.route.startsWith("/operations/") && it.method == "GET" })
    }

    @Test fun an_unstamped_contracted_503_is_retryable_unavailable_and_keeps_authority() {
        val (fixture, live) = start()
        live.signIn(fixture)
        val authority = on { live.account.verifiedSession.value }
        fixture.override = { LoopbackReply(503, """{"code":"unavailable","retryable":true}""", null) }
        on { live.library.setSaved(work, true); live.library.refresh() }
        await("a pending save and an unavailable section") {
            live.library.state.value.pending == 1 && live.library.state.value.section(LibrarySection.Saved).failure?.code == "unavailable"
        }
        assertEquals(authority, on { live.account.verifiedSession.value })
        assertEquals(actor, on { live.library.state.value.account })
    }

    @Test fun a_401_on_a_library_write_revalidates_authority_instead_of_becoming_an_unknown_transport_outcome() {
        val (fixture, live) = start()
        live.signIn(fixture)
        var rejected = false
        fixture.override = { request ->
            if (request.method == "PUT" && !rejected) { rejected = true; LoopbackReply(401, """{"code":"unauthenticated","retryable":false}""", null) } else null
        }
        val checksBefore = fixture.seen.count { it.route == "/native/identity/session" }
        on { live.library.setSaved(work, true) }
        // A genuine 401 fences the account and asks Identity again. A JDK streaming-mode failure on the PUT would
        // instead surface as status 0 ("transport"), which never revalidates the session.
        await("the 401 revalidated the session") { fixture.seen.count { it.route == "/native/identity/session" } > checksBefore }
        await("the durable save is delivered after re-admission") { live.library.state.value.pending == 0 && fixture.committed.size == 1 }
    }

    @Test fun a_success_without_the_real_stamp_is_refused_and_never_filled_from_the_expected_session() {
        val (fixture, live) = start()
        live.signIn(fixture)
        fixture.override = { request -> if (request.route.startsWith("/library/saved?")) LoopbackReply(200, """{"items":[],"next_cursor":null}""", null) else null }
        on { live.library.refresh(LibrarySection.Saved) }
        await("the unstamped success failed closed") { live.library.state.value.section(LibrarySection.Saved).failure != null }
    }

    @Test fun a_different_session_stamp_fences_authority() {
        val (fixture, live) = start()
        live.signIn(fixture)
        fixture.override = { request -> if (request.route.startsWith("/library/saved?")) LoopbackReply(200, """{"items":[],"next_cursor":null}""", "99999999-2222-4333-8444-555555555555") else null }
        on { live.library.refresh(LibrarySection.Saved) }
        await("authority fenced by the wrong stamp") { live.fenced() }
    }

    @Test fun logout_during_an_in_flight_read_discards_the_late_response() {
        val (fixture, live) = start()
        live.signIn(fixture)
        val release = CountDownLatch(1)
        fixture.gate = release
        on { live.library.refresh(LibrarySection.Saved) }
        await("the read reached the server") { fixture.seen.count { it.route.startsWith("/library/saved?") } >= 2 }
        on { live.shell.logout() }
        await("signed out and Library fenced") { live.library.state.value.account == null }
        release.countDown()
        runBlocking { delay(300) }
        assertNull(on { live.library.state.value.account })
        assertNull(on { live.account.verifiedSession.value })
        assertEquals(emptyList(), uncaught.map { it.toString() }, "cancelling an in-flight request must not escape the transport")
    }

    @Test fun an_unreachable_server_leaves_a_save_pending_and_never_reports_success() {
        val (fixture, live) = start()
        live.signIn(fixture)
        fixture.stop()
        on { live.library.setSaved(work, true) }
        await("the offline write is durable and pending") { live.library.state.value.pending == 1 }
        runBlocking { delay(500) }
        assertEquals(1, on { live.library.state.value.pending })
        assertEquals(actor, on { live.library.state.value.account }, "an outage is not a session reset")
    }

    @Test fun an_oversized_response_fails_closed_as_unavailable_and_is_never_accepted() {
        val (fixture, live) = start()
        live.signIn(fixture)
        fixture.override = { request ->
            if (request.route.startsWith("/library/saved?")) LoopbackReply(200, """{"items":[],"next_cursor":null,"pad":"${"x".repeat(70_000)}"}""", family) else null
        }
        on { live.library.refresh(LibrarySection.Saved) }
        await("the oversized read failed closed") { live.library.state.value.section(LibrarySection.Saved).failure?.code == "unavailable" }
        assertEquals(actor, on { live.library.state.value.account }, "a bounded-body refusal is not a session reset")
    }

    /** A connection that blocks in the response read until `disconnect()`, then fails the way the JDK's can when they race. */
    private class StuckConnection(url: java.net.URL, private val entered: CountDownLatch) : java.net.HttpURLConnection(url) {
        @Volatile var disconnected = false
        override fun connect() = Unit
        override fun disconnect() { disconnected = true }
        override fun usingProxy() = false
        override fun getOutputStream(): java.io.OutputStream = java.io.ByteArrayOutputStream()
        override fun getResponseCode(): Int {
            entered.countDown()
            while (!disconnected) Thread.sleep(5)
            throw RuntimeException(NullPointerException("Cannot invoke \"sun.net.www.http.HttpClient.getInputStream()\" because \"this.http\" is null"))
        }
    }

    @Test fun cancelling_a_blocked_read_whose_jdk_connection_then_throws_unchecked_delivers_nothing_and_escapes_nowhere() {
        val fixture = LoopbackServer(actor, family, work, access, refresh).also(fixtures::add)
        val entered = CountDownLatch(1)
        val stuck = CopyOnWriteArrayList<StuckConnection>()
        val http = on { PreviewNativeHttp(scope, fixture.origin) { uri -> StuckConnection(uri.toURL(), entered).also(stuck::add) } }
        val live = on { LiveEnrollmentOwners(scope, fixture.origin, http, desktopLibraryStore(":memory:")) }.also(owners::add)
        val email = assertIs<RegistrationResult.Accepted<RegistrationEmail>>(RegistrationEmail.parse("listener@example.test")).value
        on { assertTrue(live.shell.openLogin()); assertTrue(live.shell.login(email, "synthetic-test-password")) }
        assertTrue(entered.await(5, java.util.concurrent.TimeUnit.SECONDS), "the login request is blocked in its response read")
        on { live.shell.cancelLogin() }
        await("the cancelled request disconnected") { stuck.all { it.disconnected } }
        runBlocking { delay(400) }
        assertEquals(emptyList(), uncaught.map { it.toString() }, "an unchecked JDK failure after cancellation must not escape the transport")
        assertEquals(emptyList(), fixture.seen.filter { it.route.startsWith("/library") }, "a cancelled login admits nothing")
        assertNull(on { live.account.verifiedSession.value })
    }
}
