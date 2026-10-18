package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeFlow
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.NativeIdentityApi
import com.cantos.theatre.ports.NativeSession
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import kotlin.coroutines.resume

/** JSON-only Bearer v1 boundary; never retries a consumed exchange, rotation or mutation. */
class NativeIdentityClient(
    private val configuration: NativeClientConfiguration,
    private val http: NativeHttpPort,
    private val now: () -> Long,
) : NativeIdentityApi {
    private val prefix = "/native/auth/bearer"

    private suspend fun request(
        method: String,
        path: String,
        body: String? = null,
        credential: String? = null,
        expectedSession: String? = null,
    ): NativeHttpResponse = withTimeoutOrNull(15_000) {
        suspendCancellableCoroutine { continuation ->
            val handle = http.send(
                NativeHttpRequest(method, "${configuration.apiOrigin}/api/v1/theatre$path", body, credential, expectedSession),
            ) { if (continuation.isActive) continuation.resume(it) }
            continuation.invokeOnCancellation { handle.cancel() }
        }
    } ?: NativeHttpResponse(0, null, null, "timeout")

    private fun failure(response: NativeHttpResponse, mutation: Boolean): NativeApiFailure? = when {
        response.failure != null -> if (mutation) NativeApiFailure.UnknownOutcome else NativeApiFailure.Unavailable
        response.status == 401 -> NativeApiFailure.Unauthorized
        response.status == 403 -> NativeApiFailure.Denied
        response.status in 200..299 -> null
        mutation && response.status in setOf(400, 404, 409, 410, 422) -> NativeApiFailure.Protocol
        mutation -> NativeApiFailure.UnknownOutcome
        else -> NativeApiFailure.Unavailable
    }

    private fun json(response: NativeHttpResponse): JsonObject? {
        val body = response.body ?: return null
        if (body.length > 65536 || response.contentType?.substringBefore(';')?.trim()?.lowercase() != "application/json") return null
        return try {
            val value = Json.parseToJsonElement(body) as? JsonObject ?: return null
            if (repeatedTopLevelKeys(body)) null else value
        } catch (_: SerializationException) {
            null
        } catch (_: IllegalArgumentException) {
            null
        }
    }

    /** JsonObject otherwise keeps only the last duplicate, including escaped key spellings. */
    private fun repeatedTopLevelKeys(body: String): Boolean {
        val keys = mutableSetOf<String>()
        var depth = 0
        var keyExpected = false
        var index = 0
        while (index < body.length) {
            when (body[index]) {
                '{', '[' -> { depth += 1; if (depth == 1) keyExpected = true }
                '}', ']' -> depth -= 1
                ',' -> if (depth == 1) keyExpected = true
                ':' -> if (depth == 1) keyExpected = false
                '"' -> {
                    val start = index
                    index += 1
                    while (index < body.length && body[index] != '"') {
                        if (body[index] == '\\') index += 1
                        index += 1
                    }
                    if (depth == 1 && keyExpected) {
                        val key = (Json.parseToJsonElement(body.substring(start, index + 1)) as JsonPrimitive).content
                        if (!keys.add(key)) return true
                    }
                }
            }
            index += 1
        }
        return false
    }

    private fun string(value: JsonObject, name: String): String? =
        (value[name] as? JsonPrimitive)?.takeIf { it.isString }?.content

    private fun number(value: JsonObject, name: String): Long? =
        (value[name] as? JsonPrimitive)?.takeUnless { it.isString }?.longOrNull

    private fun decodeGrant(
        response: NativeHttpResponse,
        expectedSession: String? = null,
        spentRefresh: String? = null,
    ): NativeApiResult<NativeGrant> {
        failure(response, mutation = true)?.let { return NativeApiResult.Refused(it) }
        val unknown = NativeApiResult.Refused(NativeApiFailure.UnknownOutcome)
        if (response.status != 200) return unknown
        val value = json(response) ?: return unknown
        if (value.keys != setOf("token_type", "access_token", "refresh_token", "actor_id", "session_id",
                "expires_at", "refresh_expires_at", "absolute_expires_at", "authenticated_at") ||
            string(value, "token_type") != "Bearer") return unknown
        val access = string(value, "access_token") ?: return unknown
        val refresh = string(value, "refresh_token") ?: return unknown
        val actor = string(value, "actor_id") ?: return unknown
        val session = string(value, "session_id") ?: return unknown
        if ((expectedSession != null && session != expectedSession) || refresh == spentRefresh) return unknown
        val expires = number(value, "expires_at") ?: return unknown
        val refreshExpires = number(value, "refresh_expires_at") ?: return unknown
        val absoluteExpires = number(value, "absolute_expires_at") ?: return unknown
        val authenticated = number(value, "authenticated_at")
        if (value["authenticated_at"] != JsonNull && authenticated == null) return unknown
        val receivedAt = now()
        if (expires <= receivedAt) return unknown
        val grant = NativeGrant.create(access, refresh, actor, session, expires, refreshExpires, absoluteExpires, authenticated, receivedAt)
            ?: return unknown
        return NativeApiResult.Accepted(grant)
    }

    private fun decodeSession(response: NativeHttpResponse): NativeApiResult<NativeSession> {
        failure(response, mutation = false)?.let { return NativeApiResult.Refused(it) }
        val invalid = NativeApiResult.Refused(NativeApiFailure.Protocol)
        if (response.status != 200) return invalid
        val value = json(response) ?: return invalid
        if (value.keys != setOf("actor_id", "session_id", "expires_at", "absolute_expires_at", "authenticated_at")) return invalid
        val actor = string(value, "actor_id") ?: return invalid
        val session = string(value, "session_id") ?: return invalid
        val expires = number(value, "expires_at") ?: return invalid
        val absoluteExpires = number(value, "absolute_expires_at") ?: return invalid
        val authenticated = number(value, "authenticated_at")
        if (value["authenticated_at"] != JsonNull && authenticated == null) return invalid
        val receivedAt = now()
        if (receivedAt < 0 || actor.isEmpty() || actor.length > 128 || actor.any { it.isISOControl() } ||
            !NativeGrant.isSessionId(session) || expires <= receivedAt || expires - receivedAt > 120 ||
            absoluteExpires < expires || absoluteExpires - receivedAt > 900 ||
            authenticated?.let { it < 0 || it > receivedAt } == true) return invalid
        return NativeApiResult.Accepted(NativeSession(actor, session, expires, absoluteExpiresAt = absoluteExpires, authenticatedAt = authenticated))
    }

    override suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow> {
        val response = request("POST", "$prefix/flows", buildJsonObject {
            put("state", state); put("nonce", nonce); put("code_challenge", challenge)
        }.toString())
        failure(response, mutation = true)?.let { return NativeApiResult.Refused(it) }
        val unknown = NativeApiResult.Refused(NativeApiFailure.UnknownOutcome)
        if (response.status != 200) return unknown
        val value = json(response) ?: return unknown
        if (value.keys != setOf("flow_id", "authorization_url")) return unknown
        val id = string(value, "flow_id") ?: return unknown
        val url = string(value, "authorization_url") ?: return unknown
        if (!NativeGrant.isSessionId(id) || url.isEmpty() || url.length > 8192) return unknown
        return NativeApiResult.Accepted(NativeFlow(id, url))
    }

    override suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeGrant> {
        if (!NativeGrant.isSessionId(flow)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        return decodeGrant(request("POST", "$prefix/flows/$flow/exchange", buildJsonObject {
            put("state", state); put("code", code); put("code_verifier", verifier)
        }.toString()))
    }

    override suspend fun refresh(refreshToken: String, session: String): NativeApiResult<NativeGrant> {
        if (!NativeGrant.isRefreshToken(refreshToken) || !NativeGrant.isSessionId(session)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        return decodeGrant(request("POST", "$prefix/refresh", familyBody(refreshToken, session)), session, refreshToken)
    }

    private fun familyBody(refreshToken: String, session: String): String = buildJsonObject {
        put("refresh_token", refreshToken); put("session_id", session)
    }.toString()

    private fun cleanup(response: NativeHttpResponse): NativeApiResult<Unit> {
        if (response.failure == null && response.status == 204) return NativeApiResult.Accepted(Unit)
        return NativeApiResult.Refused(failure(response, mutation = true) ?: NativeApiFailure.UnknownOutcome)
    }

    override suspend fun revokeFamily(refreshToken: String, session: String): NativeApiResult<Unit> {
        if (!NativeGrant.isRefreshToken(refreshToken) || !NativeGrant.isSessionId(session)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        return cleanup(request("POST", "$prefix/revoke", familyBody(refreshToken, session)))
    }

    override suspend fun cancel(flow: String, state: String, verifier: String): NativeApiResult<Unit> {
        if (!NativeGrant.isSessionId(flow)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        return cleanup(request("POST", "$prefix/flows/$flow/cancel", buildJsonObject {
            put("state", state); put("code_verifier", verifier)
        }.toString()))
    }

    override suspend fun session(credential: String): NativeApiResult<NativeSession> {
        if (!NativeGrant.isAccessToken(credential)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        return decodeSession(request("GET", "$prefix/session", credential = credential))
    }

    override suspend fun revoke(credential: String): NativeApiResult<Unit> {
        val legacy = Regex("[0-9a-f]{64}").matches(credential)
        val path = when {
            legacy -> "/native/session"
            NativeGrant.isAccessToken(credential) -> "$prefix/session"
            else -> return NativeApiResult.Refused(NativeApiFailure.Protocol)
        }
        val response = request("DELETE", path, credential = credential)
        if (legacy && response.failure == null && response.status == 401) return NativeApiResult.Accepted(Unit)
        return cleanup(response)
    }

    override suspend fun saved(credential: String, session: String): NativeApiResult<Int> {
        if (!NativeGrant.isSessionId(session) || !NativeGrant.isAccessToken(credential)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val response = request("GET", "/library/saved", credential = credential, expectedSession = session)
        if (response.failure == null && (response.status == 409 || (response.status in 200..299 && response.actualSession != session))) {
            return NativeApiResult.Refused(NativeApiFailure.SessionChanged)
        }
        failure(response, mutation = false)?.let { return NativeApiResult.Refused(it) }
        val value = json(response) ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
        if (value.keys != setOf("items", "next_cursor")) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val items = value["items"] as? JsonArray ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
        if (items.size > 50) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val cursor = value["next_cursor"]
        val cursorText = (cursor as? JsonPrimitive)?.takeIf { it.isString }?.content
        if (cursor != JsonNull && (cursorText == null || cursorText.length !in 1..1024)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        for (item in items) {
            val envelope = item as? JsonObject ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
            val saved = envelope["saved"] as? JsonObject ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
            if (envelope.keys != setOf("saved") || saved.keys != setOf("work_id", "saved", "revision", "listener_order", "accepted_at_ms") ||
                string(saved, "work_id")?.let { NativeGrant.isSessionId(it) } != true ||
                (saved["saved"] as? JsonPrimitive)?.takeUnless { it.isString }?.booleanOrNull != true ||
                number(saved, "revision")?.let { it > 0 } != true || number(saved, "listener_order")?.let { it > 0 } != true ||
                number(saved, "accepted_at_ms") == null) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        }
        return NativeApiResult.Accepted(items.size)
    }
}
