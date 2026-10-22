package com.cantos.theatre.application.identity

import com.cantos.theatre.adapters.identity.RetiredProviderRecords
import com.cantos.theatre.adapters.identity.SecureNativeCredentialJournal
import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.adapters.http.NativeAuthorizedHttpPort
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import com.cantos.theatre.domain.identity.IdentityV1Validation
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.identity.NativeSession
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.CredentialRead
import com.cantos.theatre.ports.CredentialStore
import com.cantos.theatre.ports.IdentitySignupAccepted
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeIdentityV1Api
import com.cantos.theatre.ports.NativeJournalRead
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.async
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.time.TimeSource

/** Production session owner AND production journal codec/readback, with deterministic OS/network ports.
 * Recreation uses the same durable port bytes; this is not a device/process-death or auth E2E proof. */
@OptIn(ExperimentalCoroutinesApi::class)
class NativeIdentityV1LifecycleTest {
    private val origin = "https://api.example"
    private val family = "00000000-0000-4000-8000-000000000001"
    private val otherFamily = "00000000-0000-4000-8000-000000000002"
    private val email = assertIs<RegistrationResult.Accepted<RegistrationEmail>>(RegistrationEmail.parse("reader@example.test")).value
    private val password = "synthetic-password-never-journaled"
    private fun apiOrigin(origin: String = this.origin) = ApiOrigin.parse(origin)
    private fun pair(letter: Char = 'a', actor: String = "reader-a", session: String = family,
        expires: Long = 220, refresh: Long = 700, absolute: Long = 1000) =
        assertIs<IdentityV1Validation.Accepted<NativeGrant>>(NativeGrant.restoreIdentityV1(
            "ci1_n_" + letter.toString().repeat(64), "ci1_r_" + letter.toString().repeat(64),
            actor, session, expires, refresh, absolute, 90)).value

    private class SecureStore(val events: MutableList<String>) : CredentialStore {
        var bytes: String? = null
        var failKind: String? = null
        var corruptKind: String? = null
        var failClear = false
        override fun read(): CredentialRead = bytes?.let(CredentialRead::Found) ?: CredentialRead.Missing
        override fun write(value: String): CredentialFailure? {
            val kind = listOf("active", "refreshing", "cleared").firstOrNull { value.contains("\"kind\":\"$it\"") } ?: "other"
            events += "write:$kind"
            if (failKind == kind) return CredentialFailure.Unavailable
            bytes = if (corruptKind == kind) "corrupt" else value
            return null
        }
        override fun clear(): CredentialFailure? {
            events += "delete"
            if (failClear) return CredentialFailure.Unavailable
            bytes = null
            return null
        }
    }
    private class Barrier(val events: MutableList<String>) : LogoutBarrier {
        var pending = false
        var failClear = false
        var failMark = false
        var unavailable = false
        override fun read() = if (unavailable) LogoutBarrierRead.Unavailable else if (pending) LogoutBarrierRead.Pending else LogoutBarrierRead.Clear
        override fun mark(): CredentialFailure? {
            events += "mark"
            if (failMark) return CredentialFailure.Unavailable
            pending = true
            return null
        }
        override fun clear(): CredentialFailure? {
            events += "unmark"
            if (failClear) return CredentialFailure.Unavailable
            pending = false
            return null
        }
    }
    private inner class Api(val events: MutableList<String>) : NativeIdentityV1Api {
        var remote = pair()
        var loginResult: IdentityV1Result<NativeGrant>? = null
        var loginGate: CompletableDeferred<IdentityV1Result<NativeGrant>>? = null
        var sessionResult: IdentityV1Result<NativeSession>? = null
        var sessionGate: CompletableDeferred<IdentityV1Result<NativeSession>>? = null
        var refreshResult: IdentityV1Result<NativeGrant>? = null
        var refreshGate: CompletableDeferred<IdentityV1Result<NativeGrant>>? = null
        var revokeFailure = false
        var logins = 0
        var sessions = 0
        var refreshes = 0
        val revoked = mutableListOf<NativeGrant>()
        override suspend fun signup(email: RegistrationEmail): IdentityV1Result<IdentitySignupAccepted> = error("Signup is outside the session lifecycle")
        override suspend fun login(email: RegistrationEmail, password: String): IdentityV1Result<NativeGrant> {
            assertEquals(this@NativeIdentityV1LifecycleTest.email.value, email.value)
            assertEquals(this@NativeIdentityV1LifecycleTest.password, password)
            events += "login"; logins++
            return loginGate?.let { withContext(NonCancellable) { it.await() } } ?: loginResult ?: IdentityV1Result.Accepted(remote)
        }
        override suspend fun session(previous: NativeGrant): IdentityV1Result<NativeSession> {
            events += "session"; sessions++
            return sessionGate?.let { withContext(NonCancellable) { it.await() } } ?: sessionResult ?: IdentityV1Result.Accepted(previous.view)
        }
        override suspend fun refresh(previous: NativeGrant): IdentityV1Result<NativeGrant> {
            events += "refresh"; refreshes++
            return refreshGate?.let { withContext(NonCancellable) { it.await() } } ?: refreshResult ?: IdentityV1Result.Accepted(pair('b', expires = 330, refresh = 810))
        }
        override suspend fun revokeFamily(previous: NativeGrant): IdentityV1Result<Unit> {
            events += "revoke"; revoked += previous
            return if (revokeFailure) IdentityV1Result.Refused(IdentityV1Failure.UnknownOutcome) else IdentityV1Result.Accepted(Unit)
        }
        override suspend fun revoke(previous: NativeGrant): IdentityV1Result<Unit> = error("Family cleanup must use the refresh credential")
    }
    private inner class Fixture(val scope: CoroutineScope, initial: NativeCredentialRecord? = null,
        val clock: TimeSource = TimeSource.Monotonic) {
        val events = mutableListOf<String>()
        val secure = SecureStore(events)
        val journal = SecureNativeCredentialJournal(secure, apiOrigin())
        val barrier = Barrier(events)
        val api = Api(events)
        var now = 100L
        init { initial?.let { assertNull(journal.write(it)) }; events.clear() }
        fun recreate() = NativeAccountController(scope, api, SecureNativeCredentialJournal(secure, apiOrigin()), barrier, clock) { now }
        val owner = recreate()
        fun login() = owner.login(email, password)
        fun stored() = assertIs<NativeJournalRead.Found>(journal.read()).record
    }

