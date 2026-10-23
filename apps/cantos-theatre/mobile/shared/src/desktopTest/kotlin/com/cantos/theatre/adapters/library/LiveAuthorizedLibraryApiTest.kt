package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.NativeAuthorizedHttpPort
import com.cantos.theatre.adapters.http.NativeIdentityV1Client
import com.cantos.theatre.app.NativeSessionRuntime
import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.domain.library.BookmarkSync
import com.cantos.theatre.domain.library.LibrarySection
import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.StreamingMediaEngine
import java.nio.file.Files
import java.util.UUID
import java.util.concurrent.Executors
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.emptyFlow
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
import kotlin.test.assertTrue

/** Explicit lane in theatre_business_journey: real Identity login/session admission and TCP HTTP,
 * production authorized adapter/client/runtime and real SQLite. Only secure storage and audio are doubles. */
class LiveAuthorizedLibraryApiTest {
    private class NoAudio : StreamingMediaEngine {
        override val events = emptyFlow<PlaybackObservation>()
        override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = Unit
        override suspend fun prepare(command: PlaybackEngineCommand) = Unit
        override suspend fun start(command: PlaybackEngineCommand) = Unit
        override suspend fun pause(command: PlaybackEngineCommand) = Unit
        override suspend fun seek(command: PlaybackEngineCommand) = Unit
        override suspend fun setSpeed(command: PlaybackEngineCommand) = Unit
        override fun close() = Unit
    }

    private fun await(what: String, predicate: () -> Boolean) = runBlocking {
        val deadline = System.nanoTime() + 40_000_000_000
        while (!predicate()) {
            check(System.nanoTime() < deadline) { "timed out: $what" }
            delay(25)
        }
    }

    @Test fun real_identity_authorized_http_outage_and_lost_commit_response_recover_in_one_runtime() {
        val text = System.getenv("CANTOS_AUTHORIZED_API_ORIGIN")
        Assume.assumeTrue("explicit real Identity journey only", text != null)
        val origin = ApiOrigin.parse(requireNotNull(text))
        assertTrue(origin.toString().startsWith("http://127.0.0.1:"))
        val control = java.io.File(requireNotNull(System.getenv("CANTOS_AUTHORIZED_CONTROL")))
        val raw = Json.parseToJsonElement(requireNotNull(System.getenv("CANTOS_LIVE_AUDIO"))).jsonObject
        fun field(key: String) = raw.getValue(key).jsonPrimitive.content
        val audio = assertNotNull(AudioIdentity.of(field("work_id"), field("unit_id"), field("publication_id"),
            field("rendition_id"), field("sha256"), field("duration_ms").toLong()))
        val directory = Files.createTempDirectory("cantos-live-authorized").toFile()
        val dispatcher = Executors.newSingleThreadExecutor().asCoroutineDispatcher()
        val scope = CoroutineScope(SupervisorJob() + dispatcher)
        fun <T> on(block: () -> T): T = runBlocking { withContext(dispatcher) { block() } }
        val http = LiveBookmarkApiTest.LiveHttp()
        val account = NativeAccountController(scope, NativeIdentityV1Client(origin, http) { System.currentTimeMillis() / 1000 },
            object : NativeCredentialJournal {
                private var value: NativeCredentialRecord? = null
                override fun read() = value?.let(NativeJournalRead::Found) ?: NativeJournalRead.Missing
                override fun write(record: NativeCredentialRecord): CredentialFailure? { value = record; return null }
                override fun clear(): CredentialFailure? { value = null; return null }
            }, object : LogoutBarrier {
                override fun read() = LogoutBarrierRead.Clear
                override fun mark(): CredentialFailure? = null
                override fun clear(): CredentialFailure? = null
            }) { System.currentTimeMillis() / 1000 }
        val library = nativeLibraryController(scope,
            NativeLibraryClient(origin, NativeAuthorizedHttpPort(scope, origin, account, http)),
            desktopLibraryStore(directory.resolve("library.sqlite").absolutePath), { UUID.randomUUID().toString() },
            onSessionInvalidated = account::revalidate,
            ownsAuthorization = { account.belongsToCurrent(it.bearer, it.session) && account.verifiedSession.value?.session?.actor == it.account },
            nowMs = { System.currentTimeMillis() })
        val runtime = on { NativeSessionRuntime(scope, { NoAudio() }, object : PlaybackAccessPort {
            override suspend fun manifest(publicationId: String) = PlaybackAccessResult.Accepted(audio)
            override suspend fun authorize(identity: AudioIdentity) = PlaybackAccessResult.Accepted(
                PlaybackGrant(PlaybackLocation("https://media.invalid/not-played"), Long.MAX_VALUE))
        }, account, library, { UUID.randomUUID().toString() }, { System.currentTimeMillis() }) }
        try {
            on { account.login(assertIs<RegistrationResult.Accepted<RegistrationEmail>>(
                RegistrationEmail.parse("owner@example.com")).value, "a correct horse battery") }
            await("real native login and four admitted Library pages") {
                account.verifiedSession.value != null && LibrarySection.entries.all { library.state.value.section(it).loaded }
            }
            val authority = assertNotNull(account.verifiedSession.value)
            assertTrue(http.replies.any { it.first == 200 && it.second == authority.session.session })
            on { runtime.listening.restore(audio, 10) }
            await("paused local restore") { runtime.listening.playback.state.value.session?.source?.audio == audio }
            val player = on { runtime.listening.playback.state.value }
            http.loseMutationResponses = 1
            val mark = on { assertNotNull(library.createBookmark(audio, 30)) }
            await("committed response reconciled through its real receipt") {
                library.state.value.pending == 0 && library.state.value.bookmarks.any { it.id == mark && it.sync == BookmarkSync.Synced }
            }
            assertEquals(1, http.sent.count { it.startsWith("PUT /library/bookmarks/") })
            assertTrue(http.sent.any { it.startsWith("GET /operations/") })

            // Test-only file handshake stops/restarts the REAL Identity process; no fabricated HTTP 503.
            control.resolve("stop-requested").writeText("stop")
            await("Identity stopped") { control.resolve("stopped").isFile }
            on { library.setSaved(audio.workId, true); library.refresh() }
            await("real pre-admission 503") {
                library.state.value.section(LibrarySection.Saved).failure?.code == "unavailable" && library.state.value.pending == 1
            }
            assertEquals(authority, account.verifiedSession.value)
            assertEquals(player, on { runtime.listening.playback.state.value })
            assertTrue(http.replies.any { it.first == 503 && it.second == null })
            control.resolve("resume-requested").writeText("resume")
            await("Identity restarted") { control.resolve("restored").isFile }
            on { runtime.foregrounded(); runtime.networkRestored(); runtime.foregrounded() }
            await("durable Saved delivered") { library.state.value.pending == 0 }
            assertEquals(authority.session.session, account.verifiedSession.value?.session?.session)
            assertEquals(player, on { runtime.listening.playback.state.value })
            assertEquals(1, library.state.value.bookmarks.count { it.id == mark })
            println("LIVE-AUTHORIZED-LIBRARY-OK requests=${http.sent.size}")
        } finally {
            on { runtime.close() }
            scope.cancel(); dispatcher.close(); directory.deleteRecursively()
        }
    }
}
