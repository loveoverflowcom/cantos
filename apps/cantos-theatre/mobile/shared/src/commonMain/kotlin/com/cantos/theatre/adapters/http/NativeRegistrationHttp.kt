package com.cantos.theatre.adapters.http

import com.cantos.theatre.adapters.http.registrationwire.Accepted
import com.cantos.theatre.adapters.http.registrationwire.EmailRequest
import com.cantos.theatre.adapters.http.registrationwire.FirebaseConfirmRequest
import com.cantos.theatre.adapters.http.registrationwire.FirebaseConfirmation
import com.cantos.theatre.adapters.http.registrationwire.FirebaseConfirmationSetupReady
import com.cantos.theatre.adapters.http.registrationwire.FirebaseConfirmationUnavailable
import com.cantos.theatre.adapters.http.registrationwire.NativeConfiguration
import com.cantos.theatre.adapters.http.registrationwire.NativeConfigurationFirebaseEmailVerification
import com.cantos.theatre.adapters.http.registrationwire.NativeConfigurationUnavailable
import com.cantos.theatre.domain.identity.RegistrationAction
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationFailure
import com.cantos.theatre.domain.identity.RegistrationSetup
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.RegistrationCapability
import com.cantos.theatre.ports.RegistrationPort
import com.cantos.theatre.domain.identity.RegistrationResult
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.SerializationException
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlin.coroutines.resume

/** Same adapter on every host. No Origin, cookie, bearer, redirects or consuming retries. */
class NativeRegistrationHttp(
    private val apiOrigin: ApiOrigin,
    private val issuerOrigin: String,
    private val http: NativeHttpPort,
) : RegistrationPort {
    private val json = Json { classDiscriminator = "status" }
    private val prefix = "$apiOrigin/api/v1/theatre/native/auth/registration"

    private suspend fun request(path: String, body: String? = null): NativeHttpResponse = withTimeoutOrNull(15_000) {
        suspendCancellableCoroutine { continuation ->
            val handle = http.send(NativeHttpRequest(if (body == null) "GET" else "POST", prefix + path, body, null)) {
                if (continuation.isActive) continuation.resume(it)
            }
            continuation.invokeOnCancellation { handle.cancel() }
        }
    } ?: NativeHttpResponse(0, null, null, "timeout")

    private fun failure(response: NativeHttpResponse, mutation: Boolean): RegistrationFailure? = when {
        response.failure != null || response.status == 0 -> if (mutation) RegistrationFailure.UnknownOutcome else RegistrationFailure.Unavailable
        response.status == 404 -> RegistrationFailure.Disabled
        response.status == 403 -> RegistrationFailure.Denied
        response.status == 400 || response.status == 415 || response.status == 413 -> RegistrationFailure.Protocol
        response.status in 200..299 -> null
        else -> if (mutation) RegistrationFailure.UnknownOutcome else RegistrationFailure.Unavailable
    }

    private inline fun <reified T> decode(response: NativeHttpResponse): T? {
        val body = response.body ?: return null
        if (body.length > 16_384 || response.contentType?.substringBefore(';')?.trim()?.lowercase() != "application/json") return null
        return try {
            val objectValue = json.parseToJsonElement(body) as? JsonObject ?: return null
            if (repeatedTopLevelKeys(body) || objectValue.values.any { it !is JsonPrimitive || !it.isString }) return null
            json.decodeFromString<T>(body)
        } catch (_: SerializationException) { null }
          catch (_: IllegalArgumentException) { null }
    }

    override suspend fun configuration(): RegistrationResult<RegistrationCapability> {
        val response = request("/configuration")
        failure(response, false)?.let { return RegistrationResult.Refused(it) }
        if (response.status != 200) return RegistrationResult.Refused(RegistrationFailure.Protocol)
        return when (val value = decode<NativeConfiguration>(response)) {
            is NativeConfigurationFirebaseEmailVerification -> {
                val selected = selectCatalogOrigin(value.action_origin, allowLoopbackHttp = false)
                if (selected !is CatalogOriginSelection.Configured) RegistrationResult.Refused(RegistrationFailure.Protocol)
                else RegistrationResult.Accepted(RegistrationCapability.Available(value.action_origin))
            }
            is NativeConfigurationUnavailable -> RegistrationResult.Accepted(RegistrationCapability.Disabled)
            null -> RegistrationResult.Refused(RegistrationFailure.Protocol)
        }
    }

    override suspend fun start(email: RegistrationEmail): RegistrationResult<Unit> {
        val response = request("", json.encodeToString(EmailRequest(email.value)))
        failure(response, true)?.let { return RegistrationResult.Refused(it) }
        return if (response.status == 202 && decode<Accepted>(response) != null) RegistrationResult.Accepted(Unit)
        else RegistrationResult.Refused(RegistrationFailure.UnknownOutcome)
    }

    private fun actionBody(action: RegistrationAction): String = json.encodeToString(FirebaseConfirmRequest(action.operation, action.code))

    override suspend fun confirm(action: RegistrationAction): RegistrationResult<RegistrationSetup> {
        val response = request("/firebase/confirm", actionBody(action))
        failure(response, true)?.let { return RegistrationResult.Refused(it) }
        if (response.status != 200) return RegistrationResult.Refused(RegistrationFailure.UnknownOutcome)
        return when (val value = decode<FirebaseConfirmation>(response)) {
            is FirebaseConfirmationSetupReady -> {
                when (val setup = RegistrationSetup.parse(value.login, value.setup_url, issuerOrigin)) {
                    is RegistrationResult.Accepted -> setup
                    is RegistrationResult.Refused -> RegistrationResult.Refused(RegistrationFailure.UnknownOutcome)
                }
            }
            is FirebaseConfirmationUnavailable -> RegistrationResult.Refused(RegistrationFailure.ProofUnavailable)
            null -> RegistrationResult.Refused(RegistrationFailure.UnknownOutcome)
        }
    }

    override suspend fun cancel(action: RegistrationAction): RegistrationResult<Unit> {
        val response = request("/firebase/cancel", actionBody(action))
        failure(response, true)?.let { return RegistrationResult.Refused(it) }
        return if (response.status == 202 && decode<Accepted>(response) != null) RegistrationResult.Accepted(Unit)
        else RegistrationResult.Refused(RegistrationFailure.UnknownOutcome)
    }
}
