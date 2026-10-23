package com.cantos.theatre.adapters.identity

import com.cantos.theatre.adapters.http.ApiOrigin
import com.cantos.theatre.domain.identity.IdentityV1Validation
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

/**
 * The journal is one authenticated OS value, never preferences, SQL or a second token file. It
 * writes only the version-2 Identity v1 envelope bound to the exact API origin. A canonical
 * record of a retired protocol at this origin is recognized for local removal and relogin
 * ([NativeCredentialRecord.Retired]); anything else unreadable is [CredentialFailure.Corrupt]
 * and is never overwritten to manufacture a recovery path.
 */
class SecureNativeCredentialJournal(
    private val store: CredentialStore,
    origin: ApiOrigin,
) : NativeCredentialJournal {
    private val apiOrigin = origin.toString()

    override fun read(): NativeJournalRead = when (val result = store.read()) {
        CredentialRead.Missing -> NativeJournalRead.Missing
        is CredentialRead.Failed -> NativeJournalRead.Failed(result.reason)
        is CredentialRead.Found -> decodeBoundRecord(result.value)?.let(NativeJournalRead::Found)
            ?: NativeJournalRead.Failed(CredentialFailure.Corrupt)
    }

    private fun decodeBoundRecord(raw: String): NativeCredentialRecord? =
        decodeRecord(raw, apiOrigin) ?: NativeCredentialRecord.Retired.takeIf { isRetiredRecord(raw, apiOrigin) }

    override fun write(record: NativeCredentialRecord): CredentialFailure? {
        val encoded = encodeRecord(record, apiOrigin) ?: return CredentialFailure.Corrupt
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

private const val MaxRecordBytes = 32_768
private const val Protocol = "identity-v1"
private val legacyToken = Regex("[0-9a-f]{64}")
private val envelopeFields = setOf("version", "api_origin", "protocol", "kind")
private val grantFields = setOf("access_token", "refresh_token", "actor_id", "session_id", "expires_at",
    "refresh_expires_at", "absolute_expires_at", "authenticated_at")

private fun encodeRecord(record: NativeCredentialRecord, apiOrigin: String): String? {
    val kind = when (record) {
        is NativeCredentialRecord.Active -> "active"
        is NativeCredentialRecord.Refreshing -> "refreshing"
        NativeCredentialRecord.Cleared -> "cleared"
        // A retired record is only ever recognized and removed, never written.
        NativeCredentialRecord.Retired -> return null
    }
    val encoded = buildJsonObject {
        put("version", 2)
        put("api_origin", apiOrigin)
        put("protocol", Protocol)
        put("kind", kind)
        when (record) {
            is NativeCredentialRecord.Active -> grant(record.grant)
            is NativeCredentialRecord.Refreshing -> grant(record.grant)
            NativeCredentialRecord.Cleared, NativeCredentialRecord.Retired -> Unit
        }
    }.toString()
    return encoded.takeIf { it.encodeToByteArray().size <= MaxRecordBytes }
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

private fun decodeRecord(raw: String, apiOrigin: String): NativeCredentialRecord? {
    if (raw.encodeToByteArray().size !in 1..MaxRecordBytes) return null
    val value = parseRecord(raw) ?: return null
    if (number(value, "version") != 2L || string(value, "api_origin") != apiOrigin ||
        string(value, "protocol") != Protocol) return null
    val record = when (string(value, "kind")) {
        "active", "refreshing" -> {
            if (value.keys != envelopeFields + grantFields) return null
            val grant = when (val checked = NativeGrant.restoreIdentityV1(
                string(value, "access_token") ?: return null, string(value, "refresh_token") ?: return null,
                string(value, "actor_id") ?: return null, string(value, "session_id") ?: return null,
                number(value, "expires_at") ?: return null, number(value, "refresh_expires_at") ?: return null,
                number(value, "absolute_expires_at") ?: return null, number(value, "authenticated_at") ?: return null,
            )) {
                is IdentityV1Validation.Accepted -> checked.value
                is IdentityV1Validation.Refused -> return null
            }
            if (string(value, "kind") == "active") NativeCredentialRecord.Active(grant)
            else NativeCredentialRecord.Refreshing(grant)
        }
        "cleared" -> {
            if (value.keys != envelopeFields) return null
            NativeCredentialRecord.Cleared
        }
        else -> return null
    }
    // We only write this canonical representation. Equality also rejects duplicate fields,
    // ambiguous number encodings and trailing/reordered data before restoring an identity.
    return record.takeIf { encodeRecord(it, apiOrigin) == raw }
}

// Recognition-only shapes of the retired provider journal (an issuer-bound version-1 envelope).
// Nothing below restores a credential: a match only permits local removal and a fresh login.
private val retiredIssuer = Regex("https://[a-z0-9.-]+(:[0-9]{1,5})?/oauth2/openid/cantos-theatre-native")
private val retiredAccess = Regex("ca1_n_[0-9a-f]{64}")
private val retiredRefresh = Regex("cr1_[0-9a-f]{64}")
private val retiredUuid = Regex("[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}")
private val retiredSecret = Regex("[A-Za-z0-9._~-]{43,128}")
private val retiredEnvelopeFields = setOf("version", "api_origin", "issuer", "kind")

/**
 * Only an exact canonical retired envelope at this exact origin, or the unbound pre-provider
 * opaque token, is recognized. Unknown, corrupt and foreign values stay quarantined.
 */
private fun isRetiredRecord(raw: String, apiOrigin: String): Boolean {
    if (legacyToken.matches(raw)) return true
    if (raw.encodeToByteArray().size !in 1..MaxRecordBytes) return false
    val value = parseRecord(raw) ?: return false
    if (number(value, "version") != 1L || string(value, "api_origin") != apiOrigin) return false
    val issuer = string(value, "issuer") ?: return false
    if (issuer.length > 2048 || !retiredIssuer.matches(issuer)) return false
    val shape = when (string(value, "kind")) {
        "active", "refreshing" -> value.keys == retiredEnvelopeFields + grantFields &&
            string(value, "access_token")?.let(retiredAccess::matches) == true &&
            string(value, "refresh_token")?.let(retiredRefresh::matches) == true &&
            string(value, "actor_id")?.let { it.length in 1..128 && it.none(Char::isISOControl) } == true &&
            string(value, "session_id")?.let(retiredUuid::matches) == true &&
            listOf("expires_at", "refresh_expires_at", "absolute_expires_at").all { number(value, it) != null } &&
            (value["authenticated_at"] == JsonNull || number(value, "authenticated_at") != null)
        "flow" -> value.keys == retiredEnvelopeFields + setOf("flow_id", "state", "code_verifier") &&
            string(value, "flow_id")?.let(retiredUuid::matches) == true &&
            string(value, "state")?.let(retiredSecret::matches) == true &&
            string(value, "code_verifier")?.let(retiredSecret::matches) == true
        "legacy" -> value.keys == retiredEnvelopeFields + "access_token" &&
            string(value, "access_token")?.let(legacyToken::matches) == true
        "cleared" -> value.keys == retiredEnvelopeFields
        else -> false
    }
    // Our writer emitted JsonObject.toString(); a round trip rejects whitespace, escapes and duplicates.
    return shape && value.toString() == raw
}

private fun string(value: JsonObject, key: String): String? =
    (value[key] as? JsonPrimitive)?.takeIf { it.isString }?.content

private fun number(value: JsonObject, key: String): Long? =
    (value[key] as? JsonPrimitive)?.takeUnless { it.isString }?.longOrNull

private fun parseRecord(raw: String): JsonObject? = try { Json.parseToJsonElement(raw) as? JsonObject }
    catch (_: SerializationException) { null }
    catch (_: IllegalArgumentException) { null }
