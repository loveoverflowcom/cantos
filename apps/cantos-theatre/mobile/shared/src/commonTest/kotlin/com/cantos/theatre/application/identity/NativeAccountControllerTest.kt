package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeCallback
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.IdentityCrypto
import com.cantos.theatre.ports.LogoutBarrier
import com.cantos.theatre.ports.LogoutBarrierRead
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeFlow
import com.cantos.theatre.ports.NativeIdentityApi
import com.cantos.theatre.ports.NativeJournalRead
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.ports.SystemBrowser
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

@OptIn(ExperimentalCoroutinesApi::class)
class NativeAccountControllerTest {
    private val actor = "actor-A"
    private val family = "00000000-0000-4000-8000-000000000001"
    private fun pair(access: Char = 'a', refresh: Char = 'b', expires: Long = 200, actor: String = this.actor,
        session: String = family, absolute: Long = 900): NativeGrant = requireNotNull(NativeGrant.restore(
        "ca1_n_" + access.toString().repeat(64), "cr1_" + refresh.toString().repeat(64), actor, session, expires, 600, absolute, 90))
    private class Journal(val events: MutableList<String>) : NativeCredentialJournal {
        var record: NativeCredentialRecord? = null
        var readFailure: CredentialFailure? = null
        var failKind: String? = null
        var clearFailure = false
        var commitThenFailReadKind: String? = null
        override fun read(): NativeJournalRead = readFailure?.let { NativeJournalRead.Failed(it) }
            ?: record?.let { NativeJournalRead.Found(it) } ?: NativeJournalRead.Missing
        override fun write(record: NativeCredentialRecord): CredentialFailure? {
            val kind = when (record) {
                is NativeCredentialRecord.Active -> "active"
                is NativeCredentialRecord.Refreshing -> "refreshing"
                is NativeCredentialRecord.Flow -> "flow"
                is NativeCredentialRecord.Legacy -> "legacy"
                NativeCredentialRecord.Cleared -> "cleared"
            }
            events += "write:$kind"
            if (failKind == kind) return CredentialFailure.Unavailable
            this.record = record
            if (commitThenFailReadKind == kind) { readFailure = CredentialFailure.Corrupt; return CredentialFailure.Corrupt }
            return null
        }
        override fun clear(): CredentialFailure? {
            events += "delete"
            if (clearFailure) return CredentialFailure.Denied
            record = null
            return null
        }
    }
    private class Barrier(val events: MutableList<String>) : LogoutBarrier {
        var pending = false
        var unavailable = false
        var markFailure = false
        var clearFailure = false
        override fun read() = if (unavailable) LogoutBarrierRead.Unavailable else if (pending) LogoutBarrierRead.Pending else LogoutBarrierRead.Clear
        override fun mark(): CredentialFailure? { events += "mark"; if (markFailure) return CredentialFailure.Denied; pending = true; return null }
        override fun clear(): CredentialFailure? { events += "unmark"; if (clearFailure) return CredentialFailure.Unavailable; pending = false; return null }
    }
    private class Crypto : IdentityCrypto {
        var sequence = 0
        override fun secret() = requireNotNull(LoginSecret.parse(('a' + sequence++).toString().repeat(64)))
        override fun challenge(verifier: LoginSecret) = "x".repeat(43)
    }
    private class Browser : SystemBrowser {
        var opened: String? = null
        override fun open(url: String): Boolean { opened = url; return true }
        override fun cancel() { opened = null }
    }
    private inner class Api(val events: MutableList<String>) : NativeIdentityApi {
        var refreshes = 0
        var exchanges = 0
        var sessions = 0
        var state = ""
        var sessionValue = pair().view
        var sessionResult: CompletableDeferred<NativeApiResult<NativeSession>>? = null
        var refreshResult: CompletableDeferred<NativeApiResult<NativeGrant>>? = null
        var exchangeResult: NativeApiResult<NativeGrant> = NativeApiResult.Accepted(pair())
        var revokeFails = false
        val revoked = mutableListOf<Pair<String, String>>()
        val cancelled = mutableListOf<String>()
        override suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow> {
            this.state = state; events += "begin"
            return NativeApiResult.Accepted(NativeFlow(family,
                "https://idm.example/ui/oauth2?client_id=cantos-theatre-native&response_type=code&scope=openid&redirect_uri=com.cantos.theatre%3A%2Foauth2%2Fcallback&state=$state&nonce=$nonce&code_challenge_method=S256&code_challenge=$challenge"))
        }
        override suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeGrant> {
            exchanges++; events += "exchange"
            assertEquals(this.state, state); assertEquals("code", code)
            assertIs<NativeCredentialRecord.Flow>(journalForCheck?.record)
            return exchangeResult
        }
        var journalForCheck: Journal? = null
        override suspend fun session(credential: String): NativeApiResult<NativeSession> {
            sessions++; events += "session"
            return sessionResult?.let { withContext(NonCancellable) { it.await() } } ?: NativeApiResult.Accepted(sessionValue)
        }
        override suspend fun refresh(refreshToken: String, session: String): NativeApiResult<NativeGrant> {
            refreshes++; events += "refresh"
            assertIs<NativeCredentialRecord.Refreshing>(journalForCheck?.record)
            return refreshResult?.let { withContext(NonCancellable) { it.await() } } ?: NativeApiResult.Accepted(pair('c', 'd', 300))
        }
        override suspend fun revokeFamily(refreshToken: String, session: String): NativeApiResult<Unit> {
            events += "revoke"; revoked += refreshToken to session
            return if (revokeFails) NativeApiResult.Refused(NativeApiFailure.UnknownOutcome) else NativeApiResult.Accepted(Unit)
        }
        override suspend fun revoke(credential: String): NativeApiResult<Unit> { events += "legacy-revoke"; return NativeApiResult.Accepted(Unit) }
        override suspend fun cancel(flow: String, state: String, verifier: String): NativeApiResult<Unit> {
            events += "cancel"; cancelled += flow
            return if (revokeFails) NativeApiResult.Refused(NativeApiFailure.UnknownOutcome) else NativeApiResult.Accepted(Unit)
        }
        override suspend fun saved(credential: String, session: String) = NativeApiResult.Accepted(3)
    }
    private inner class Fixture(scope: CoroutineScope, initial: NativeCredentialRecord? = NativeCredentialRecord.Active(pair())) {
        val events = mutableListOf<String>()
        val journal = Journal(events).also { it.record = initial }
        val barrier = Barrier(events)
        val api = Api(events).also { it.journalForCheck = journal }
        val browser = Browser()
        var time = 100L
        fun recreate(scope: CoroutineScope) = NativeAccountController(scope,
            requireNotNull(NativeClientConfiguration.create("https://api.example", "https://idm.example/oauth2/openid/cantos-theatre-native")),
            api, Crypto(), browser, journal, barrier) { time }
        val owner = recreate(scope)
        fun callback() = owner.callback(NativeCallback("com.cantos.theatre:/oauth2/callback", api.state, "code", false))
    }

