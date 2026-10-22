package com.cantos.theatre.adapters.identity

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.domain.identity.IdentityV1Validation
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
import kotlin.test.assertNull
import kotlin.test.assertSame

class NativeCredentialJournalTest {
    private val origin = "https://api.example.test"
    private val access = "ci1_n_" + "a".repeat(64)
    private val refresh = "ci1_r_" + "b".repeat(64)
    private val session = "11111111-1111-4111-8111-111111111111"

    private fun grant() = assertIs<IdentityV1Validation.Accepted<NativeGrant>>(
        NativeGrant.restoreIdentityV1(access, refresh, "listener-a", session, 100, 200, 300, 50)).value
    private fun createJournal(store: CredentialStore, at: String = origin) = SecureNativeCredentialJournal(store, ApiOrigin.parse(at))
    private fun retired(raw: String, store: MockCredentialStore = MockCredentialStore()): NativeJournalRead {
        store.value = raw
        return createJournal(store).read()
    }

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
            assertEquals(50L, restoredGrant.authenticatedAt)
            assertNull(restoredGrant.admit(1_000))
        }
    }

    @Test fun cleared_marker_is_one_atomic_secure_value_in_the_protocol_bound_envelope() {
        val store = MockCredentialStore()
        val journal = createJournal(store)
        assertNull(journal.write(NativeCredentialRecord.Cleared))
        assertSame(NativeCredentialRecord.Cleared, assertIs<NativeJournalRead.Found>(journal.read()).record)
        assertEquals("{\"version\":2,\"api_origin\":\"$origin\",\"protocol\":\"identity-v1\",\"kind\":\"cleared\"}", store.value)
        assertEquals(1, store.writes)
    }

    @Test fun retired_records_are_recognized_for_local_removal_and_are_never_written_or_restored_as_credentials() {
        val store = MockCredentialStore()
        val recognized = listOf(
            RetiredProviderRecords.opaqueToken(), RetiredProviderRecords.active(origin), RetiredProviderRecords.refreshing(origin),
            RetiredProviderRecords.flow(origin), RetiredProviderRecords.legacyKind(origin), RetiredProviderRecords.cleared(origin),
            RetiredProviderRecords.activeWithoutAuthTime(origin),
        )
        for (raw in recognized) {
            assertSame(NativeCredentialRecord.Retired, assertIs<NativeJournalRead.Found>(retired(raw, store)).record)
        }
        // Recognition is read-only: the journal cannot manufacture a retired record.
        store.value = null
        assertEquals(CredentialFailure.Corrupt, createJournal(store).write(NativeCredentialRecord.Retired))
        assertEquals(0, store.writes)
    }

    @Test fun unrecognized_or_foreign_retired_shapes_stay_corrupt_and_quarantined() {
        val valid = RetiredProviderRecords.active(origin)
        val corrupt = listOf(
            RetiredProviderRecords.opaqueToken('A'),                                  // upper-case hex is not our token
            RetiredProviderRecords.active("https://other-api.example.test"),         // another API origin
            RetiredProviderRecords.active("https://api.example.test:443"),           // origin is exact, not normalized
            RetiredProviderRecords.active(origin, issuer = "https://idm.example/oauth2/openid/other"),
            RetiredProviderRecords.active(origin, issuer = "http://idm.example/oauth2/openid/cantos-theatre-native"),
            valid + " ", " $valid", valid.replace("\"version\":1", "\"version\":1,\"version\":1"),
            valid.replace("\"kind\":\"active\"", "\"kind\":\"unknown\""),
            valid.replace("ca1_n_", "ca1_s_"), valid.replace("cr1_", "ca1_n_"),
            valid.replace("\"actor_id\":\"reader-a\"", "\"actor_id\":\"\""),
            valid.replace("\"authenticated_at\":90", "\"authenticated_at\":\"90\""),
            valid.replace("\"authenticated_at\":90", "\"authenticated_at\":90,\"extra\":true"),
            valid.replace("\"session_id\":\"${RetiredProviderRecords.Session}\"", "\"session_id\":\"bad\""),
            RetiredProviderRecords.flow(origin).replace("\"state\":\"${"e".repeat(64)}\"", "\"state\":\"short\""),
            valid.replace("\"api_origin\"", "\"api_origin\\u0000\""),
            "{\"version\":1}", "{}", "[]", "null", "", "x".repeat(32_769),
        )
        for (raw in corrupt) assertEquals(NativeJournalRead.Failed(CredentialFailure.Corrupt), retired(raw), raw.take(60))
    }

    @Test fun decoder_rejects_ambiguous_corrupt_and_cross_purpose_v2_records() {
        val store = MockCredentialStore()
        val journal = createJournal(store)
        assertNull(journal.write(NativeCredentialRecord.Active(grant())))
        val valid = assertIs<String>(store.value)
        val invalid = listOf(
            "", "null", "[]", "{}", "x".repeat(32_769), valid + " ",
            valid.replace("\"version\":2", "\"version\":1"),
            valid.replace("\"version\":2", "\"version\":\"2\""),
            valid.replace("\"version\":2", "\"version\":2,\"version\":2"),
            valid.replace("identity-v1", "provider"),
            valid.replace("\"kind\":\"active\"", "\"kind\":\"unknown\""),
            valid.replace("\"kind\":\"active\"", "\"kind\":\"flow\""),
            valid.replace("\"kind\":\"active\"", "\"kind\":\"legacy\""),
            valid.replace("ci1_n_", "ci1_s_"), valid.replace("ci1_r_", "ci1_n_"), valid.replace("ci1_r_", "cr1_"),
            valid.replace("\"expires_at\":100", "\"expires_at\":301"),
            valid.replace("\"authenticated_at\":50", "\"authenticated_at\":null"),
            valid.replace("\"authenticated_at\":50", "\"authenticated_at\":false"),
            valid.replace("\"authenticated_at\":50", "\"authenticated_at\":50,\"extra\":true"),
            valid.replace("\"session_id\":\"$session\"", "\"session_id\":\"bad\""),
        )
        for (raw in invalid) {
            store.value = raw
            assertEquals(NativeJournalRead.Failed(CredentialFailure.Corrupt), journal.read(), raw.take(60))
        }
        assertEquals(1, store.writes)
    }

    @Test fun storage_failures_stay_distinct_from_corrupt_and_incomplete_readback_refuses_admission() {
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

    @Test fun diagnostics_never_reveal_tokens() {
        val records = listOf(NativeCredentialRecord.Active(grant()), NativeCredentialRecord.Refreshing(grant()))
        for (record in records) {
            for (output in listOf(record.toString(), NativeJournalRead.Found(record).toString())) {
                for (secret in listOf(access, refresh, "a".repeat(64))) assertFalse(output.contains(secret))
            }
        }
    }

    @Test fun journal_cannot_restore_a_pair_for_another_or_renormalized_origin() {
        val store = MockCredentialStore()
        for (record in listOf(NativeCredentialRecord.Active(grant()), NativeCredentialRecord.Refreshing(grant()), NativeCredentialRecord.Cleared)) {
            assertNull(createJournal(store).write(record))
            for (other in listOf("https://other-api.example.test", "https://api.example.test:443", "http://127.0.0.1:8080")) {
                assertEquals(NativeJournalRead.Failed(CredentialFailure.Corrupt), createJournal(store, other).read(), other)
            }
            assertIs<NativeJournalRead.Found>(createJournal(store).read())
        }
    }

    @Test fun an_admitted_debug_loopback_origin_binds_exactly_like_any_other_origin() {
        val store = MockCredentialStore()
        val loopback = createJournal(store, "http://127.0.0.1:8080")
        assertNull(loopback.write(NativeCredentialRecord.Active(grant())))
        assertIs<NativeCredentialRecord.Active>(assertIs<NativeJournalRead.Found>(createJournal(store, "http://127.0.0.1:8080").read()).record)
        assertEquals(NativeJournalRead.Failed(CredentialFailure.Corrupt), createJournal(store, "http://localhost:8080").read())
        assertEquals(NativeJournalRead.Failed(CredentialFailure.Corrupt), createJournal(store, "https://127.0.0.1:8080").read())
    }
}
