package com.cantos.theatre.adapters.identity

import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeGrant
import com.cantos.theatre.domain.identity.NativeCredentialProtocol
import com.cantos.theatre.domain.identity.NativeIdentityV1Configuration
import com.cantos.theatre.domain.identity.IdentityV1Validation
import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.CredentialRead
import com.cantos.theatre.ports.CredentialStore
import com.cantos.theatre.ports.NativeCredentialJournal
import com.cantos.theatre.ports.NativeCredentialRecord
import com.cantos.theatre.ports.NativeJournalRead
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put

/** The journal is one authenticated OS value, never preferences, SQL or a second token file. */
class SecureNativeCredentialJournal private constructor(
    private val store: CredentialStore,
    private val apiOrigin: String,
    private val issuer: String,
    private val identityV1: Boolean,
) : NativeCredentialJournal {
    constructor(store: CredentialStore, apiOrigin: String, issuer: String) : this(store, apiOrigin, issuer, false)
    constructor(store: CredentialStore, configuration: NativeIdentityV1Configuration) :
        this(store, configuration.apiOrigin, "identity-v1", true)

    private val validBinding = identityV1 || NativeClientConfiguration.create(apiOrigin, issuer) != null

    override fun read(): NativeJournalRead {
        if (!validBinding) return NativeJournalRead.Failed(CredentialFailure.Corrupt)
        return when (val result = store.read()) {
            CredentialRead.Missing -> NativeJournalRead.Missing
            is CredentialRead.Failed -> NativeJournalRead.Failed(result.reason)
            is CredentialRead.Found -> decodeBoundRecord(result.value)?.let(NativeJournalRead::Found)
                ?: NativeJournalRead.Failed(CredentialFailure.Corrupt)
        }
    }

    private fun decodeBoundRecord(raw: String): NativeCredentialRecord? {
        if (!identityV1) return decodeRecord(raw, apiOrigin, issuer)
        decodeRecord(raw, apiOrigin, issuer, true)?.let { return it }
        if (legacyToken.matches(raw)) return NativeCredentialRecord.Legacy(raw)
        // Only our exact canonical v1 envelope at this exact origin can be retired. Unknown,
        // corrupt and foreign records remain quarantined, including behind a pending barrier.
        if (raw.encodeToByteArray().size !in 1..32_768) return null
        val old = parseRecord(raw) ?: return null
        val oldIssuer = string(old, "issuer") ?: return null
        if (NativeClientConfiguration.create(apiOrigin, oldIssuer) == null) return null
        return decodeRecord(raw, apiOrigin, oldIssuer)?.let { NativeCredentialRecord.RetiredProvider }
    }

    override fun write(record: NativeCredentialRecord): CredentialFailure? {
        if (!validBinding) return CredentialFailure.Corrupt
        val encoded = encodeRecord(record, apiOrigin, issuer, identityV1) ?: return CredentialFailure.Corrupt
        store.write(encoded)?.let { return it }
        return when (val result = store.read()) {
            CredentialRead.Missing -> CredentialFailure.Unavailable
            is CredentialRead.Failed -> result.reason
            is CredentialRead.Found -> if (result.value == encoded) null else CredentialFailure.Corrupt
        }
    }

    override fun clear(): CredentialFailure? {
        store.clear()?.let { return it }
        return when (val result = store.read()) {
            CredentialRead.Missing -> null
            is CredentialRead.Failed -> result.reason
            is CredentialRead.Found -> CredentialFailure.Unavailable
        }
    }
}

private val journalUuid = Regex("[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}")
private val legacyToken = Regex("[0-9a-f]{64}")
private val envelopeFields = setOf("version", "api_origin", "issuer", "kind")
private val grantFields = setOf("access_token", "refresh_token", "actor_id", "session_id", "expires_at",
    "refresh_expires_at", "absolute_expires_at", "authenticated_at")

private fun encodeRecord(record: NativeCredentialRecord, apiOrigin: String, issuer: String, identityV1: Boolean = false): String? {
    val pair = when (record) {
        is NativeCredentialRecord.Active -> record.grant
        is NativeCredentialRecord.Refreshing -> record.grant
        else -> null
    }
    if (pair != null && pair.protocol != if (identityV1) NativeCredentialProtocol.IdentityV1 else NativeCredentialProtocol.ProviderBearerV1) return null
    if (record == NativeCredentialRecord.RetiredProvider || (identityV1 && record is NativeCredentialRecord.Flow)) return null
    when (record) {
        is NativeCredentialRecord.Flow -> if (!journalUuid.matches(record.flow) ||
            LoginSecret.parse(record.state) == null || LoginSecret.parse(record.verifier) == null) return null
        is NativeCredentialRecord.Legacy -> if (!legacyToken.matches(record.accessToken)) return null
        is NativeCredentialRecord.Active, is NativeCredentialRecord.Refreshing, NativeCredentialRecord.Cleared, NativeCredentialRecord.RetiredProvider -> Unit
    }
    val encoded = buildJsonObject {
        put("version", if (identityV1) 2 else 1)
        put("api_origin", apiOrigin)
        put(if (identityV1) "protocol" else "issuer", issuer)
        put("kind", when (record) {
            is NativeCredentialRecord.Active -> "active"
            is NativeCredentialRecord.Refreshing -> "refreshing"
            is NativeCredentialRecord.Flow -> "flow"
            is NativeCredentialRecord.Legacy -> "legacy"
            NativeCredentialRecord.Cleared -> "cleared"
            NativeCredentialRecord.RetiredProvider -> "retired"
        })
        when (record) {
            is NativeCredentialRecord.Active -> grant(record.grant)
            is NativeCredentialRecord.Refreshing -> grant(record.grant)
            is NativeCredentialRecord.Flow -> {
                put("flow_id", record.flow)
                put("state", record.state)
                put("code_verifier", record.verifier)
            }
            is NativeCredentialRecord.Legacy -> put("access_token", record.accessToken)
            NativeCredentialRecord.Cleared, NativeCredentialRecord.RetiredProvider -> Unit
        }
    }.toString()
    return encoded.takeIf { it.encodeToByteArray().size <= 32_768 }
}

