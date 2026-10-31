package com.cantos.theatre.adapters.http

import com.cantos.theatre.application.identity.NativeAccountController
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeHttpFailure
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeRequestHandle
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resume

/**
 * Account business requests borrow authority from the process identity owner. A Library operation
 * may retain an older access token across rotation, but only within the same local login and
 * server family. The request is sent once; a 401 or lost mutation response is never replayed here.
 * Cancelling a request cancels its waiter/transport, not the owner's single refresh operation.
 */
class NativeAuthorizedHttpPort(
    private val scope: CoroutineScope,
    private val origin: ApiOrigin,
    private val account: NativeAccountController,
    private val transport: NativeHttpPort,
) : NativeHttpPort {
    override fun send(request: NativeHttpRequest, completion: (NativeHttpResponse) -> Unit): NativeRequestHandle {
        val job = scope.launch {
            if (!isBusinessRequest(request)) {
                completion(refused(NativeHttpFailure.Protocol))
                return@launch
            }
            val bearer = request.bearer
            val family = request.expectedSession
            if (bearer == null || family == null || !account.belongsToCurrent(bearer, family)) {
                completion(refused(NativeHttpFailure.SessionChanged))
                return@launch
            }
            val lease = when (val result = account.authorization(family)) {
                is NativeApiResult.Accepted -> result.value
                is NativeApiResult.Refused -> {
                    val failure = when (result.reason) {
                        NativeApiFailure.Unauthorized -> NativeHttpFailure.Unauthenticated
                        NativeApiFailure.Denied -> NativeHttpFailure.Denied
                        NativeApiFailure.Protocol -> NativeHttpFailure.Protocol
                        NativeApiFailure.SessionChanged -> NativeHttpFailure.SessionChanged
                        NativeApiFailure.Unavailable, NativeApiFailure.UnknownOutcome -> NativeHttpFailure.Unavailable
                    }
                    val identityFailure = result.identityFailure
                    completion(when (identityFailure) {
                        is IdentityV1Failure.RateLimited -> NativeHttpResponse(429, null, null, failure,
                            retryAfter = identityFailure.retryAfterSeconds?.toString())
                        IdentityV1Failure.ServiceUnavailable -> NativeHttpResponse(503, null, null, failure)
                        null, IdentityV1Failure.Unauthorized, IdentityV1Failure.NotConfigured,
                        IdentityV1Failure.SessionChanged, IdentityV1Failure.Denied, IdentityV1Failure.Protocol,
                        IdentityV1Failure.Transport, IdentityV1Failure.UnknownOutcome -> refused(failure)
                    })
                    return@launch
                }
            }
            // Recheck after waiting: a logout or another login may have advanced the owner.
            if (!account.belongsToCurrent(bearer, family) || lease.session.session != family || !account.accepts(lease)) {
                completion(refused(NativeHttpFailure.SessionChanged))
                return@launch
            }
            val response = suspendCancellableCoroutine { continuation ->
                val handle = transport.send(NativeHttpRequest(request.method, request.url, request.body,
                    lease.bearer, family, request.maxBodyBytes)) { reply ->
                    if (continuation.isActive) continuation.resume(reply)
                }
                continuation.invokeOnCancellation { handle.cancel() }
            }
            // The body may be account-owned even on an error path. Discard it after rotation,
            // expiry or logout; operation receipts reconcile uncertain writes at the Library owner.
            // This is a local discarded lease, not a server rejection of the current family.
            // A normal rotation must not quarantine the healthy replacement authority/player.
            completion(if (account.accepts(lease)) response else refused(NativeHttpFailure.AuthorityChanged))
        }
        return object : NativeRequestHandle { override fun cancel() { job.cancel() } }
    }

    private fun refused(reason: String) = NativeHttpResponse(0, null, null, reason)

    /** Exact API origin and existing Theatre business routes only; no auth-route recursion. */
    private fun isBusinessRequest(request: NativeHttpRequest): Boolean {
        val prefix = origin.probeUrl
        // The host admitted this exact scheme/host/port once (debug loopback included).
        // The slash and closed route shapes below prevent authority/path-prefix escapes.
        if (!request.url.startsWith("$prefix/")) return false
        val route = request.url.removePrefix(prefix)
        val id = "[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}"
        return when (request.method) {
            // Pages are only ever read with the closed `presentation=1` opt-in (Library v1.1); the default shape has no consumer.
            "GET" -> sharedCatalogReadPath(route) || route.matches(Regex("/library/(continue|saved|history)\\?limit=(20|100)&presentation=1(&cursor=[0-9a-f]{64})?")) ||
                // Bookmark pages always carry metadata: the server refuses the opt-in query, so it has its own shape.
                route.matches(Regex("/library/bookmarks\\?limit=20(&cursor=[0-9a-f]{64})?")) ||
                route.matches(Regex("/library/saved/$id")) ||
                route.matches(Regex("/(progress|operations)/$id"))
            "PUT" -> route.matches(Regex("/library/saved/$id")) || route.matches(Regex("/progress/$id")) ||
                route.matches(Regex("/library/bookmarks/$id"))
            "POST" -> route in setOf("/library/history/playback-starts", "/library/history/clear") ||
                route.matches(Regex("/library/bookmarks/$id/delete"))
            else -> false
        }
    }
}
