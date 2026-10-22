package com.cantos.theatre.ports

enum class NativeApiFailure { Unavailable, Unauthorized, Denied, Protocol, UnknownOutcome, SessionChanged }
sealed interface NativeApiResult<out T> {
    class Accepted<T>(val value: T) : NativeApiResult<T>
    data class Refused(val reason: NativeApiFailure, val identityFailure: IdentityV1Failure? = null) : NativeApiResult<Nothing>
}
typealias NativeSession = com.cantos.theatre.domain.identity.NativeSession
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
