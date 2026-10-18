package com.cantos.theatre.adapters.identity

import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.CredentialRead
import com.cantos.theatre.ports.CredentialStore
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeJournalRead
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertSame

class NativeCredentialJournalTest {
    private val apiOrigin = "https://api.example.test"
    private val issuer = "https://id.example.test/oauth2/openid/cantos-theatre-native"
    private val access = "ca1_n_" + "a".repeat(64)
    private val refresh = "cr1_" + "b".repeat(64)
    private val session = "11111111-1111-1111-1111-111111111111"
    private val flow = "22222222-2222-2222-2222-222222222222"
    private val state = "c".repeat(43)
    private val verifier = "d".repeat(43)

    private fun grant() = assertNotNull(NativeGrant.restore(access, refresh, "listener-a", session, 100, 200, 300, 0))
    private fun createJournal(store: CredentialStore) = SecureNativeCredentialJournal(store, apiOrigin, issuer)

    private class MockCredentialStore : CredentialStore {
        var value: String? = null
        var failure: CredentialFailure? = null
        var writeFailure: CredentialFailure? = null
        var corruptWrite = false
        var ignoreClear = false
        var writes = 0
        override fun read(): CredentialRead = failure?.let { CredentialRead.Failed(it) }
            ?: value?.let { CredentialRead.Found(it) } ?: CredentialRead.Missing
        override fun write(value: String): CredentialFailure? {
            writes += 1
            writeFailure?.let { return it }
            this.value = if (corruptWrite) "corrupt" else value
            return null
        }
        override fun clear(): CredentialFailure? {
            if (!ignoreClear) value = null
            return null
        }
    }

    @Test fun full_active_and_inflight_records_survive_recreation_even_when_expired() {
        val store = MockCredentialStore()
        val journal = createJournal(store)
        for (record in listOf(NativeCredentialRecord.Active(grant()), NativeCredentialRecord.Refreshing(grant()))) {
            assertNull(journal.write(record))
            val restored = assertIs<NativeJournalRead.Found>(createJournal(store).read()).record
            val restoredGrant = when (restored) {
                is NativeCredentialRecord.Active -> { assertIs<NativeCredentialRecord.Active>(record); restored.grant }
                is NativeCredentialRecord.Refreshing -> { assertIs<NativeCredentialRecord.Refreshing>(record); restored.grant }
                else -> error("Wrong journal record")
            }
            assertEquals(access, restoredGrant.accessToken)
            assertEquals(refresh, restoredGrant.refreshToken)
            assertEquals("listener-a", restoredGrant.view.actor)
            assertEquals(session, restoredGrant.view.session)
            assertEquals(100L, restoredGrant.expiresAt)
            assertEquals(200L, restoredGrant.refreshExpiresAt)
            assertEquals(300L, restoredGrant.absoluteExpiresAt)
            assertEquals(0L, restoredGrant.authenticatedAt)
            assertNull(restoredGrant.admit(1_000))
        }
    }

    @Test fun flow_and_cleared_markers_are_one_atomic_secure_value() {
        val store = MockCredentialStore()
        val journal = createJournal(store)
        assertNull(journal.write(NativeCredentialRecord.Flow(flow, state, verifier)))
        val record = assertIs<NativeCredentialRecord.Flow>(assertIs<NativeJournalRead.Found>(journal.read()).record)
        assertEquals(flow, record.flow)
        assertEquals(state, record.state)
        assertEquals(verifier, record.verifier)
        assertNull(journal.write(NativeCredentialRecord.Cleared))
        assertSame(NativeCredentialRecord.Cleared, assertIs<NativeJournalRead.Found>(journal.read()).record)
        assertEquals("{\"version\":1,\"api_origin\":\"$apiOrigin\",\"issuer\":\"$issuer\",\"kind\":\"cleared\"}", store.value)
        assertEquals(2, store.writes)
    }

    @Test fun legacy_hex_is_cleanup_only_and_never_interpreted_as_a_pair() {
        val store = MockCredentialStore().apply { value = "a".repeat(64) }
        val journal = createJournal(store)
        val legacy = assertIs<NativeCredentialRecord.Legacy>(assertIs<NativeJournalRead.Found>(journal.read()).record)
        assertEquals("a".repeat(64), legacy.accessToken)
        assertNull(journal.write(legacy))
        assertIs<NativeCredentialRecord.Legacy>(assertIs<NativeJournalRead.Found>(journal.read()).record)
        store.value = "A".repeat(64)
        assertEquals(NativeJournalRead.Failed(CredentialFailure.Corrupt), journal.read())
    }

