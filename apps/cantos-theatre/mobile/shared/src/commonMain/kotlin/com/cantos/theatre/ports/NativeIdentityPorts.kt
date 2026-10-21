package com.cantos.theatre.ports

import com.cantos.theatre.domain.identity.NativeGrant

enum class NativeApiFailure { Unavailable, Unauthorized, Denied, Protocol, UnknownOutcome, SessionChanged }
sealed interface NativeApiResult<out T> {
    class Accepted<T>(val value: T) : NativeApiResult<T>
    data class Refused(val reason: NativeApiFailure, val identityFailure: IdentityV1Failure? = null) : NativeApiResult<Nothing>
}
class NativeFlow(val id: String, val authorizationUrl: String) {
    override fun toString(): String = "NativeFlow([redacted])"
}
typealias NativeSession = com.cantos.theatre.domain.identity.NativeSession
interface NativeIdentityApi {
    suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow>
    suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeGrant>
    suspend fun refresh(refreshToken: String, session: String): NativeApiResult<NativeGrant>
    suspend fun revokeFamily(refreshToken: String, session: String): NativeApiResult<Unit>
    suspend fun cancel(flow: String, state: String, verifier: String): NativeApiResult<Unit>
    suspend fun session(credential: String): NativeApiResult<NativeSession>
    /** Access-only revocation exists to clean up compatibility credentials during migration. */
    suspend fun revoke(credential: String): NativeApiResult<Unit>
    suspend fun saved(credential: String, session: String): NativeApiResult<Int>
}

/** Platform HTTP boundary. Requests and responses never stringify bearer/body values. */
class NativeHttpRequest internal constructor(val method: String, val url: String, val body: String?, val bearer: String?, val expectedSession: String? = null) {
    /** Closed header set: native requests cannot acquire browser credentials or metadata here. */
    val headers: Map<String, String> get() = buildMap {
        put("Accept", "application/json")
        if (body != null) put("Content-Type", "application/json")
        bearer?.let { put("Authorization", "Bearer $it") }
        expectedSession?.let { put("X-Cantos-Theatre-Session", it) }
    }
    override fun toString(): String = "NativeHttpRequest([redacted])"
}
class NativeHttpResponse(val status: Int, val contentType: String?, val body: String?, val failure: String?, val actualSession: String? = null, val retryAfter: String? = null) {
    override fun toString(): String = "NativeHttpResponse([redacted])"
}
interface NativeRequestHandle { fun cancel() }
interface NativeHttpPort {
    fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle
}
enum class LogoutBarrierRead { Clear, Pending, Unavailable }
/** Nonsecret durable logout intent only. Written before HTTP revocation or OS deletion. */
interface LogoutBarrier {
    fun read(): LogoutBarrierRead
    fun mark(): CredentialFailure?
    fun clear(): CredentialFailure?
}
