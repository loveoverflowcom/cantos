package com.cantos.theatre.domain.identity

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertSame

class NativeGrantTest {
    private val access = "ca1_n_${"a".repeat(64)}"
    private val refresh = "cr1_${"b".repeat(64)}"
    private val actor = "listener-a"
    private val session = "00000000-0000-4000-8000-000000000001"

    private fun grant(
        accessToken: String = access,
        refreshToken: String = refresh,
        actor: String = this.actor,
        session: String = this.session,
        expiresAt: Long = 1120,
        refreshExpiresAt: Long = 1600,
        absoluteExpiresAt: Long = 1900,
        authenticatedAt: Long? = 900,
        now: Long = 1000,
        previous: NativeGrant? = null,
    ): NativeGrant? = NativeGrant.create(
        accessToken, refreshToken, actor, session, expiresAt, refreshExpiresAt,
        absoluteExpiresAt, authenticatedAt, now, previous,
    )

    @Test fun native_grant_preserves_noncredential_view_and_redacts_every_secret_representation() {
        val value = assertNotNull(grant())
        assertEquals(actor, value.view.actor)
        assertEquals(session, value.view.session)
        assertEquals(1120L, value.expiresAt)
        assertEquals(1600L, value.refreshExpiresAt)
        assertEquals(1900L, value.absoluteExpiresAt)
        assertEquals(900L, value.authenticatedAt)
        assertNull(value.view.credential)
        assertEquals("NativeGrant([redacted])", value.toString())
        assertEquals("NativeSession([redacted])", value.view.toString())
        assertSame(value, value.admit(1000))
    }

    @Test fun only_exact_native_access_and_refresh_purposes_can_construct_a_grant() {
        for (wrong in listOf(access.replace("ca1_n_", "ca1_s_"), access.replace("ca1_n_", "ca1_t_"), "a".repeat(64), access.uppercase(), access + "a", " $access")) {
            assertNull(grant(accessToken = wrong))
        }
        for (wrong in listOf(access, refresh.uppercase(), refresh + "a", "producer-token", "")) {
            assertNull(grant(refreshToken = wrong))
        }
        for (wrong in listOf("", "a".repeat(129), "listener\n", "listener\u0000")) assertNull(grant(actor = wrong))
        for (wrong in listOf("listener-a", session.uppercase().replace("000000000001", "00000000000A"), " $session")) assertNull(grant(session = wrong))
    }

    @Test fun lifetime_limits_are_inclusive_capped_ordered_and_overflow_safe() {
        assertNotNull(grant())
        assertNotNull(grant(expiresAt = 1001, refreshExpiresAt = 1001, absoluteExpiresAt = 1001, authenticatedAt = null))
        assertNull(grant(expiresAt = 1121))
        assertNull(grant(refreshExpiresAt = 1601))
        assertNull(grant(absoluteExpiresAt = 1901))
        assertNull(grant(expiresAt = -1))
        assertNull(grant(expiresAt = 1120, refreshExpiresAt = 1119))
        assertNull(grant(refreshExpiresAt = 1600, absoluteExpiresAt = 1599))
        assertNull(grant(refreshExpiresAt = 1000))
        assertNull(grant(now = -1))
        assertNull(grant(authenticatedAt = -1))
        assertNull(grant(authenticatedAt = 1001))
        assertNull(grant(expiresAt = Long.MAX_VALUE, refreshExpiresAt = Long.MAX_VALUE, absoluteExpiresAt = Long.MAX_VALUE))
        assertNotNull(grant(expiresAt = Long.MAX_VALUE, refreshExpiresAt = Long.MAX_VALUE, absoluteExpiresAt = Long.MAX_VALUE,
            authenticatedAt = null, now = Long.MAX_VALUE - 1))
    }

    @Test fun structural_restore_keeps_expired_cleanup_facts_without_admitting_expired_authority() {
        val expired = assertNotNull(NativeGrant.restore(access, refresh, actor, session, 120, 600, 900, null))
        assertEquals(refresh, expired.refreshToken)
        assertNull(expired.admit(1000))
        assertNull(expired.admit(900))
        assertNull(NativeGrant.restore(access, refresh, actor, session, -1, 600, 900, null))
        assertNull(NativeGrant.restore(access, refresh, actor, session, 120, 600, 599, null))
        assertNull(NativeGrant.restore(access, refresh, actor, session, 120, 600, 900, 901))
    }

    @Test fun an_expired_access_token_can_be_restored_only_as_a_refresh_candidate() {
        val value = assertNotNull(grant(expiresAt = 900))
        assertEquals(900L, value.view.expiresAt)
        assertEquals(1600L, value.refreshExpiresAt)
        assertSame(value, value.admit(1200))
        assertNull(value.admit(1600))
    }

    @Test fun rotation_preserves_actor_family_absolute_deadline_and_original_authentication_time() {
        val previous = assertNotNull(grant())
        val rotatedAccess = "ca1_n_${"c".repeat(64)}"
        val rotatedRefresh = "cr1_${"d".repeat(64)}"
        val next = assertNotNull(grant(accessToken = rotatedAccess, refreshToken = rotatedRefresh,
            expiresAt = 1220, refreshExpiresAt = 1700, now = 1100, previous = previous))
        assertEquals(1900L, next.absoluteExpiresAt)
        assertEquals(900L, next.authenticatedAt)
        assertEquals(actor, next.view.actor)
        assertEquals(session, next.view.session)
        assertNull(grant(accessToken = rotatedAccess, refreshToken = rotatedRefresh, actor = "listener-b", now = 1100, previous = previous))
        assertNull(grant(accessToken = rotatedAccess, refreshToken = rotatedRefresh,
            session = "00000000-0000-4000-8000-000000000002", now = 1100, previous = previous))
        assertNull(grant(accessToken = rotatedAccess, refreshToken = rotatedRefresh, absoluteExpiresAt = 1950, now = 1100, previous = previous))
        assertNull(grant(accessToken = rotatedAccess, refreshToken = rotatedRefresh, absoluteExpiresAt = 1850, now = 1100, previous = previous))
        assertNull(grant(accessToken = rotatedAccess, refreshToken = rotatedRefresh, authenticatedAt = 1000, now = 1100, previous = previous))
        assertNull(grant(accessToken = rotatedAccess, refreshToken = rotatedRefresh, authenticatedAt = null, now = 1100, previous = previous))
    }

    @Test fun rotation_requires_both_new_secrets_and_never_substitutes_token_issue_time_for_auth_time() {
        val previous = assertNotNull(grant(authenticatedAt = null))
        assertNull(grant(previous = previous, now = 1100, authenticatedAt = null))
        assertNull(grant(accessToken = "ca1_n_${"c".repeat(64)}", previous = previous, now = 1100, authenticatedAt = null))
        assertNull(grant(refreshToken = "cr1_${"d".repeat(64)}", previous = previous, now = 1100, authenticatedAt = null))
        val rotated = assertNotNull(grant(accessToken = "ca1_n_${"c".repeat(64)}", refreshToken = "cr1_${"d".repeat(64)}",
            previous = previous, now = 1100, authenticatedAt = null))
        assertNull(rotated.authenticatedAt)
    }
}
