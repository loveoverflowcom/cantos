package com.cantos.theatre.adapters.identity

import com.cantos.theatre.domain.identity.LoginSecret
import com.cantos.theatre.domain.identity.NativeClientConfiguration
import com.cantos.theatre.domain.identity.NativeGrant
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
class SecureNativeCredentialJournal(
    private val store: CredentialStore,
    private val apiOrigin: String,
    private val issuer: String,
) : NativeCredentialJournal {
    private val validBinding = NativeClientConfiguration.create(apiOrigin, issuer) != null

    override fun read(): NativeJournalRead {
        if (!validBinding) return NativeJournalRead.Failed(CredentialFailure.Corrupt)
        return when (val result = store.read()) {
            CredentialRead.Missing -> NativeJournalRead.Missing
            is CredentialRead.Failed -> NativeJournalRead.Failed(result.reason)
            is CredentialRead.Found -> decodeRecord(result.value, apiOrigin, issuer)?.let(NativeJournalRead::Found)
                ?: NativeJournalRead.Failed(CredentialFailure.Corrupt)
        }
    }

    override fun write(record: NativeCredentialRecord): CredentialFailure? {
        if (!validBinding) return CredentialFailure.Corrupt
        val encoded = encodeRecord(record, apiOrigin, issuer) ?: return CredentialFailure.Corrupt
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

private fun encodeRecord(record: NativeCredentialRecord, apiOrigin: String, issuer: String): String? {
    when (record) {
        is NativeCredentialRecord.Flow -> if (!journalUuid.matches(record.flow) ||
            LoginSecret.parse(record.state) == null || LoginSecret.parse(record.verifier) == null) return null
        is NativeCredentialRecord.Legacy -> if (!legacyToken.matches(record.accessToken)) return null
        is NativeCredentialRecord.Active, is NativeCredentialRecord.Refreshing, NativeCredentialRecord.Cleared -> Unit
    }
    val encoded = buildJsonObject {
        put("version", 1)
        put("api_origin", apiOrigin)
        put("issuer", issuer)
        put("kind", when (record) {
            is NativeCredentialRecord.Active -> "active"
            is NativeCredentialRecord.Refreshing -> "refreshing"
            is NativeCredentialRecord.Flow -> "flow"
            is NativeCredentialRecord.Legacy -> "legacy"
            NativeCredentialRecord.Cleared -> "cleared"
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
            NativeCredentialRecord.Cleared -> Unit
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

private fun decodeRecord(raw: String, apiOrigin: String, issuer: String): NativeCredentialRecord? {
    if (legacyToken.matches(raw)) return NativeCredentialRecord.Legacy(raw)
    if (raw.encodeToByteArray().size !in 1..32_768) return null
    val value = try { Json.parseToJsonElement(raw) as? JsonObject }
        catch (_: SerializationException) { null }
        catch (_: IllegalArgumentException) { null } ?: return null
    if (number(value, "version") != 1L || string(value, "api_origin") != apiOrigin || string(value, "issuer") != issuer) return null
    val record = when (string(value, "kind")) {
        "active", "refreshing" -> {
            if (value.keys != envelopeFields + grantFields) return null
            val authenticated = value["authenticated_at"]
            if (authenticated != JsonNull && number(value, "authenticated_at") == null) return null
            val grant = NativeGrant.restore(
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
            if (value.keys != envelopeFields + setOf("flow_id", "state", "code_verifier")) return null
            NativeCredentialRecord.Flow(string(value, "flow_id") ?: return null,
                string(value, "state") ?: return null, string(value, "code_verifier") ?: return null)
        }
        "legacy" -> {
            if (value.keys != envelopeFields + "access_token") return null
            NativeCredentialRecord.Legacy(string(value, "access_token") ?: return null)
        }
        "cleared" -> {
            if (value.keys != envelopeFields) return null
            NativeCredentialRecord.Cleared
        }
        else -> return null
    }
    // We only write this canonical representation. Equality also rejects duplicate fields,
    // ambiguous number encodings and trailing/reordered data before restoring an identity.
    return record.takeIf { encodeRecord(it, apiOrigin, issuer) == raw }
}

private fun string(value: JsonObject, key: String): String? =
    (value[key] as? JsonPrimitive)?.takeIf { it.isString }?.content

private fun number(value: JsonObject, key: String): Long? =
    (value[key] as? JsonPrimitive)?.takeUnless { it.isString }?.longOrNull
