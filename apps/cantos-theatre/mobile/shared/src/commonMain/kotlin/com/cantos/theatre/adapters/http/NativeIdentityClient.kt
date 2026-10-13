package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.ports.NativeApiFailure
import com.cantos.theatre.ports.NativeApiResult
import com.cantos.theatre.ports.NativeFlow
import com.cantos.theatre.ports.NativeIdentityApi
import com.cantos.theatre.ports.NativeSession
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import kotlin.coroutines.resume

/** The only wire decoder; real hosts and deterministic common tests use this same client. */
class NativeIdentityClient(private val configuration: NativeClientConfiguration, private val http: NativeHttpPort) : NativeIdentityApi {
    private val uuid = Regex("[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}")
    private suspend fun request(method: String, path: String, body: String? = null, credential: String? = null, expectedSession: String? = null): NativeHttpResponse =
        withTimeoutOrNull(15_000) { suspendCancellableCoroutine { continuation ->
            val handle = http.send(NativeHttpRequest(method, "${configuration.apiOrigin}/api/v1/theatre$path", body, credential, expectedSession)) {
                if (continuation.isActive) continuation.resume(it)
            }
            continuation.invokeOnCancellation { handle.cancel() }
        } } ?: NativeHttpResponse(0, null, null, "timeout")
    private fun failure(response: NativeHttpResponse, mutation: Boolean): NativeApiFailure? = when {
        response.failure != null -> if (mutation) NativeApiFailure.UnknownOutcome else NativeApiFailure.Unavailable
        response.status == 401 -> NativeApiFailure.Unauthorized
        response.status == 403 -> NativeApiFailure.Denied
        response.status !in 200..299 -> NativeApiFailure.Unavailable
        else -> null
    }
    private fun json(response: NativeHttpResponse): JsonObject? {
        val body = response.body ?: return null
        if (body.length > 65536 || response.contentType?.substringBefore(';')?.trim()?.lowercase() != "application/json") return null
        return try { Json.parseToJsonElement(body) as? JsonObject } catch (_: kotlinx.serialization.SerializationException) { null }
            catch (_: IllegalArgumentException) { null }
    }
    private fun string(objectValue: JsonObject, name: String): String? = (objectValue[name] as? JsonPrimitive)?.takeIf { it.isString }?.content
    private fun number(objectValue: JsonObject, name: String): Long? = (objectValue[name] as? JsonPrimitive)?.takeUnless { it.isString }?.longOrNull
    private fun decodeSession(response: NativeHttpResponse, exchanging: Boolean): NativeApiResult<NativeSession> {
        failure(response, exchanging)?.let { return NativeApiResult.Refused(it) }
        val value = json(response) ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val expected = setOf("actor_id", "session_id", "expires_at") + if (exchanging) setOf("access_token") else emptySet()
        if (value.keys != expected) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val actor = string(value, "actor_id") ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val session = string(value, "session_id") ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val expiry = number(value, "expires_at") ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val token = if (exchanging) string(value, "access_token") else null
        if (actor.isEmpty() || actor.length > 128 || actor.any { it.isISOControl() } || !uuid.matches(session) || expiry < 0 ||
            (exchanging && (token == null || !Regex("[0-9a-f]{64}").matches(token)))) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        return NativeApiResult.Accepted(NativeSession(actor, session, expiry, token))
    }
    override suspend fun begin(state: String, nonce: String, challenge: String): NativeApiResult<NativeFlow> {
        val response = request("POST", "/native/auth/flows", buildJsonObject { put("state", state); put("nonce", nonce); put("code_challenge", challenge) }.toString())
        failure(response, true)?.let { return NativeApiResult.Refused(it) }
        val value = json(response) ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
        if (value.keys != setOf("flow_id", "authorization_url")) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val id = string(value, "flow_id") ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val url = string(value, "authorization_url") ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
        if (!uuid.matches(id) || url.length > 8192) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        return NativeApiResult.Accepted(NativeFlow(id, url))
    }
    override suspend fun exchange(flow: String, state: String, code: String, verifier: String): NativeApiResult<NativeSession> {
        if (!uuid.matches(flow)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        return decodeSession(request("POST", "/native/auth/flows/$flow/exchange", buildJsonObject { put("state", state); put("code", code); put("code_verifier", verifier) }.toString()), true)
    }
    override suspend fun session(credential: String): NativeApiResult<NativeSession> {
        if (!Regex("[0-9a-f]{64}").matches(credential)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        return decodeSession(request("GET", "/native/session", credential = credential), false)
    }
    override suspend fun revoke(credential: String): NativeApiResult<Unit> {
        if (!Regex("[0-9a-f]{64}").matches(credential)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val response = request("DELETE", "/native/session", credential = credential)
        if (response.failure == null && response.status in setOf(204, 401)) return NativeApiResult.Accepted(Unit)
        return NativeApiResult.Refused(failure(response, true) ?: NativeApiFailure.Protocol)
    }
    override suspend fun saved(credential: String, session: String): NativeApiResult<Int> {
        if (!uuid.matches(session) || !Regex("[0-9a-f]{64}").matches(credential)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val response = request("GET", "/library/saved", credential = credential, expectedSession = session)
        if (response.failure == null && (response.status == 409 || (response.status in 200..299 && response.actualSession != session))) return NativeApiResult.Refused(NativeApiFailure.SessionChanged)
        failure(response, false)?.let { return NativeApiResult.Refused(it) }
        val value = json(response) ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
        if (value.keys != setOf("items", "next_cursor")) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val items = value["items"] as? kotlinx.serialization.json.JsonArray ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
        if (items.size > 50) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        val cursor = value["next_cursor"]
        val cursorText = (cursor as? JsonPrimitive)?.takeIf { it.isString }?.content
        if (cursor != JsonNull && (cursorText == null || cursorText.length !in 1..1024)) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        for (item in items) {
            val envelope = item as? JsonObject ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
            val saved = envelope["saved"] as? JsonObject ?: return NativeApiResult.Refused(NativeApiFailure.Protocol)
            if (envelope.keys != setOf("saved") || saved.keys != setOf("work_id", "saved", "revision", "listener_order", "accepted_at_ms") ||
                string(saved, "work_id")?.let(uuid::matches) != true || (saved["saved"] as? JsonPrimitive)?.takeUnless { it.isString }?.booleanOrNull != true ||
                number(saved, "revision")?.let { it > 0 } != true || number(saved, "listener_order")?.let { it > 0 } != true ||
                number(saved, "accepted_at_ms") == null) return NativeApiResult.Refused(NativeApiFailure.Protocol)
        }
        return NativeApiResult.Accepted(items.size)
    }
}