private fun kotlinx.serialization.json.JsonObjectBuilder.grant(value: NativeGrant) {
    put("access_token", value.accessToken)
    put("refresh_token", value.refreshToken)
    put("actor_id", value.view.actor)
    put("session_id", value.view.session)
    put("expires_at", value.expiresAt)
    put("refresh_expires_at", value.refreshExpiresAt)
    put("absolute_expires_at", value.absoluteExpiresAt)
    val authenticatedAt = value.authenticatedAt
    if (authenticatedAt == null) put("authenticated_at", JsonNull) else put("authenticated_at", authenticatedAt)
}

private fun decodeRecord(raw: String, apiOrigin: String, issuer: String, identityV1: Boolean = false): NativeCredentialRecord? {
    if (legacyToken.matches(raw)) return NativeCredentialRecord.Legacy(raw)
    if (raw.encodeToByteArray().size !in 1..32_768) return null
    val value = parseRecord(raw) ?: return null
    val binding = if (identityV1) "protocol" else "issuer"
    val fields = if (identityV1) envelopeFields - "issuer" + "protocol" else envelopeFields
    if (number(value, "version") != (if (identityV1) 2L else 1L) || string(value, "api_origin") != apiOrigin || string(value, binding) != issuer) return null
    val record = when (string(value, "kind")) {
        "active", "refreshing" -> {
            if (value.keys != fields + grantFields) return null
            val authenticated = value["authenticated_at"]
            if (authenticated != JsonNull && number(value, "authenticated_at") == null) return null
            val access = string(value, "access_token") ?: return null
            val refresh = string(value, "refresh_token") ?: return null
            val grant = if (identityV1) {
                when (val checked = NativeGrant.restoreIdentityV1(access, refresh,
                    string(value, "actor_id") ?: return null, string(value, "session_id") ?: return null,
                    number(value, "expires_at") ?: return null, number(value, "refresh_expires_at") ?: return null,
                    number(value, "absolute_expires_at") ?: return null, number(value, "authenticated_at") ?: return null)) {
                    is IdentityV1Validation.Accepted -> checked.value
                    is IdentityV1Validation.Refused -> return null
                }
            } else NativeGrant.restore(
                string(value, "access_token") ?: return null,
                string(value, "refresh_token") ?: return null,
                string(value, "actor_id") ?: return null,
                string(value, "session_id") ?: return null,
                number(value, "expires_at") ?: return null,
                number(value, "refresh_expires_at") ?: return null,
                number(value, "absolute_expires_at") ?: return null,
                number(value, "authenticated_at"),
            ) ?: return null
            if (string(value, "kind") == "active") NativeCredentialRecord.Active(grant)
            else NativeCredentialRecord.Refreshing(grant)
        }
        "flow" -> {
            if (value.keys != fields + setOf("flow_id", "state", "code_verifier")) return null
            NativeCredentialRecord.Flow(string(value, "flow_id") ?: return null,
                string(value, "state") ?: return null, string(value, "code_verifier") ?: return null)
        }
        "legacy" -> {
            if (value.keys != fields + "access_token") return null
            NativeCredentialRecord.Legacy(string(value, "access_token") ?: return null)
        }
        "cleared" -> {
            if (value.keys != fields) return null
            NativeCredentialRecord.Cleared
        }
        else -> return null
    }
    // We only write this canonical representation. Equality also rejects duplicate fields,
    // ambiguous number encodings and trailing/reordered data before restoring an identity.
    return record.takeIf { encodeRecord(it, apiOrigin, issuer, identityV1) == raw }
}

private fun string(value: JsonObject, key: String): String? =
    (value[key] as? JsonPrimitive)?.takeIf { it.isString }?.content

private fun number(value: JsonObject, key: String): Long? =
    (value[key] as? JsonPrimitive)?.takeUnless { it.isString }?.longOrNull

private fun parseRecord(raw: String): JsonObject? = try { Json.parseToJsonElement(raw) as? JsonObject }
    catch (_: SerializationException) { null }
    catch (_: IllegalArgumentException) { null }
