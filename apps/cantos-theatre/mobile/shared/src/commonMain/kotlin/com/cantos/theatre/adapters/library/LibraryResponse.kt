package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.LibraryError
import com.cantos.theatre.ports.NativeHttpResponse

/** Pure boundary decision. Only null permits decoding an account-owned payload. */
internal fun libraryResponseFailure(
    response: NativeHttpResponse,
    expectedSession: String,
    mutation: Boolean,
): LibraryResult.Failed? {
    when (response.failure) {
        "authority_changed" -> return LibraryResult.Failed("authority_changed", true, mutation)
        "session_changed", "unauthenticated", "denied", "protocol" ->
            return LibraryResult.Failed(response.failure, false, mutation)
        null -> Unit
        else -> return LibraryResult.Failed("unavailable", true, mutation)
    }
    if (response.status == 401) return LibraryResult.Failed("unauthenticated", false)
    if (response.actualSession != null && response.actualSession != expectedSession) {
        return LibraryResult.Failed("session_changed", false, mutation)
    }
    val raw = response.body
    if (raw == null || raw.encodeToByteArray().size > 1_048_576 ||
        response.contentType?.substringBefore(';')?.trim()?.lowercase() != "application/json") {
        return LibraryResult.Failed("protocol", false, mutation)
    }
    val error = LibraryCodec.decode<LibraryError>("LibraryError", raw)
    // Identity auth v1 + identity_auth::admit_theatre: admission can fail before
    // there is a session to stamp. This exact Library error grants no body/account authority.
    if (response.actualSession == null) {
        return if (response.status == 503 && error?.code == "unavailable" && error.retryable) {
            LibraryResult.Failed("unavailable", true)
        } else LibraryResult.Failed("protocol", false, mutation)
    }
    if (response.status == 200 || response.status == 409 && error == null) return null
    val contractedStatus = when (error?.code) {
        "invalid_request", "invalid_cursor" -> 400
        "unauthenticated" -> 401
        "not_found" -> 404
        "invalid_position", "idempotency_key_reused" -> 422
        "stale_device_sequence", "revision_exhausted" -> 409
        "unavailable" -> 503
        "corrupt_state" -> 500
        else -> null
    }
    return if (error != null && response.status == contractedStatus && error.retryable == (response.status == 503)) {
        LibraryResult.Failed(error.code, error.retryable)
    } else LibraryResult.Failed("protocol", false, mutation)
}
