package com.cantos.theatre.adapters.http

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/** An origin, never a URL with credentials, a signed URL or a Studio route. */
class ApiOrigin private constructor(private val value: String) {
    val probeUrl: String get() = "$value/api/v1/theatre"
    /** False only for the explicit loopback `http://` development origins admitted below. */
    val isHttps: Boolean get() = value.startsWith("https://")
    override fun equals(other: Any?): Boolean = other is ApiOrigin && value == other.value
    override fun hashCode(): Int = value.hashCode()
    override fun toString(): String = value
    companion object {
        fun parse(value: String): ApiOrigin {
            val match = Regex("^(https?)://([a-zA-Z0-9.-]+|\\[::1\\])(?::([0-9]{1,5}))?$").matchEntire(value)
                ?: throw IllegalArgumentException("API origin must contain only scheme, host and optional port")
            val host = match.groupValues[2].lowercase()
            require(!host.startsWith('.') && !host.endsWith('.') && !host.contains(".."))
            require(match.groupValues[1] == "https" || host in setOf("localhost", "127.0.0.1", "[::1]")) {
                "HTTP is limited to explicit loopback development"
            }
            val port = match.groupValues[3]
            require(port.isEmpty() || port.toInt() in 1..65535)
            return ApiOrigin(value)
        }
    }
}
/** Closed GET-only plan. No cookies, bearer headers or listener DTOs exist to pass here. */
class ProbeRequest internal constructor(val url: String)
data class ProbeResponse(val status: Int, val contentType: String?, val body: String)
fun interface HttpTransport { suspend fun get(request: ProbeRequest): ProbeResponse }
enum class UnsupportedCapability { ListenerApi, Publication, Playback, Downloads, Progress, Identity }
enum class HttpFailure { Transport, Protocol, Unauthenticated, Forbidden, Unavailable, CookieRuntime }
sealed interface ListenerOutcome {
    data object Idle : ListenerOutcome
    data object Pending : ListenerOutcome
    data class Unsupported(val capability: UnsupportedCapability) : ListenerOutcome
    data class Failed(val failure: HttpFailure) : ListenerOutcome
}

/** Probe the existing reserved group, without inventing a catalog/manifest endpoint. */
class ListenerClient(private val origin: ApiOrigin, private val transport: HttpTransport) {
    fun capability(capability: UnsupportedCapability): ListenerOutcome = ListenerOutcome.Unsupported(capability)
    suspend fun probe(): ListenerOutcome = try {
        classifyProbe(transport.get(ProbeRequest(origin.probeUrl)))
    } catch (cancelled: CancellationException) {
        throw cancelled
    } catch (_: CookieRuntimeException) {
        ListenerOutcome.Failed(HttpFailure.CookieRuntime)
    } catch (_: Exception) {
        ListenerOutcome.Failed(HttpFailure.Transport)
    }
}
class CookieRuntimeException : IllegalStateException("Listener transport refuses an ambient cookie runtime")

fun classifyProbe(response: ProbeResponse): ListenerOutcome {
    if (response.body.length > 8192 || response.contentType?.substringBefore(';')?.trim()?.lowercase() != "application/json") {
        return ListenerOutcome.Failed(HttpFailure.Protocol)
    }
    return when (response.status) {
        404 -> if (isNotFoundEnvelope(response.body)) ListenerOutcome.Unsupported(UnsupportedCapability.ListenerApi)
            else ListenerOutcome.Failed(HttpFailure.Protocol)
        401 -> ListenerOutcome.Failed(HttpFailure.Unauthenticated)
        403 -> ListenerOutcome.Failed(HttpFailure.Forbidden)
        503 -> ListenerOutcome.Failed(HttpFailure.Unavailable)
        else -> ListenerOutcome.Failed(HttpFailure.Protocol)
    }
}
private fun isNotFoundEnvelope(body: String): Boolean = try {
    val json = Json.parseToJsonElement(body) as? JsonObject
    json != null && json.keys == setOf("code", "current_revision", "issues") &&
        json["code"] == JsonPrimitive("not_found") && json["current_revision"] == JsonNull &&
        json["issues"]?.toString() == "[]"
} catch (_: Exception) { false }

/** Separate live authority; never seeds the mock catalog or creates a second media engine. */
data class ProbeState(val generation: Long = 0, val outcome: ListenerOutcome = ListenerOutcome.Idle)
class ListenerProbeController(private val scope: CoroutineScope, private val client: ListenerClient) {
    private val mutableState = MutableStateFlow(ProbeState())
    val state: StateFlow<ProbeState> = mutableState.asStateFlow()
    private var request: Job? = null
    private var closed = false
    private fun advance(): Long {
        check(mutableState.value.generation < Long.MAX_VALUE) { "Listener request generation exhausted" }
        return mutableState.value.generation + 1
    }
    fun refresh() {
        if (closed) return
        request?.cancel()
        val generation = advance()
        mutableState.value = ProbeState(generation, ListenerOutcome.Pending)
        request = scope.launch {
            val outcome = client.probe()
            if (!closed && mutableState.value.generation == generation) mutableState.value = ProbeState(generation, outcome)
        }
    }
    /** Future listener account changes fence old responses; no Studio session is accepted. */
    fun sessionChanged() {
        if (closed) return
        request?.cancel()
        mutableState.value = ProbeState(advance(), ListenerOutcome.Idle)
    }
    fun close() { if (!closed) { sessionChanged(); closed = true } }
}