    @Test fun restore_requires_exact_api_confirmation_before_private_authority() = runTest {
        val f = Fixture(backgroundScope)
        f.owner.restore(); assertNull(f.owner.verifiedSession.value)
        runCurrent()
        assertEquals(actor, f.owner.verifiedSession.value?.session?.actor)
        assertEquals(1, f.api.sessions)
        assertEquals("NativeVerifiedSession([redacted])", f.owner.verifiedSession.value.toString())
        f.owner.close()
    }
    @Test fun wrong_actor_family_or_deadline_cannot_restore_local_record() = runTest {
        for (view in listOf(NativeSession("actor-B", family, 200, absoluteExpiresAt = 900, authenticatedAt = 90),
            NativeSession(actor, "00000000-0000-4000-8000-000000000002", 200, absoluteExpiresAt = 900, authenticatedAt = 90),
            NativeSession(actor, family, 201, absoluteExpiresAt = 900, authenticatedAt = 90))) {
            val f = Fixture(backgroundScope); f.api.sessionValue = view
            f.owner.restore(); runCurrent()
            assertNull(f.owner.verifiedSession.value)
            assertEquals(NativeAccountState.Failed(NativeAccountFailure.Protocol), f.owner.state.value)
            assertEquals(listOf(pair().refreshToken to family), f.api.revoked)
            f.owner.close()
        }
    }
    @Test fun concurrent_waiters_spend_once_and_cancelled_waiter_does_not_cancel_rotation() = runTest {
        val f = Fixture(backgroundScope); f.owner.restore(); runCurrent()
        val old = assertNotNull(f.owner.verifiedSession.value)
        f.time = 190
        val response = CompletableDeferred<NativeApiResult<NativeGrant>>(); f.api.refreshResult = response
        val first = async { f.owner.authorization(family) }; val second = async { f.owner.authorization(family) }
        runCurrent(); assertEquals(1, f.api.refreshes)
        first.cancel(); response.complete(NativeApiResult.Accepted(pair('c', 'd', 300))); runCurrent()
        val accepted = assertIs<NativeApiResult.Accepted<NativeVerifiedSession>>(second.await()).value
        assertEquals(pair('c', 'd', 300).accessToken, accepted.bearer)
        assertFalse(f.owner.accepts(old)); assertTrue(f.owner.accepts(accepted))
        assertTrue(f.owner.belongsToCurrent(old.bearer, family)); assertFalse(f.barrier.pending)
        assertIs<NativeCredentialRecord.Active>(f.journal.record)
        assertTrue(f.events.indexOf("write:refreshing") < f.events.indexOf("refresh"))
        f.owner.close()
    }
    @Test fun unknown_refresh_revokes_old_family_and_never_replays() = runTest {
        val f = Fixture(backgroundScope); f.owner.restore(); runCurrent(); f.time = 190
        f.api.refreshResult = CompletableDeferred(NativeApiResult.Refused(NativeApiFailure.UnknownOutcome))
        val result = async { f.owner.authorization(family) }; runCurrent()
        assertEquals(NativeApiFailure.UnknownOutcome, assertIs<NativeApiResult.Refused>(result.await()).reason)
        assertNull(f.owner.verifiedSession.value); assertEquals(1, f.api.refreshes)
        assertEquals(listOf(pair().refreshToken to family), f.api.revoked)
        assertIs<NativeApiResult.Refused>(f.owner.authorization(family)); assertEquals(1, f.api.refreshes)
        f.owner.close()
    }
    @Test fun interrupted_refresh_record_is_cleanup_only_after_restart() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Refreshing(pair()))
        f.owner.restore(); runCurrent()
        assertEquals(0, f.api.sessions); assertEquals(0, f.api.refreshes)
        assertEquals(listOf(pair().refreshToken to family), f.api.revoked)
        assertEquals(NativeAccountState.SignedOut, f.owner.state.value); assertNull(f.journal.record)
        f.owner.close()
    }
    @Test fun expired_access_restores_by_one_rotation_but_absolute_expiry_revokes() = runTest {
        val f = Fixture(backgroundScope); f.time = 210
        f.owner.restore(); runCurrent()
        assertEquals(1, f.api.refreshes); assertEquals(0, f.api.sessions)
        assertEquals(pair('c', 'd', 300).accessToken, f.owner.verifiedSession.value?.bearer)
        f.owner.close()
        val expired = Fixture(backgroundScope); expired.time = 1000
        expired.owner.restore(); runCurrent()
        assertEquals(0, expired.api.refreshes); assertEquals(1, expired.api.revoked.size)
        assertNull(expired.owner.verifiedSession.value); expired.owner.close()
    }
    @Test fun failed_rotation_journal_write_stops_before_spending_refresh() = runTest {
        val f = Fixture(backgroundScope); f.owner.restore(); runCurrent(); f.time = 190; f.journal.failKind = "refreshing"
        val result = async { f.owner.authorization(family) }; runCurrent()
        assertIs<NativeApiResult.Refused>(result.await()); assertEquals(0, f.api.refreshes)
        assertNull(f.owner.verifiedSession.value); assertTrue(f.barrier.pending)
        f.owner.close()
    }
    @Test fun failed_pair_replacement_revokes_family_and_never_adopts_new_access() = runTest {
        val f = Fixture(backgroundScope); f.owner.restore(); runCurrent(); f.time = 190; f.journal.failKind = "active"
        val result = async { f.owner.authorization(family) }; runCurrent()
        assertIs<NativeApiResult.Refused>(result.await()); assertNull(f.owner.verifiedSession.value)
        assertEquals(1, f.api.refreshes); assertEquals(listOf(pair().refreshToken to family), f.api.revoked)
        f.owner.close()
    }
    @Test fun logout_hides_and_marks_synchronously_then_late_refresh_cannot_restore() = runTest {
        val f = Fixture(backgroundScope); f.owner.restore(); runCurrent(); f.time = 190
        val delayed = CompletableDeferred<NativeApiResult<NativeGrant>>(); f.api.refreshResult = delayed
        val request = async { f.owner.authorization(family) }; runCurrent()
        f.owner.logout(); assertNull(f.owner.verifiedSession.value); assertTrue(f.barrier.pending)
        runCurrent(); assertEquals(NativeAccountState.SignedOut, f.owner.state.value)
        delayed.complete(NativeApiResult.Accepted(pair('c', 'd', 300))); runCurrent()
        assertIs<NativeApiResult.Refused>(request.await()); assertNull(f.owner.verifiedSession.value)
        assertNull(f.journal.record); assertTrue(f.api.revoked.all { it.second == family }); f.owner.close()
    }
    @Test fun cancelled_restore_late_A_cannot_replace_B() = runTest {
        val f = Fixture(backgroundScope)
        val delayed = CompletableDeferred<NativeApiResult<NativeSession>>(); f.api.sessionResult = delayed
        f.owner.restore(); runCurrent(); f.owner.cancelLogin(); runCurrent()
        f.journal.record = NativeCredentialRecord.Active(pair(actor = "actor-B", session = "00000000-0000-4000-8000-000000000002"))
        f.api.sessionResult = null; f.api.sessionValue = (f.journal.record as NativeCredentialRecord.Active).grant.view
        f.owner.restore(); runCurrent(); assertEquals("actor-B", f.owner.verifiedSession.value?.session?.actor)
        delayed.complete(NativeApiResult.Accepted(pair().view)); runCurrent()
        assertEquals("actor-B", f.owner.verifiedSession.value?.session?.actor)
        assertFalse(f.owner.belongsToCurrent(pair().accessToken, family)); f.owner.close()
    }
    @Test fun unknown_exchange_cancels_durable_flow_without_replaying_code() = runTest {
        val f = Fixture(backgroundScope, null); f.api.exchangeResult = NativeApiResult.Refused(NativeApiFailure.UnknownOutcome)
        f.owner.beginLogin(); runCurrent(); assertNotNull(f.browser.opened)
        f.callback(); f.callback(); runCurrent()
        assertEquals(1, f.api.exchanges); assertEquals(listOf(family), f.api.cancelled)
        assertNull(f.owner.verifiedSession.value); assertNull(f.journal.record); f.owner.close()
    }
    @Test fun pending_flow_after_process_death_is_cancelled_never_exchanged() = runTest {
        val record = NativeCredentialRecord.Flow(family, "a".repeat(64), "c".repeat(64))
        val f = Fixture(backgroundScope, record)
        f.owner.restore(); runCurrent()
        assertEquals(listOf(family), f.api.cancelled); assertEquals(0, f.api.exchanges)
        assertEquals(NativeAccountState.SignedOut, f.owner.state.value); f.owner.close()
    }
    @Test fun cleanup_retries_exact_family_and_cleared_record_recovers_delete_failure() = runTest {
        val f = Fixture(backgroundScope); f.owner.restore(); runCurrent(); f.api.revokeFails = true
        f.owner.logout(); runCurrent(); assertEquals(NativeAccountState.CleanupRequired, f.owner.state.value)
        assertTrue(f.barrier.pending); assertNotNull(f.journal.record)
        f.api.revokeFails = false; f.journal.clearFailure = true
        f.owner.logout(); runCurrent(); assertIs<NativeCredentialRecord.Cleared>(f.journal.record)
        assertFalse(f.barrier.pending); f.journal.clearFailure = false
        f.owner.restore(); runCurrent(); assertEquals(NativeAccountState.SignedOut, f.owner.state.value)
        assertNull(f.journal.record); assertEquals(2, f.api.revoked.size); f.owner.close()
    }
    @Test fun orphan_barrier_missing_or_corrupt_storage_never_starts_new_authorization() = runTest {
        val f = Fixture(backgroundScope, null); f.barrier.pending = true
        f.owner.beginLogin(); runCurrent(); assertEquals(NativeAccountState.CleanupRequired, f.owner.state.value)
        assertEquals(0, f.api.exchanges); assertNull(f.browser.opened)
        f.journal.readFailure = CredentialFailure.Corrupt
        f.owner.restore(); runCurrent(); assertNull(f.owner.verifiedSession.value); f.owner.close()
    }
    @Test fun unbound_legacy_credential_is_removed_without_disclosing_to_current_origin() = runTest {
        val f = Fixture(backgroundScope, NativeCredentialRecord.Legacy("e".repeat(64)))
        f.owner.restore(); runCurrent()
        assertEquals(listOf("mark", "write:cleared", "unmark", "delete"), f.events)
        assertEquals(0, f.api.sessions); assertEquals(NativeAccountState.SignedOut, f.owner.state.value); f.owner.close()
    }
    @Test fun first_login_marker_can_recover_if_process_stops_before_scheduled_cleanup() = runTest {
        val f = Fixture(backgroundScope, null)
        f.owner.beginLogin()
        assertTrue(f.barrier.pending)
        assertIs<NativeCredentialRecord.Cleared>(f.journal.record)
        f.owner.close()
        val restarted = f.recreate(backgroundScope)
        restarted.restore(); runCurrent()
        assertEquals(NativeAccountState.SignedOut, restarted.state.value)
        assertFalse(f.barrier.pending); assertNull(f.journal.record)
        restarted.beginLogin(); runCurrent(); assertNotNull(f.browser.opened)
        restarted.close()
    }
    @Test fun committed_rotation_with_failed_secure_readback_still_revokes_known_old_family() = runTest {
        val f = Fixture(backgroundScope); f.owner.restore(); runCurrent(); f.time = 190
        f.journal.commitThenFailReadKind = "active"
        val request = async { f.owner.authorization(family) }; runCurrent()
        assertIs<NativeApiResult.Refused>(request.await())
        assertEquals(listOf(pair().refreshToken to family), f.api.revoked)
        assertNull(f.owner.verifiedSession.value); assertTrue(f.barrier.pending)
        assertEquals(NativeAccountState.CleanupRequired, f.owner.state.value)
        f.owner.close()
    }
    @Test fun committed_exchange_with_failed_secure_readback_still_cancels_exact_flow() = runTest {
        val f = Fixture(backgroundScope, null)
        f.owner.beginLogin(); runCurrent(); f.journal.commitThenFailReadKind = "active"
        f.callback(); runCurrent()
        assertEquals(1, f.api.exchanges); assertEquals(listOf(family), f.api.cancelled)
        assertTrue(f.barrier.pending); assertNull(f.owner.verifiedSession.value)
        f.owner.close()
    }
    @Test fun wrong_actor_on_rotation_revokes_old_family_without_adopting_payload() = runTest {
        val f = Fixture(backgroundScope); f.owner.restore(); runCurrent(); f.time = 190
        f.api.refreshResult = CompletableDeferred(NativeApiResult.Accepted(pair('c', 'd', 300, actor = "actor-B")))
        val request = async { f.owner.authorization(family) }; runCurrent()
        assertIs<NativeApiResult.Refused>(request.await()); assertNull(f.owner.verifiedSession.value)
        assertEquals(listOf(pair().refreshToken to family), f.api.revoked); f.owner.close()
    }
    @Test fun late_A_rotation_cannot_rewrite_secure_B_record_or_revoke_B() = runTest {
        val f = Fixture(backgroundScope); f.owner.restore(); runCurrent(); f.time = 190
        val delayed = CompletableDeferred<NativeApiResult<NativeGrant>>(); f.api.refreshResult = delayed
        val request = async { f.owner.authorization(family) }; runCurrent()
        f.owner.logout(); runCurrent()
        val b = pair('e', 'f', 300, actor = "actor-B", session = "00000000-0000-4000-8000-000000000002")
        f.journal.record = NativeCredentialRecord.Active(b); f.api.sessionValue = b.view
        f.owner.restore(); runCurrent(); assertEquals("actor-B", f.owner.verifiedSession.value?.session?.actor)
        delayed.complete(NativeApiResult.Accepted(pair('c', 'd', 300))); runCurrent()
        assertIs<NativeApiResult.Refused>(request.await())
        assertEquals("actor-B", assertIs<NativeCredentialRecord.Active>(f.journal.record).grant.view.actor)
        assertEquals("actor-B", f.owner.verifiedSession.value?.session?.actor)
        assertTrue(f.api.revoked.all { it.second == family }); f.owner.close()
    }
    @Test fun active_process_expiry_timer_uses_same_rotation_owner() = runTest {
        val f = Fixture(backgroundScope); f.owner.restore(); runCurrent()
        f.time = 190; advanceTimeBy(85_000); runCurrent()
        assertEquals(1, f.api.refreshes)
        assertEquals(pair('c', 'd', 300).accessToken, f.owner.verifiedSession.value?.bearer)
        f.owner.close()
    }

    @Test fun rejected_authority_is_quarantined_before_deferred_reconfirmation() = runTest {
        val f = Fixture(backgroundScope); f.owner.restore(); runCurrent()
        val old = assertNotNull(f.owner.verifiedSession.value)
        val delayed = CompletableDeferred<NativeApiResult<NativeSession>>(); f.api.sessionResult = delayed
        f.owner.revalidate(family)
        assertNull(f.owner.verifiedSession.value)
        assertFalse(f.owner.accepts(old)); assertFalse(f.owner.belongsToCurrent(old.bearer, family))
        assertIs<NativeApiResult.Refused>(f.owner.authorization(family))
        assertEquals(NativeAccountState.Pending, f.owner.state.value)
        runCurrent(); delayed.complete(NativeApiResult.Accepted(pair().view)); runCurrent()
        assertEquals(actor, f.owner.verifiedSession.value?.session?.actor)
        assertTrue(f.owner.accepts(assertNotNull(f.owner.verifiedSession.value)))
        f.owner.close()
    }

    @Test fun access_at_original_authority_ceiling_does_not_spin_refresh_before_expiry() = runTest {
        val finalPair = requireNotNull(NativeGrant.restore(pair().accessToken, pair().refreshToken,
            actor, family, 110, 110, 110, 90))
        val f = Fixture(backgroundScope, NativeCredentialRecord.Active(finalPair))
        f.api.sessionValue = finalPair.view
        f.owner.restore(); runCurrent()
        assertEquals(0, f.api.refreshes); assertEquals(1, f.api.sessions)
        f.time = 109
        assertIs<NativeApiResult.Accepted<NativeVerifiedSession>>(f.owner.authorization(family))
        assertEquals(0, f.api.refreshes)
        f.time = 110; advanceTimeBy(10_000); runCurrent()
        assertNull(f.owner.verifiedSession.value); assertEquals(0, f.api.refreshes)
        assertEquals(listOf(finalPair.refreshToken to family), f.api.revoked)
        f.owner.close()
    }

}