    @Test fun login_persists_and_reads_back_pair_then_waits_for_contract_admission() = runTest {
        val f = Fixture(backgroundScope)
        val admission = CompletableDeferred<IdentityV1Result<NativeSession>>()
        f.api.sessionGate = admission
        f.login(); runCurrent()
        assertNull(f.owner.verifiedSession.value)
        assertEquals(NativeAccountState.Pending, f.owner.state.value)
        assertEquals(pair().accessToken, assertIs<NativeCredentialRecord.Active>(f.stored()).grant.accessToken)
        assertTrue(f.barrier.pending)
        assertTrue(f.events.indexOf("write:active") < f.events.indexOf("session"))
        assertFalse(assertNotNull(f.secure.bytes).contains(password))
        admission.complete(IdentityV1Result.Accepted(pair().view)); runCurrent()
        assertEquals("reader-a", assertIs<NativeAccountState.SignedIn>(f.owner.state.value).actor)
        assertTrue(f.owner.accepts(assertNotNull(f.owner.verifiedSession.value)))
        assertFalse(f.barrier.pending)
        assertEquals(1, f.api.logins)
        f.owner.close()
    }

    @Test fun login_failures_have_no_invented_family_no_password_retry_and_keep_typed_reason() = runTest {
        for (failure in listOf(IdentityV1Failure.Unauthorized, IdentityV1Failure.UnknownOutcome,
            IdentityV1Failure.RateLimited(17), IdentityV1Failure.ServiceUnavailable, IdentityV1Failure.NotConfigured)) {
            val f = Fixture(backgroundScope); f.api.loginResult = IdentityV1Result.Refused(failure)
            f.login(); runCurrent()
            assertNull(f.owner.verifiedSession.value)
            assertEquals(failure, assertIs<NativeAccountState.Failed>(f.owner.state.value).identityFailure)
            assertEquals(1, f.api.logins); assertEquals(0, f.api.sessions); assertTrue(f.api.revoked.isEmpty())
            assertNull(f.secure.bytes); assertFalse(f.barrier.pending)
            f.owner.close()
        }
    }

    @Test fun login_persistence_or_readback_failure_never_publishes_and_cleans_known_family() = runTest {
        for (corrupt in listOf(false, true)) {
            val f = Fixture(backgroundScope)
            if (corrupt) f.secure.corruptKind = "active" else f.secure.failKind = "active"
            f.login(); runCurrent()
            assertNull(f.owner.verifiedSession.value); assertEquals(0, f.api.sessions)
            assertTrue(f.api.revoked.isNotEmpty()); assertTrue(f.api.revoked.all { it.view.session == family })
            if (corrupt) { assertTrue(f.barrier.pending); assertEquals("corrupt", f.secure.bytes) }
            f.owner.close()
        }
    }

