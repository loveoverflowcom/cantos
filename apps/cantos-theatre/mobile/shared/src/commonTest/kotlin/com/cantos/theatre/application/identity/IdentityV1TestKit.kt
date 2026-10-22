package com.cantos.theatre.application.identity

import com.cantos.theatre.domain.identity.IdentityV1Validation
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.ports.IdentitySignupAccepted
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.NativeIdentityV1Api
import com.cantos.theatre.ports.NativeSession

/** Synthetic Identity v1 access credential: the `ci1_n_` + 64 hex shape the API issues. */
internal fun v1Access(letter: Char) = "ci1_n_" + letter.toString().repeat(64)

/** Synthetic Identity v1 refresh credential: the `ci1_r_` + 64 hex shape the API issues. */
internal fun v1Refresh(letter: Char) = "ci1_r_" + letter.toString().repeat(64)

/**
 * A pair admitted exactly as an API response is: checked against [now] and, on rotation, against
 * the [previous] pair (same actor, family, authentication time and absolute deadline).
 */
internal fun v1Grant(
    accessLetter: Char, refreshLetter: Char = accessLetter, actor: String, session: String,
    expiresAt: Long, refreshExpiresAt: Long, absoluteExpiresAt: Long, authenticatedAt: Long, now: Long,
    previous: NativeGrant? = null,
): NativeGrant {
    val checked = NativeGrant.identityV1(v1Access(accessLetter), v1Refresh(refreshLetter), actor, session,
        expiresAt, refreshExpiresAt, absoluteExpiresAt, authenticatedAt, now, previous)
    return (checked as? IdentityV1Validation.Accepted ?: error("Synthetic pair was refused: $checked")).value
}

/**
 * A scripted Identity v1 API for scenarios that exercise another owner (Library, player, shell).
 * Session reads confirm the presented pair, cleanup succeeds, and a refresh is unavailable unless
 * the scenario overrides it. Login and signup are outside those scenarios.
 */
internal open class StubIdentityV1Api : NativeIdentityV1Api {
    override suspend fun signup(email: RegistrationEmail): IdentityV1Result<IdentitySignupAccepted> = error("Signup is outside this scenario")
    override suspend fun login(email: RegistrationEmail, password: String): IdentityV1Result<NativeGrant> = error("Login is outside this scenario")
    override suspend fun refresh(previous: NativeGrant): IdentityV1Result<NativeGrant> = IdentityV1Result.Refused(IdentityV1Failure.ServiceUnavailable)
    override suspend fun session(previous: NativeGrant): IdentityV1Result<NativeSession> = IdentityV1Result.Accepted(previous.view)
    override suspend fun revokeFamily(previous: NativeGrant): IdentityV1Result<Unit> = IdentityV1Result.Accepted(Unit)
    override suspend fun revoke(previous: NativeGrant): IdentityV1Result<Unit> = error("Use exact family cleanup")
}
