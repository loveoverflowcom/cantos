package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.playback.AudioIdentity
import com.cantos.theatre.ports.NativeHttpFailure
import com.cantos.theatre.ports.NativeHttpPort
import com.cantos.theatre.ports.NativeHttpRequest
import com.cantos.theatre.ports.NativeHttpResponse
import com.cantos.theatre.ports.PlaybackAccessFailure
import com.cantos.theatre.ports.PlaybackAccessPort
import com.cantos.theatre.ports.PlaybackAccessResult
import com.cantos.theatre.ports.PlaybackGrant
import com.cantos.theatre.ports.PlaybackLocation
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlin.coroutines.resume

/** Discovery v1 DTOs are drift-checked against the committed schema and manifest fixture. */
@Serializable
internal data class ManifestWire(
    @SerialName("schema_version") val version: String,
    @SerialName("publication_id") val publication: String,
    @SerialName("work_id") val work: String,
    @SerialName("unit_id") val unit: String,
    @SerialName("rendition_id") val rendition: String,
    @SerialName("manifest_digest") val digest: String,
    @SerialName("sha256") val hash: String,
    @SerialName("byte_length") @Serializable(with = StrictJsonLong::class) val bytes: Long,
    @SerialName("duration_ms") @Serializable(with = StrictJsonLong::class) val duration: Long,
    val container: String,
    val codec: String,
    @Serializable(with = StrictJsonLong::class) val channels: Long,
    @SerialName("sample_rate_hz") @Serializable(with = StrictJsonLong::class) val rate: Long,
    @SerialName("profile_id") val profile: String,
    val language: String,
    @SerialName("published_at_ms") @Serializable(with = StrictJsonLong::class) val published: Long,
)

@Serializable
internal class PlaybackGrantWire(
    @SerialName("publication_id") val publication: String,
    @SerialName("rendition_id") val rendition: String,
    @SerialName("sha256") val hash: String,
    val url: String,
    @SerialName("expires_at_ms") @Serializable(with = StrictJsonLong::class) val expires: Long,
    @SerialName("content_type") val contentType: String,
    @SerialName("byte_range_supported") @Serializable(with = StrictJsonBoolean::class) val ranges: Boolean,
)

/** Only small public JSON uses this port. Media bytes go directly to the native decoder. */
class PlaybackAccessHttp(
    private val origin: ApiOrigin,
    private val http: NativeHttpPort,
    private val nowMs: () -> Long,
) : PlaybackAccessPort {
    private suspend fun request(method: String, publication: String, suffix: String = ""): NativeHttpResponse =
        withTimeoutOrNull(15_000) {
            suspendCancellableCoroutine { continuation ->
                val handle = http.send(NativeHttpRequest(method,
                    "$origin/api/v1/theatre/publications/$publication$suffix", if (method == "POST") "{}" else null, null)) {
                    if (continuation.isActive) continuation.resume(it)
                }
                continuation.invokeOnCancellation { handle.cancel() }
            }
        } ?: NativeHttpResponse(0, null, null, NativeHttpFailure.Timeout)

    private fun refusal(response: NativeHttpResponse): PlaybackAccessResult.Refused? = when {
        response.failure != null -> PlaybackAccessResult.Refused(PlaybackAccessFailure.Network)
        response.status == 404 -> PlaybackAccessResult.Refused(PlaybackAccessFailure.NotFound)
        response.status == 503 -> PlaybackAccessResult.Refused(PlaybackAccessFailure.Unavailable)
        response.status != 200 || response.body == null || response.body.length > 65_536 ||
            response.contentType?.substringBefore(';')?.trim()?.lowercase() != "application/json" ->
            PlaybackAccessResult.Refused(PlaybackAccessFailure.Protocol)
        else -> null
    }

    override suspend fun manifest(publicationId: String): PlaybackAccessResult<AudioIdentity> {
        if (!canonicalId(publicationId)) return PlaybackAccessResult.Refused(PlaybackAccessFailure.Protocol)
        val response = request("GET", publicationId)
        refusal(response)?.let { return it }
        val wire = try { catalogJson.decodeFromString<ManifestWire>(response.body ?: "") }
            catch (_: SerializationException) { return PlaybackAccessResult.Refused(PlaybackAccessFailure.Protocol) }
        val identity = AudioIdentity.of(wire.work, wire.unit, wire.publication, wire.rendition, wire.hash, wire.duration)
        if (wire.version != "1" || wire.publication != publicationId || identity == null || wire.bytes !in 1..9_007_199_254_740_991L ||
            wire.container != "ogg" || wire.codec != "opus" || wire.channels != 1L || wire.rate != 48_000L ||
            wire.profile !in setOf("opus-mono-20-v1", "opus-mono-24-v1", "opus-mono-32-v1", "opus-mono-40-v1") ||
            !wire.digest.matches(Regex("rm-r1:sha256:[0-9a-f]{64}")) || wire.published !in 0..9_007_199_254_740_991L || wire.language.isBlank() || wire.language.length > 63) {
            return PlaybackAccessResult.Refused(PlaybackAccessFailure.Protocol)
        }
        return PlaybackAccessResult.Accepted(identity)
    }

    override suspend fun authorize(identity: AudioIdentity): PlaybackAccessResult<PlaybackGrant> {
        // Re-read the exact immutable release, never a unit's newly active publication.
        when (val manifest = manifest(identity.publicationId)) {
            is PlaybackAccessResult.Refused -> return manifest
            is PlaybackAccessResult.Accepted -> if (manifest.value != identity) {
                return PlaybackAccessResult.Refused(PlaybackAccessFailure.IdentityChanged)
            }
        }
        val response = request("POST", identity.publicationId, "/playback-authorizations")
        refusal(response)?.let { return it }
        val grant = try { catalogJson.decodeFromString<PlaybackGrantWire>(response.body ?: "") }
            catch (_: SerializationException) { return PlaybackAccessResult.Refused(PlaybackAccessFailure.Protocol) }
        val now = nowMs()
        if (grant.publication != identity.publicationId || grant.rendition != identity.renditionId ||
            grant.hash != identity.sha256 || grant.contentType != "audio/ogg" || !grant.ranges ||
            grant.expires <= now || grant.expires - now > PlaybackGrant.TTL_MS + PlaybackGrant.CLOCK_SKEW_ALLOWANCE_MS ||
            !safeMediaUrl(grant.url)) {
            return PlaybackAccessResult.Refused(PlaybackAccessFailure.Protocol)
        }
        return PlaybackAccessResult.Accepted(PlaybackGrant(PlaybackLocation(grant.url), grant.expires))
    }
}

private fun canonicalId(value: String): Boolean = value.matches(Regex("[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}"))

/** HTTPS transport only; rejects credentials, fragments, unsafe bytes and malformed authority. */
internal fun safeMediaUrl(value: String): Boolean = value.length in 9..8192 &&
    value.matches(Regex("https://[A-Za-z0-9.-]+(?::[0-9]{1,5})?/[^\\s#\\\\]*")) &&
    value.none { it.isISOControl() } && !value.substringAfter("https://").substringBefore('/').contains('@')