    @Test fun login_session_outage_keeps_only_a_candidate_then_restore_rechecks_without_password() = runTest {
        val f = Fixture(backgroundScope); f.api.sessionResult = IdentityV1Result.Refused(IdentityV1Failure.ServiceUnavailable)
        f.login(); runCurrent()
        assertNull(f.owner.verifiedSession.value); assertIs<NativeCredentialRecord.Active>(f.stored())
        assertEquals(IdentityV1Failure.ServiceUnavailable, assertIs<NativeAccountState.Failed>(f.owner.state.value).identityFailure)
        assertTrue(f.api.revoked.isEmpty()); assertFalse(f.barrier.pending)
        f.api.sessionResult = null; f.owner.restore(); runCurrent()
        assertNotNull(f.owner.verifiedSession.value); assertEquals(1, f.api.logins); assertEquals(2, f.api.sessions)
        f.owner.close()
    }

    @Test fun session_mismatch_and_refusal_do_not_admit_login() = runTest {
        for (response in listOf(IdentityV1Result.Accepted(pair(actor = "reader-b").view),
            IdentityV1Result.Accepted(pair(absolute = 999).view), IdentityV1Result.Refused(IdentityV1Failure.Unauthorized))) {
            val f = Fixture(backgroundScope); f.api.sessionResult = response
            f.login(); runCurrent(); assertNull(f.owner.verifiedSession.value)
            assertTrue(f.api.revoked.isNotEmpty()); f.owner.close()
        }
    }

