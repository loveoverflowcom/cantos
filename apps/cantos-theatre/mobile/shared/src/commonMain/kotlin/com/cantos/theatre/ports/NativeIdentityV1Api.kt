package com.cantos.theatre.ports

import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.identity.NativeSession
import com.cantos.theatre.domain.identity.RegistrationEmail

sealed interface IdentityV1Failure {
    data object Unauthorized : IdentityV1Failure
    data object NotConfigured : IdentityV1Failure
    data object SessionChanged : IdentityV1Failure
    data class RateLimited(val retryAfterSeconds: Long?) : IdentityV1Failure
    /** A 503 does not establish rollback of a login, refresh or signup. */
    data object ServiceUnavailable : IdentityV1Failure
    data object Denied : IdentityV1Failure
    data object Protocol : IdentityV1Failure
    data object Transport : IdentityV1Failure
    data object UnknownOutcome : IdentityV1Failure
}

sealed interface IdentityV1Result<out T> {
    class Accepted<T>(val value: T) : IdentityV1Result<T>
    data class Refused(val reason: IdentityV1Failure) : IdentityV1Result<Nothing>
}

/** Receipt is only a generic initiation acknowledgment, never mailbox or session evidence. */
class IdentitySignupAccepted internal constructor(val proofTtlSeconds: Long)

/** The first-party Identity v1 HTTP boundary: explicit email/password, rotation and revocation only. */
interface NativeIdentityV1Api {
    suspend fun signup(email: RegistrationEmail): IdentityV1Result<IdentitySignupAccepted>
    suspend fun login(email: RegistrationEmail, password: String): IdentityV1Result<NativeGrant>
    suspend fun refresh(previous: NativeGrant): IdentityV1Result<NativeGrant>
    suspend fun revokeFamily(previous: NativeGrant): IdentityV1Result<Unit>
    suspend fun session(previous: NativeGrant): IdentityV1Result<NativeSession>
    // TODO(owner): production cleanup uses revokeFamily only; this DELETE /session access-only revoke has no
    // production caller. Kept because the contract defines it and the client tests cover it.
    suspend fun revoke(previous: NativeGrant): IdentityV1Result<Unit>
}
