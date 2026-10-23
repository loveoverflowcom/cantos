package com.cantos.theatre.domain.identity

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull

class NativeIdentityV1ValuesTest {
    private val access = "ci1_n_" + "a".repeat(64)
    private val refresh = "ci1_r_" + "b".repeat(64)
    private val session = "11111111-2222-4333-8444-555555555555"

    private fun pair(
        access: String = this.access, refresh: String = this.refresh, actor: String = "listener_a",
        session: String = this.session, expires: Long = 1120, idle: Long = 1600,
        absolute: Long = 1900, authenticated: Long = 1000, now: Long = 1000,
        previous: NativeGrant? = null,
    ) = NativeGrant.identityV1(access, refresh, actor, session, expires, idle, absolute, authenticated, now, previous)

    private fun invalid(reason: IdentityV1Invalid, value: IdentityV1Validation<*>) {
        assertEquals(IdentityV1Validation.Refused(reason), value)
    }

    @Test fun only_exact_native_identity_credentials_construct_a_grant_and_retired_provider_secrets_are_refused() {
        assertIs<IdentityV1Validation.Accepted<NativeGrant>>(pair())
        for ((a, r) in listOf(
            "ca1_n_" + "a".repeat(64) to refresh,
            access to "cr1_" + "b".repeat(64),
            "ca1_n_" + "a".repeat(64) to "cr1_" + "b".repeat(64),
            "ci1_t_" + "a".repeat(64) to refresh,
            "ci1_s_" + "a".repeat(64) to refresh,
            refresh to access,
            access.uppercase() to refresh,
            access + "a" to refresh,
            "a".repeat(64) to refresh,
        )) invalid(IdentityV1Invalid.Credential, pair(access = a, refresh = r))
        assertEquals(true, NativeGrant.isIdentityV1AccessToken(access))
        assertEquals(false, NativeGrant.isIdentityV1AccessToken(refresh))
        assertEquals(true, NativeGrant.isIdentityV1RefreshToken(refresh))
        assertEquals(false, NativeGrant.isIdentityV1RefreshToken("cr1_" + "b".repeat(64)))
    }

    @Test fun actor_session_deadline_and_authentication_boundaries_are_checked() {
        for (actor in listOf("", "A", "a".repeat(65), "listener a", "a\n", "người")) {
            invalid(IdentityV1Invalid.Actor, pair(actor = actor))
        }
        for (id in listOf("", "not-uuid", "00000000-0000-0000-0000-000000000000", session.uppercase().replace('1', 'A'))) {
            invalid(IdentityV1Invalid.Session, pair(session = id))
        }
        for (value in listOf(pair(now = -1), pair(expires = 1000, authenticated = 999), pair(expires = 1121), pair(idle = 1601),
            pair(absolute = 1901), pair(idle = 1119), pair(absolute = 1599), pair(expires = Long.MAX_VALUE))) {
            invalid(IdentityV1Invalid.Deadline, value)
        }
        for (auth in listOf(-1L, 0L, 1001L, 1120L, Long.MAX_VALUE)) {
            invalid(IdentityV1Invalid.AuthenticatedAt, pair(authenticated = auth))
        }
        assertIs<IdentityV1Validation.Accepted<NativeGrant>>(pair(actor = "a".repeat(64), expires = 1001, idle = 1001, absolute = 1001))
    }

    @Test fun rotation_preserves_family_authentication_and_absolute_deadline() {
        val previous = assertIs<IdentityV1Validation.Accepted<NativeGrant>>(pair()).value
        val nextAccess = "ci1_n_" + "c".repeat(64)
        val nextRefresh = "ci1_r_" + "d".repeat(64)
        val next = assertIs<IdentityV1Validation.Accepted<NativeGrant>>(pair(
            access = nextAccess, refresh = nextRefresh, expires = 1180, idle = 1660, now = 1060, previous = previous,
        )).value
        assertEquals(previous.view.actor, next.view.actor)
        assertEquals(previous.authenticatedAt, next.authenticatedAt)
        for (value in listOf(
            pair(access = nextAccess, refresh = nextRefresh, actor = "other", previous = previous),
            pair(access = nextAccess, refresh = nextRefresh, session = "22222222-2222-4333-8444-555555555555", previous = previous),
            pair(access = nextAccess, refresh = nextRefresh, absolute = 1899, previous = previous),
            pair(access = nextAccess, refresh = nextRefresh, authenticated = 999, previous = previous),
            pair(access = nextAccess, previous = previous), pair(refresh = nextRefresh, previous = previous),
            pair(access = nextAccess, refresh = nextRefresh, idle = 1599, previous = previous),
            pair(access = nextAccess, refresh = nextRefresh, expires = 1119, previous = previous),
        )) invalid(IdentityV1Invalid.Rotation, value)
    }

    @Test fun expired_structural_restoration_never_admits_expired_authority() {
        val restored = assertIs<IdentityV1Validation.Accepted<NativeGrant>>(NativeGrant.restoreIdentityV1(
            access, refresh, "listener_a", session, 1120, 1600, 1900, 1000,
        )).value
        assertNull(restored.admit(1600))
        assertNull(restored.admit(1901))
        // Expired access can only be a refresh candidate, as in the retained coordinator.
        assertNotNull(restored.admit(1200))
        invalid(IdentityV1Invalid.Deadline, pair(now = 1200))
        assertEquals("NativeGrant([redacted])", restored.toString())
        assertEquals("NativeSession([redacted])", restored.view.toString())
    }
}