    @Test fun restore_is_api_confirmed_or_safely_refreshes_expired_access() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        f.api.sessionGate = CompletableDeferred()
        f.owner.restore(); runCurrent(); assertNull(f.owner.verifiedSession.value)
        f.api.sessionGate?.complete(IdentityV1Result.Accepted(pair().view)); runCurrent()
        assertNotNull(f.owner.verifiedSession.value); assertEquals(0, f.api.logins)
        f.owner.close()
        val expired = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        expired.now = 230; expired.owner.restore(); runCurrent()
        assertEquals(1, expired.api.refreshes); assertEquals(0, expired.api.sessions)
        assertEquals(pair('b', expires = 330, refresh = 810).accessToken, expired.owner.verifiedSession.value?.bearer)
        expired.owner.close()
    }

    @Test fun timer_and_private_waiters_share_refresh_and_waiter_cancellation_does_not_spend_twice() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        f.owner.restore(); runCurrent(); val before = assertNotNull(f.owner.verifiedSession.value)
        f.now = 210; val gate = CompletableDeferred<IdentityV1Result<NativeGrant>>(); f.api.refreshGate = gate
        advanceTimeBy(105_000); runCurrent()
        val first = async { f.owner.authorization(family) }; val second = async { f.owner.authorization(family) }
        runCurrent(); first.cancel(); assertEquals(1, f.api.refreshes)
        assertIs<NativeCredentialRecord.Refreshing>(f.stored())
        assertTrue(f.barrier.pending); assertTrue(f.events.indexOf("write:refreshing") < f.events.indexOf("refresh"))
        gate.complete(IdentityV1Result.Accepted(pair('b', expires = 330, refresh = 810))); runCurrent()
        val after = assertIs<NativeApiResult.Accepted<NativeVerifiedSession>>(second.await()).value
        assertEquals(before.intent, after.intent); assertEquals(1000L, after.session.absoluteExpiresAt)
        assertFalse(f.owner.accepts(before)); assertTrue(f.owner.belongsToCurrent(before.bearer, family))
        assertEquals(after.bearer, assertIs<NativeCredentialRecord.Active>(f.stored()).grant.accessToken)
        assertEquals(1, f.api.refreshes); assertFalse(f.barrier.pending); f.owner.close()
    }

    @Test fun all_refresh_failures_revoke_known_family_and_never_replay_a_spent_secret() = runTest {
        for (failure in listOf(IdentityV1Failure.UnknownOutcome, IdentityV1Failure.ServiceUnavailable,
            IdentityV1Failure.RateLimited(5), IdentityV1Failure.Unauthorized)) {
            val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
            f.owner.restore(); runCurrent(); f.now = 210
            f.api.refreshResult = IdentityV1Result.Refused(failure)
            val waiter = async { f.owner.authorization(family) }; runCurrent()
            assertEquals(failure, assertIs<NativeApiResult.Refused>(waiter.await()).identityFailure)
            assertNull(f.owner.verifiedSession.value); assertEquals(1, f.api.refreshes)
            assertEquals(listOf(family), f.api.revoked.map { it.view.session })
            assertIs<NativeApiResult.Refused>(f.owner.authorization(family)); assertEquals(1, f.api.refreshes)
            f.owner.close()
        }
    }

    @Test fun refreshing_write_failure_stops_before_effect_and_replacement_failure_revokes() = runTest {
        for (kind in listOf("refreshing", "active")) {
            val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
            f.owner.restore(); runCurrent(); f.now = 210; f.secure.failKind = kind
            val waiter = async { f.owner.authorization(family) }; runCurrent()
            assertIs<NativeApiResult.Refused>(waiter.await()); assertNull(f.owner.verifiedSession.value)
            assertEquals(if (kind == "refreshing") 0 else 1, f.api.refreshes)
            if (kind == "active") assertEquals(listOf(family), f.api.revoked.map { it.view.session })
            else assertTrue(f.barrier.pending)
            f.owner.close()
        }
    }

    @Test fun rotation_cannot_extend_absolute_deadline_or_change_actor_or_family() = runTest {
        for (invalid in listOf(pair('b', expires = 330, refresh = 810, absolute = 1001),
            pair('b', actor = "reader-b", expires = 330, refresh = 810),
            pair('b', session = otherFamily, expires = 330, refresh = 810))) {
            val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
            f.owner.restore(); runCurrent(); f.now = 210; f.api.refreshResult = IdentityV1Result.Accepted(invalid)
            val waiter = async { f.owner.authorization(family) }; runCurrent()
            assertIs<NativeApiResult.Refused>(waiter.await()); assertNull(f.owner.verifiedSession.value)
            assertTrue(f.api.revoked.all { it.view.session == family }); f.owner.close()
        }
    }

    @Test fun offline_logout_hides_synchronously_and_restart_retries_only_exact_family_cleanup() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        f.owner.restore(); runCurrent(); f.api.revokeFailure = true
        f.owner.logout(); assertNull(f.owner.verifiedSession.value); assertTrue(f.barrier.pending)
        runCurrent(); assertEquals(NativeAccountState.CleanupRequired, f.owner.state.value); f.owner.close()
        val restart = f.recreate(); restart.restore(); runCurrent()
        assertNull(restart.verifiedSession.value); assertEquals(0, f.api.refreshes)
        f.api.revokeFailure = false; restart.restore(); runCurrent()
        assertEquals(NativeAccountState.SignedOut, restart.state.value); assertNull(f.secure.bytes)
        assertFalse(f.barrier.pending); assertTrue(f.api.revoked.all { it.view.session == family }); restart.close()
    }

    @Test fun restart_at_refreshing_active_pending_and_cleared_never_replays_refresh_or_password() = runTest {
        for (record in listOf(NativeCredentialRecord.Refreshing(pair()), NativeCredentialRecord.Active(pair()), NativeCredentialRecord.Cleared)) {
            val f = Fixture(backgroundScope, record); f.barrier.pending = true
            f.owner.restore(); runCurrent()
            assertEquals(NativeAccountState.SignedOut, f.owner.state.value)
            assertEquals(0, f.api.sessions); assertEquals(0, f.api.refreshes); assertEquals(0, f.api.logins)
            assertEquals(if (record == NativeCredentialRecord.Cleared) 0 else 1, f.api.revoked.size)
            assertNull(f.secure.bytes); assertFalse(f.barrier.pending); f.owner.close()
        }
    }

    @Test fun restart_after_cleared_before_barrier_clear_finishes_without_network() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        f.owner.restore(); runCurrent(); f.barrier.failClear = true
        f.owner.logout(); runCurrent(); assertIs<NativeCredentialRecord.Cleared>(f.stored()); assertTrue(f.barrier.pending)
        f.owner.close(); f.barrier.failClear = false
        val restart = f.recreate(); restart.restore(); runCurrent()
        assertEquals(NativeAccountState.SignedOut, restart.state.value); assertEquals(1, f.api.revoked.size)
        assertNull(f.secure.bytes); restart.close()
    }

    @Test fun missing_corrupt_foreign_and_unavailable_barriers_cannot_be_erased_by_login() = runTest {
        for (raw in listOf<String?>(null, "corrupt", "{\"version\":2}", "ci1_r_" + "a".repeat(64))) {
            val f = Fixture(backgroundScope); f.secure.bytes = raw; f.barrier.pending = true
            f.login(); runCurrent(); assertEquals(NativeAccountState.CleanupRequired, f.owner.state.value)
            assertEquals(raw, f.secure.bytes); assertEquals(0, f.api.logins); assertTrue(f.barrier.pending)
            f.owner.close()
        }
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair())); f.barrier.unavailable = true
        f.owner.restore(); runCurrent(); assertNull(f.owner.verifiedSession.value); assertEquals(0, f.api.sessions)
        f.owner.close()
    }

    @Test fun retired_provider_records_and_unbound_hex_retire_locally_without_v1_disclosure() = runTest {
        for (raw in listOf(RetiredProviderRecords.active(origin), RetiredProviderRecords.refreshing(origin),
            RetiredProviderRecords.flow(origin), RetiredProviderRecords.legacyKind(origin), RetiredProviderRecords.cleared(origin),
            RetiredProviderRecords.activeWithoutAuthTime(origin), RetiredProviderRecords.opaqueToken())) {
            val f = Fixture(backgroundScope)
            f.secure.bytes = raw
            assertEquals(NativeCredentialRecord.Retired, assertIs<NativeJournalRead.Found>(f.journal.read()).record)
            f.owner.restore(); runCurrent()
            assertEquals(NativeAccountState.SignedOut, f.owner.state.value); assertNull(f.secure.bytes)
            assertTrue(f.api.revoked.isEmpty()); assertEquals(0, f.api.sessions); assertEquals(0, f.api.refreshes)
            assertEquals(0, f.api.logins)
            f.owner.close()
        }
    }

    @Test fun retired_record_blocks_nothing_and_an_explicit_login_after_it_starts_a_fresh_v1_family() = runTest {
        val f = Fixture(backgroundScope)
        f.secure.bytes = RetiredProviderRecords.active(origin)
        f.login(); runCurrent()
        assertEquals(1, f.api.logins); assertTrue(f.api.revoked.isEmpty())
        assertEquals(pair().accessToken, assertIs<NativeCredentialRecord.Active>(f.stored()).grant.accessToken)
        f.owner.close()
    }

    @Test fun journal_binding_rejects_changed_origin_protocol_ambiguous_and_mixed_credentials() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        val valid = assertNotNull(f.secure.bytes)
        assertEquals(NativeJournalRead.Failed(CredentialFailure.Corrupt), SecureNativeCredentialJournal(f.secure, apiOrigin("https://other.example")).read())
        for (raw in listOf(valid.replace("identity-v1", "provider"), valid.replace("\"version\":2", "\"version\":1"),
            valid.replace("ci1_r_", "cr1_"), valid.replace("ci1_n_", "ci1_s_"), valid + " ",
            valid.replace("\"version\":2", "\"version\":2,\"version\":2"), valid.replace("\"authenticated_at\":90", "\"authenticated_at\":null"))) {
            f.secure.bytes = raw; f.barrier.pending = true; f.owner.restore(); runCurrent()
            assertNull(f.owner.verifiedSession.value); assertEquals(raw, f.secure.bytes)
            assertTrue(f.barrier.pending); assertEquals(0, f.api.sessions)
        }
        f.owner.close()
    }

    @Test fun cancelled_late_login_A_cleans_only_orphan_A_and_never_replaces_B() = runTest {
        val f = Fixture(backgroundScope); val late = CompletableDeferred<IdentityV1Result<NativeGrant>>()
        f.api.loginGate = late; f.login(); runCurrent()
        f.owner.cancelLogin(); runCurrent()
        f.api.loginGate = null; f.api.remote = pair('c', "reader-b", otherFamily)
        f.login(); runCurrent(); val b = assertNotNull(f.owner.verifiedSession.value)
        late.complete(IdentityV1Result.Accepted(pair())); runCurrent()
        assertEquals("reader-b", f.owner.verifiedSession.value?.session?.actor)
        assertEquals(otherFamily, assertIs<NativeCredentialRecord.Active>(f.stored()).grant.view.session)
        assertTrue(f.owner.accepts(b)); assertEquals(listOf(family), f.api.revoked.map { it.view.session })
        assertFalse(f.barrier.pending); f.owner.close()
    }

    @Test fun cancelled_login_without_pair_is_signedout_with_unknown_remote_effect() = runTest {
        val f = Fixture(backgroundScope); val late = CompletableDeferred<IdentityV1Result<NativeGrant>>()
        f.api.loginGate = late; f.login(); runCurrent(); f.owner.cancelLogin(); runCurrent()
        late.complete(IdentityV1Result.Refused(IdentityV1Failure.UnknownOutcome)); runCurrent()
        assertEquals(NativeAccountState.SignedOut, f.owner.state.value)
        assertNull(f.owner.verifiedSession.value); assertTrue(f.api.revoked.isEmpty()); assertEquals(1, f.api.logins)
        f.owner.close()
    }

    @Test fun late_refresh_A_cannot_replace_B_or_revoke_B() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        f.owner.restore(); runCurrent(); f.now = 210
        val late = CompletableDeferred<IdentityV1Result<NativeGrant>>(); f.api.refreshGate = late
        val waiter = async { f.owner.authorization(family) }; runCurrent()
        f.owner.logout(); runCurrent(); f.api.remote = pair('c', "reader-b", otherFamily, expires = 330, refresh = 810)
        f.login(); runCurrent()
        late.complete(IdentityV1Result.Accepted(pair('b', expires = 330, refresh = 810))); runCurrent()
        assertIs<NativeApiResult.Refused>(waiter.await())
        assertEquals(otherFamily, assertIs<NativeCredentialRecord.Active>(f.stored()).grant.view.session)
        assertEquals("reader-b", f.owner.verifiedSession.value?.session?.actor)
        assertTrue(f.api.revoked.all { it.view.session == family }); f.owner.close()
    }

    @Test fun revalidation_outage_hides_authority_without_revoking_candidate() = runTest {
        for (failure in listOf(IdentityV1Failure.RateLimited(8), IdentityV1Failure.ServiceUnavailable)) {
            val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
            f.owner.restore(); runCurrent(); f.api.sessionResult = IdentityV1Result.Refused(failure)
            f.owner.revalidate(family); assertNull(f.owner.verifiedSession.value); runCurrent()
            assertTrue(f.api.revoked.isEmpty()); assertIs<NativeCredentialRecord.Active>(f.stored())
            assertEquals(failure, assertIs<NativeAccountState.Failed>(f.owner.state.value).identityFailure)
            f.api.sessionResult = null; f.owner.restore(); runCurrent(); assertNotNull(f.owner.verifiedSession.value)
            f.owner.close()
        }
    }
    @Test fun known_login_pair_is_retained_for_restart_cleanup_when_active_write_and_revoke_fail() = runTest {
        val f = Fixture(backgroundScope); f.secure.failKind = "active"; f.api.revokeFailure = true
        f.login(); runCurrent()
        assertNull(f.owner.verifiedSession.value); assertTrue(f.barrier.pending)
        assertIs<NativeCredentialRecord.Refreshing>(f.stored()); f.owner.close()
        f.secure.failKind = null; f.api.revokeFailure = false
        val restart = f.recreate(); restart.restore(); runCurrent()
        assertEquals(NativeAccountState.SignedOut, restart.state.value)
        assertEquals(1, f.api.logins); assertEquals(0, f.api.refreshes)
        assertTrue(f.api.revoked.all { it.view.session == family }); assertNull(f.secure.bytes); restart.close()
    }

    @Test fun refresh_pair_committed_with_corrupt_readback_keeps_barrier_and_revokes_known_old_family() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        f.owner.restore(); runCurrent(); f.now = 210; f.secure.corruptKind = "active"
        val waiter = async { f.owner.authorization(family) }; runCurrent()
        assertIs<NativeApiResult.Refused>(waiter.await()); assertNull(f.owner.verifiedSession.value)
        assertTrue(f.barrier.pending); assertEquals("corrupt", f.secure.bytes)
        assertTrue(f.api.revoked.all { it.refreshToken == pair().refreshToken })
        assertEquals(1, f.api.refreshes); f.owner.close()
        val restart = f.recreate(); restart.restore(); runCurrent()
        assertEquals(NativeAccountState.CleanupRequired, restart.state.value)
        assertEquals("corrupt", f.secure.bytes); assertEquals(1, f.api.refreshes); restart.close()
    }

    @Test fun interrupted_first_login_before_dispatch_recovers_without_password_or_family() = runTest {
        val f = Fixture(backgroundScope); f.login()
        assertTrue(f.barrier.pending); assertIs<NativeCredentialRecord.Cleared>(f.stored())
        f.owner.close()
        val restart = f.recreate(); restart.restore(); runCurrent()
        assertEquals(NativeAccountState.SignedOut, restart.state.value)
        assertEquals(0, f.api.logins); assertTrue(f.api.revoked.isEmpty()); assertNull(f.secure.bytes); restart.close()
    }

    @Test fun failed_denial_barrier_prevents_login_effect_and_keeps_old_candidate() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        f.barrier.failMark = true; f.login(); runCurrent()
        assertEquals(0, f.api.logins); assertNull(f.owner.verifiedSession.value)
        assertIs<NativeCredentialRecord.Active>(f.stored()); f.owner.close()
    }

    @Test fun provider_envelope_at_changed_origin_is_not_even_locally_retired() = runTest {
        val f = Fixture(backgroundScope)
        f.secure.bytes = RetiredProviderRecords.active("https://old.example")
        val bytes = f.secure.bytes; f.barrier.pending = true; f.login(); runCurrent()
        assertEquals(bytes, f.secure.bytes); assertTrue(f.barrier.pending)
        assertEquals(NativeAccountState.CleanupRequired, f.owner.state.value)
        assertEquals(0, f.api.logins); assertTrue(f.api.revoked.isEmpty()); f.owner.close()
    }

    @Test fun late_restore_session_A_cannot_admit_over_explicit_login_B() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        val late = CompletableDeferred<IdentityV1Result<NativeSession>>(); f.api.sessionGate = late
        f.owner.restore(); runCurrent(); f.api.sessionGate = null
        f.api.remote = pair('c', "reader-b", otherFamily); f.login(); runCurrent()
        assertEquals("reader-b", f.owner.verifiedSession.value?.session?.actor)
        late.complete(IdentityV1Result.Accepted(pair().view)); runCurrent()
        assertEquals("reader-b", f.owner.verifiedSession.value?.session?.actor)
        assertEquals(otherFamily, assertIs<NativeCredentialRecord.Active>(f.stored()).grant.view.session)
        assertTrue(f.api.revoked.all { it.view.session == family }); f.owner.close()
    }

    @Test fun expired_absolute_authority_is_cleanup_only_and_cannot_refresh() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        f.now = 1001; f.owner.restore(); runCurrent()
        assertNull(f.owner.verifiedSession.value); assertEquals(0, f.api.refreshes); assertEquals(0, f.api.sessions)
        assertEquals(listOf(family), f.api.revoked.map { it.view.session }); f.owner.close()
    }

    @Test fun wall_clock_rollback_cannot_extend_absolute_authority() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair(expires = 220, refresh = 220, absolute = 220)), testScheduler.timeSource)
        f.owner.restore(); runCurrent()
        val lease = assertNotNull(f.owner.verifiedSession.value)
        f.now = 115 // The elapsed 120 seconds still end authority after a device-clock rollback.
        advanceTimeBy(120_000); runCurrent()
        assertFalse(f.owner.accepts(lease))
        assertNull(f.owner.verifiedSession.value)
        assertEquals(0, f.api.refreshes)
        assertEquals(listOf(family), f.api.revoked.map { it.view.session })
        f.owner.close()
    }

    @Test fun wall_clock_rollback_does_not_disable_timer_rotation() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()), testScheduler.timeSource)
        // At elapsed time 205 the server's 120-second access deadline is 325.
        f.api.refreshResult = IdentityV1Result.Accepted(pair('b', expires = 325, refresh = 800))
        f.owner.restore(); runCurrent()
        f.now = 200 // 105 elapsed seconds means renewal is due, despite five seconds of rollback.
        advanceTimeBy(105_000); runCurrent()
        assertEquals(1, f.api.refreshes)
        assertEquals(pair('b', expires = 325, refresh = 800).accessToken,
            assertNotNull(f.owner.verifiedSession.value).bearer)
        f.owner.close()
    }

    @Test fun rotation_and_revalidation_preserve_elapsed_absolute_ceiling() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair(expires = 200, refresh = 220, absolute = 220)),
            testScheduler.timeSource)
        f.api.refreshResult = IdentityV1Result.Accepted(pair('b', expires = 220, refresh = 220, absolute = 220))
        f.owner.restore(); runCurrent()
        f.now = 180; advanceTimeBy(85_000); runCurrent()
        assertEquals(1, f.api.refreshes)
        f.now = 190; advanceTimeBy(20_000); runCurrent()
        f.owner.revalidate(family); runCurrent()
        val lease = assertNotNull(f.owner.verifiedSession.value)
        assertEquals(220L, lease.session.absoluteExpiresAt)
        f.now = 100; advanceTimeBy(15_000); runCurrent()
        assertFalse(f.owner.accepts(lease)); assertNull(f.owner.verifiedSession.value)
        assertEquals(1, f.api.refreshes)
        assertEquals(listOf(family), f.api.revoked.map { it.view.session })
        f.owner.close()
    }

    @Test fun business_authorization_failure_preserves_retry_after_without_dispatch_or_mutation_replay() = runTest {
        for (failure in listOf(IdentityV1Failure.RateLimited(11), IdentityV1Failure.ServiceUnavailable)) {
            val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
            f.owner.restore(); runCurrent(); f.now = 210; f.api.refreshResult = IdentityV1Result.Refused(failure)
            var sends = 0
            val http = NativeAuthorizedHttpPort(backgroundScope, ApiOrigin.parse(origin), f.owner, object : NativeHttpPort {
                override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
                    sends++; error("No business mutation may dispatch after failed authority acquisition")
                }
            })
            var response: NativeHttpResponse? = null
            http.send(NativeHttpRequest("PUT", "$origin/api/v1/theatre/library/saved/$otherFamily", "{}", pair().accessToken, family)) { response = it }
            runCurrent()
            assertEquals(if (failure is IdentityV1Failure.RateLimited) 429 else 503, response?.status)
            assertEquals(if (failure is IdentityV1Failure.RateLimited) "11" else null, response?.retryAfter)
            assertEquals(0, sends); assertEquals(1, f.api.refreshes); assertNull(f.owner.verifiedSession.value)
            f.owner.close()
        }
    }

    @Test fun business_401_and_409_are_returned_once_without_blind_write_replay() = runTest {
        for (status in listOf(401, 409)) {
            val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
            f.owner.restore(); runCurrent()
            var sends = 0
            val http = NativeAuthorizedHttpPort(backgroundScope, ApiOrigin.parse(origin), f.owner, object : NativeHttpPort {
                override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
                    sends++
                    completion(NativeHttpResponse(status, "application/json", "{}", null, if (status == 409) otherFamily else family))
                    return object : NativeRequestHandle { override fun cancel() = Unit }
                }
            })
            var response: NativeHttpResponse? = null
            http.send(NativeHttpRequest("PUT", "$origin/api/v1/theatre/library/saved/$otherFamily", "{}", pair().accessToken, family)) { response = it }
            runCurrent(); assertEquals(status, response?.status); assertEquals(1, sends); assertEquals(0, f.api.refreshes)
            f.owner.close()
        }
    }

    @Test fun restore_requires_the_exact_actor_family_and_deadline_from_the_api() = runTest {
        for (view in listOf(pair(actor = "reader-b").view, pair(session = otherFamily).view, pair(expires = 221).view)) {
            val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
            f.api.sessionResult = IdentityV1Result.Accepted(view)
            f.owner.restore(); runCurrent()
            assertNull(f.owner.verifiedSession.value)
            assertEquals(NativeAccountState.Failed(NativeAccountFailure.Protocol), f.owner.state.value)
            assertEquals(listOf(family), f.api.revoked.map { it.view.session })
            f.owner.close()
        }
    }

    @Test fun logout_hides_and_marks_synchronously_then_a_late_refresh_cannot_restore_authority() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        f.owner.restore(); runCurrent(); f.now = 210
        val late = CompletableDeferred<IdentityV1Result<NativeGrant>>(); f.api.refreshGate = late
        val request = async { f.owner.authorization(family) }; runCurrent()
        f.owner.logout(); assertNull(f.owner.verifiedSession.value); assertTrue(f.barrier.pending)
        runCurrent(); assertEquals(NativeAccountState.SignedOut, f.owner.state.value)
        late.complete(IdentityV1Result.Accepted(pair('b', expires = 330, refresh = 810))); runCurrent()
        assertIs<NativeApiResult.Refused>(request.await()); assertNull(f.owner.verifiedSession.value)
        assertNull(f.secure.bytes); assertTrue(f.api.revoked.all { it.view.session == family }); f.owner.close()
    }

    @Test fun rejected_authority_is_quarantined_synchronously_before_deferred_reconfirmation() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(pair()))
        f.owner.restore(); runCurrent()
        val old = assertNotNull(f.owner.verifiedSession.value)
        val delayed = CompletableDeferred<IdentityV1Result<NativeSession>>(); f.api.sessionGate = delayed
        f.owner.revalidate(family)
        assertNull(f.owner.verifiedSession.value)
        assertFalse(f.owner.accepts(old)); assertFalse(f.owner.belongsToCurrent(old.bearer, family))
        assertIs<NativeApiResult.Refused>(f.owner.authorization(family))
        assertEquals(NativeAccountState.Pending, f.owner.state.value)
        runCurrent(); delayed.complete(IdentityV1Result.Accepted(pair().view)); runCurrent()
        assertEquals("reader-a", f.owner.verifiedSession.value?.session?.actor)
        assertTrue(f.owner.accepts(assertNotNull(f.owner.verifiedSession.value)))
        f.owner.close()
    }

    @Test fun access_at_the_original_authority_ceiling_does_not_spin_refresh_before_expiry() = runTest {
        val finalPair = pair(expires = 110, refresh = 110, absolute = 110)
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(finalPair))
        f.owner.restore(); runCurrent()
        assertEquals(0, f.api.refreshes); assertEquals(1, f.api.sessions)
        f.now = 109
        assertIs<NativeApiResult.Accepted<NativeVerifiedSession>>(f.owner.authorization(family))
        assertEquals(0, f.api.refreshes)
        f.now = 110; advanceTimeBy(10_000); runCurrent()
        assertNull(f.owner.verifiedSession.value); assertEquals(0, f.api.refreshes)
        assertEquals(listOf(family), f.api.revoked.map { it.view.session })
        f.owner.close()
    }
}
