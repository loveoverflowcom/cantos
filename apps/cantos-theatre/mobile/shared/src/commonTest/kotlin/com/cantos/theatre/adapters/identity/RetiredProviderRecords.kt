package com.cantos.theatre.adapters.identity

import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObjectBuilder
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * Synthetic bytes of the retired provider journal (version-1, issuer-bound envelope) and of the
 * pre-provider opaque token. The production journal can no longer write or restore these; devices
 * that still hold them must retire them locally and require a fresh Identity v1 login.
 */
internal object RetiredProviderRecords {
    const val Issuer = "https://idm.example/oauth2/openid/cantos-theatre-native"
    const val Session = "00000000-0000-4000-8000-000000000001"

    /** Unbound old token record: exactly 64 hex characters, no envelope. */
    fun opaqueToken(letter: Char = 'a') = letter.toString().repeat(64)

    private fun envelope(origin: String, kind: String, issuer: String, body: JsonObjectBuilder.() -> Unit = {}) =
        buildJsonObject {
            put("version", 1)
            put("api_origin", origin)
            put("issuer", issuer)
            put("kind", kind)
            body()
        }.toString()

    private fun JsonObjectBuilder.pair(letter: Char) {
        put("access_token", "ca1_n_" + letter.toString().repeat(64))
        put("refresh_token", "cr1_" + letter.toString().repeat(64))
        put("actor_id", "reader-a")
        put("session_id", Session)
        put("expires_at", 220)
        put("refresh_expires_at", 700)
        put("absolute_expires_at", 1000)
        put("authenticated_at", 90)
    }

    fun active(origin: String, letter: Char = 'c', issuer: String = Issuer) = envelope(origin, "active", issuer) { pair(letter) }
    fun refreshing(origin: String, letter: Char = 'c', issuer: String = Issuer) = envelope(origin, "refreshing", issuer) { pair(letter) }
    fun activeWithoutAuthTime(origin: String, issuer: String = Issuer) = envelope(origin, "active", issuer) {
        pair('c'); put("authenticated_at", JsonNull)
    }
    fun flow(origin: String, issuer: String = Issuer) = envelope(origin, "flow", issuer) {
        put("flow_id", Session)
        put("state", "e".repeat(64))
        put("code_verifier", "f".repeat(64))
    }
    fun legacyKind(origin: String, issuer: String = Issuer) = envelope(origin, "legacy", issuer) { put("access_token", opaqueToken()) }
    fun cleared(origin: String, issuer: String = Issuer) = envelope(origin, "cleared", issuer)
}
