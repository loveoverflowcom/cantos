package com.cantos.theatre.ports

enum class NativeApiFailure { Unavailable, Unauthorized, Denied, Protocol, UnknownOutcome, SessionChanged }
sealed interface NativeApiResult<out T> {
    class Accepted<T>(val value: T) : NativeApiResult<T>
    data class Refused(val reason: NativeApiFailure, val identityFailure: IdentityV1Failure? = null) : NativeApiResult<Nothing>
}
typealias NativeSession = com.cantos.theatre.domain.identity.NativeSession
/**
 * Wire spellings the native HTTP boundary shares between the request builder and the platform
 * transports that stamp or read them. Tests keep their own literals as the independent oracle.
 */
object NativeHttpWire {
    /** Names the session family a request is fenced to and the family the server answered for. */
    const val SessionHeader = "X-Cantos-Theatre-Session"
}

/**
 * The local refusal codes a [NativeHttpPort] reports in [NativeHttpResponse.failure] when no HTTP
 * reply exists (`status` 0). Producers and the Library boundary branch on these exact strings,
 * and they surface as `LibraryResult.Failed.code`: a misspelling on one side would fall through
 * to a generic failure without failing compilation, so both sides reference this one owner.
 */
object NativeHttpFailure {
    const val Timeout = "timeout"
    const val Transport = "transport"
    const val Configuration = "configuration"
    const val Protocol = "protocol"
    const val Denied = "denied"
    const val Unauthenticated = "unauthenticated"
    const val SessionChanged = "session_changed"
    const val AuthorityChanged = "authority_changed"
    const val Unavailable = "unavailable"
}

/** Platform HTTP boundary. Requests and responses never stringify bearer/body values. */
class NativeHttpRequest internal constructor(val method: String, val url: String, val body: String?, val bearer: String?, val expectedSession: String? = null) {
    /** Closed header set: native requests cannot acquire browser credentials or metadata here. */
    val headers: Map<String, String> get() = buildMap {
        put("Accept", "application/json")
        if (body != null) put("Content-Type", "application/json")
        bearer?.let { put("Authorization", "Bearer $it") }
        expectedSession?.let { put(NativeHttpWire.SessionHeader, it) }
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