    @Test fun decoder_rejects_ambiguous_corrupt_and_cross_purpose_records() {
        val store = MockCredentialStore()
        val journal = createJournal(store)
        assertNull(journal.write(NativeCredentialRecord.Active(grant())))
        val valid = assertNotNull(store.value)
        val invalid = listOf(
            "", "null", "[]", "{}", "x".repeat(32_769), valid + " ",
            valid.replace("\"version\":1", "\"version\":2"),
            valid.replace("\"version\":1", "\"version\":\"1\""),
            valid.replace("\"version\":1", "\"version\":1,\"version\":1"),
            valid.replace("\"kind\":\"active\"", "\"kind\":\"unknown\""),
            valid.replace("ca1_n_", "ca1_s_"), valid.replace("cr1_", "ca1_n_"),
            valid.replace("\"expires_at\":100", "\"expires_at\":301"),
            valid.replace("\"authenticated_at\":0", "\"authenticated_at\":null,\"extra\":true"),
            valid.replace("\"authenticated_at\":0", "\"authenticated_at\":false"),
            valid.replace("\"session_id\":\"$session\"", "\"session_id\":\"bad\""),
        )
        for (raw in invalid) {
            store.value = raw
            assertEquals(NativeJournalRead.Failed(CredentialFailure.Corrupt), journal.read())
        }
        assertEquals(CredentialFailure.Corrupt, journal.write(NativeCredentialRecord.Flow(flow, "bad", verifier)))
        assertEquals(CredentialFailure.Corrupt, journal.write(NativeCredentialRecord.Legacy("bad")))
        assertEquals(1, store.writes)
    }

    @Test fun storage_failures_and_incomplete_readback_refuse_admission() {
        val store = MockCredentialStore()
        val journal = createJournal(store)
        assertSame(NativeJournalRead.Missing, journal.read())
        for (failure in CredentialFailure.entries) {
            store.failure = failure
            assertEquals(NativeJournalRead.Failed(failure), journal.read())
            store.failure = null
            store.writeFailure = failure
            assertEquals(failure, journal.write(NativeCredentialRecord.Active(grant())))
        }
        store.writeFailure = null
        store.corruptWrite = true
        assertEquals(CredentialFailure.Corrupt, journal.write(NativeCredentialRecord.Refreshing(grant())))
        store.corruptWrite = false
        assertNull(journal.write(NativeCredentialRecord.Cleared))
        store.ignoreClear = true
        assertEquals(CredentialFailure.Unavailable, journal.clear())
        store.ignoreClear = false
        assertNull(journal.clear())
        assertSame(NativeJournalRead.Missing, journal.read())
    }

    @Test fun diagnostics_never_reveal_tokens_or_pkce_material() {
        val records = listOf(NativeCredentialRecord.Active(grant()), NativeCredentialRecord.Refreshing(grant()),
            NativeCredentialRecord.Flow(flow, state, verifier), NativeCredentialRecord.Legacy("a".repeat(64)))
        for (record in records) {
            for (output in listOf(record.toString(), NativeJournalRead.Found(record).toString())) {
                for (secret in listOf(access, refresh, flow, state, verifier, "a".repeat(64))) assertFalse(output.contains(secret))
            }
        }
    }

    @Test fun journal_cannot_restore_a_pair_or_flow_for_another_api_or_issuer() {
        val store = MockCredentialStore()
        for (record in listOf(NativeCredentialRecord.Active(grant()), NativeCredentialRecord.Refreshing(grant()),
                NativeCredentialRecord.Flow(flow, state, verifier), NativeCredentialRecord.Cleared)) {
            assertNull(createJournal(store).write(record))
            for (other in listOf(
                    SecureNativeCredentialJournal(store, "https://other-api.example.test", issuer),
                    SecureNativeCredentialJournal(store, apiOrigin, "https://other-id.example.test/oauth2/openid/cantos-theatre-native"),
                    SecureNativeCredentialJournal(store, "https://api.example.test:443", issuer),
                )) {
                assertEquals(NativeJournalRead.Failed(CredentialFailure.Corrupt), other.read())
            }
            assertIs<NativeJournalRead.Found>(createJournal(store).read())
        }
        val invalid = SecureNativeCredentialJournal(store, "http://api.example.test", issuer)
        assertEquals(NativeJournalRead.Failed(CredentialFailure.Corrupt), invalid.read())
        assertEquals(CredentialFailure.Corrupt, invalid.write(NativeCredentialRecord.Cleared))
    }
}
