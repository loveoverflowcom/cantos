package com.cantos.theatre.adapters.http

import com.cantos.theatre.adapters.http.identityv1wire.Session
import com.cantos.theatre.adapters.http.identityv1wire.SignupAccepted
import com.cantos.theatre.adapters.http.identityv1wire.TokenPair
import com.cantos.theatre.domain.identity.IdentityV1Validation
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.identity.NativeSession
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.ports.IdentitySignupAccepted
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.ports.IdentityV1Result
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeIdentityV1Api
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlin.coroutines.resume

/**
 * First-party Identity v1 only. Native hosts pair this transport with the origin-bound
 * secure journal and the existing account coordinator. The [origin] is the one admitted by the
 * host's origin owner (HTTPS; loopback HTTP only for a debug host); this client never widens it.
 * No provider discovery/fallback, mailbox completion/polling, or transparent mutation retry.
 */
class NativeIdentityV1Client(
    private val origin: ApiOrigin,
    private val http: NativeHttpPort,
    private val now: () -> Long,
) : NativeIdentityV1Api {
    private val prefix = "$origin/api/v1/theatre/native/identity"

    private suspend fun request(
        method: String, path: String, body: String? = null,
        bearer: String? = null, expectedSession: String? = null,
    ): NativeHttpResponse {
        if (body != null && body.encodeToByteArray().size > 4096) {
            return NativeHttpResponse(400, null, null, null)
        }
        return withTimeoutOrNull(15_000) {
            suspendCancellableCoroutine { continuation ->
                val handle = http.send(NativeHttpRequest(method, prefix + path, body, bearer, expectedSession)) {
                    if (continuation.isActive) continuation.resume(it)
                }
                continuation.invokeOnCancellation { handle.cancel() }
            }
        } ?: NativeHttpResponse(0, null, null, "timeout")
    }

    private fun failure(response: NativeHttpResponse, mutation: Boolean): IdentityV1Failure? = when {
        response.failure != null || response.status == 0 ->
            if (mutation) IdentityV1Failure.UnknownOutcome else IdentityV1Failure.Transport
        response.status == 401 -> IdentityV1Failure.Unauthorized
        response.status == 403 -> IdentityV1Failure.Denied
        response.status == 404 -> IdentityV1Failure.NotConfigured
        response.status == 409 -> IdentityV1Failure.SessionChanged
        response.status == 429 -> IdentityV1Failure.RateLimited(retryAfter(response.retryAfter))
        response.status == 503 -> IdentityV1Failure.ServiceUnavailable
        response.status in setOf(400, 413, 415, 422) -> IdentityV1Failure.Protocol
        response.status in 200..299 -> null
        else -> if (mutation) IdentityV1Failure.UnknownOutcome else IdentityV1Failure.Protocol
    }

    // This contract sends delta-seconds. Never echo an untrusted header or guess a retry time.
    private fun retryAfter(raw: String?): Long? = raw?.trim()?.takeIf {
        it.isNotEmpty() && it.length <= 19 && it.all { char -> char in '0'..'9' }
    }?.toLongOrNull()

    private inline fun <reified T> decode(response: NativeHttpResponse, shape: (JsonObject) -> Boolean): T? {
        val body = response.body ?: return null
        if (body.encodeToByteArray().size > 65_536 ||
            response.contentType?.substringBefore(';')?.trim()?.lowercase() != "application/json") return null
        return try {
            val value = Json.parseToJsonElement(body) as? JsonObject ?: return null
            if (repeatedTopLevelKeys(body) || !shape(value)) return null
            Json.decodeFromString<T>(body)
        } catch (_: SerializationException) {
            null
        } catch (_: IllegalArgumentException) {
            null
        }
    }

    override suspend fun signup(email: RegistrationEmail): IdentityV1Result<IdentitySignupAccepted> {
        val response = request("POST", "/signup", buildJsonObject { put("email", email.value) }.toString())
        failure(response, mutation = true)?.let { return IdentityV1Result.Refused(it) }
        val dto = decode<SignupAccepted>(response, SignupAccepted::accepts)
        // Schema requires a positive integer; the Rust DTO additionally bounds this to u32.
        if (response.status != 202 || dto == null || dto.proof_ttl_seconds > 4_294_967_295L) {
            return IdentityV1Result.Refused(IdentityV1Failure.UnknownOutcome)
        }
        return IdentityV1Result.Accepted(IdentitySignupAccepted(dto.proof_ttl_seconds))
    }

    override suspend fun login(email: RegistrationEmail, password: String): IdentityV1Result<NativeGrant> =
        grant(request("POST", "/login", buildJsonObject {
            put("email", email.value)
            put("password", password)
        }.toString()))

    private fun grant(response: NativeHttpResponse, previous: NativeGrant? = null): IdentityV1Result<NativeGrant> {
        failure(response, mutation = true)?.let { return IdentityV1Result.Refused(it) }
        val unknown = IdentityV1Result.Refused(IdentityV1Failure.UnknownOutcome)
        if (response.status != 200) return unknown
        val dto = decode<TokenPair>(response, TokenPair::accepts) ?: return unknown
        // The reused Rust DTO permits null auth_time for legacy providers; Identity v1 does not.
        return when (val checked = NativeGrant.identityV1(
            dto.access_token, dto.refresh_token, dto.actor_id, dto.session_id, dto.expires_at,
            dto.refresh_expires_at, dto.absolute_expires_at, dto.authenticated_at, now(), previous,
        )) {
            is IdentityV1Validation.Accepted -> IdentityV1Result.Accepted(checked.value)
            is IdentityV1Validation.Refused -> unknown
        }
    }

    private fun familyBody(value: NativeGrant): String = buildJsonObject {
        put("refresh_token", value.refreshToken)
        put("session_id", value.view.session)
    }.toString()

    override suspend fun refresh(previous: NativeGrant): IdentityV1Result<NativeGrant> {
        if (previous.admit(now()) == null) return IdentityV1Result.Refused(IdentityV1Failure.Unauthorized)
        return grant(request("POST", "/refresh", familyBody(previous), expectedSession = previous.view.session), previous)
    }

    private fun cleanup(response: NativeHttpResponse): IdentityV1Result<Unit> {
        failure(response, mutation = true)?.let { return IdentityV1Result.Refused(it) }
        return if (response.status == 204) IdentityV1Result.Accepted(Unit)
        else IdentityV1Result.Refused(IdentityV1Failure.UnknownOutcome)
    }

    override suspend fun revokeFamily(previous: NativeGrant): IdentityV1Result<Unit> {
        // Expiry does not block family cleanup; the server accepts spent/expired refresh secrets.
        return cleanup(request("POST", "/revoke", familyBody(previous), expectedSession = previous.view.session))
    }

    override suspend fun session(previous: NativeGrant): IdentityV1Result<NativeSession> {
        if (previous.expiresAt <= now() || previous.admit(now()) == null) {
            return IdentityV1Result.Refused(IdentityV1Failure.Unauthorized)
        }
        val response = request("GET", "/session", bearer = previous.accessToken, expectedSession = previous.view.session)
        failure(response, mutation = false)?.let { return IdentityV1Result.Refused(it) }
        val invalid = IdentityV1Result.Refused(IdentityV1Failure.Protocol)
        if (response.status != 200) return invalid
        if (response.actualSession != null && response.actualSession != previous.view.session) {
            return IdentityV1Result.Refused(IdentityV1Failure.SessionChanged)
        }
        val dto = decode<Session>(response, Session::accepts) ?: return invalid
        if (dto.actor_id != previous.view.actor || dto.session_id != previous.view.session ||
            dto.absolute_expires_at != previous.absoluteExpiresAt || dto.authenticated_at != previous.authenticatedAt ||
            dto.expires_at != previous.expiresAt) return invalid
        return when (val checked = NativeSession.identityV1(
            dto.actor_id, dto.session_id, dto.expires_at, dto.absolute_expires_at, dto.authenticated_at, now(),
        )) {
            is IdentityV1Validation.Accepted -> IdentityV1Result.Accepted(checked.value)
            is IdentityV1Validation.Refused -> invalid
        }
    }

    override suspend fun revoke(previous: NativeGrant): IdentityV1Result<Unit> {
        return cleanup(request("DELETE", "/session", bearer = previous.accessToken, expectedSession = previous.view.session))
    }
}
