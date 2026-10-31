package com.cantos.theatre.adapters.http

import com.cantos.theatre.domain.catalog.PublicCatalogQuery

import com.cantos.theatre.domain.catalog.CatalogCheck
import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.CatalogViolation
import com.cantos.theatre.domain.catalog.CoverContentType
import com.cantos.theatre.domain.catalog.MaxSafeMillis
import com.cantos.theatre.domain.catalog.PublicCover
import com.cantos.theatre.domain.catalog.PublicUnit
import com.cantos.theatre.domain.catalog.PublicUnitId
import com.cantos.theatre.domain.catalog.PublicUnitKind
import com.cantos.theatre.domain.catalog.PublicUnitPage
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.catalog.PublicWorkKind
import com.cantos.theatre.domain.catalog.PublicWorkPage
import com.cantos.theatre.domain.catalog.PublicationId
import com.cantos.theatre.ports.CatalogCachePort
import com.cantos.theatre.ports.CatalogProtocolFault
import com.cantos.theatre.ports.PublicCatalogPort
import com.cantos.theatre.ports.PublicCatalogRead
import kotlinx.coroutines.CancellationException
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonNull

/*
 * Hand-written Theatre Discovery v1 wire DTOs owned by this client. They mirror the Server-owned
 * `services/cantos-server/contracts/theatre/discovery/v1/discovery.schema.json` field for field
 * and are **drift-checked** against that schema and its fixtures in `PublicCatalogWireDriftTest`;
 * they are not generated.
 * Every object is strict like the Rust `deny_unknown_fields`: unknown keys, missing keys
 * (including a missing nullable `next_cursor`), wrong JSON types and unknown enum values fail.
 */

/**
 * kotlinx accepts a quoted number (`"5000"`) for a Long even in strict mode; serde and the schema
 * do not. This keeps integer fields JSON numbers only, without fractions or exponents.
 */
internal object StrictJsonLong : KSerializer<Long> {
    override val descriptor: SerialDescriptor = PrimitiveSerialDescriptor("com.cantos.theatre.StrictJsonLong", PrimitiveKind.LONG)
    override fun serialize(encoder: Encoder, value: Long) = encoder.encodeLong(value)
    override fun deserialize(decoder: Decoder): Long {
        val json = decoder as? JsonDecoder ?: return decoder.decodeLong()
        val primitive = json.decodeJsonElement() as? JsonPrimitive
        if (primitive == null || primitive.isString) throw SerializationException("Expected a JSON integer")
        return primitive.content.toLongOrNull() ?: throw SerializationException("Expected a JSON integer")
    }
}

/** Same strictness for booleans: `"false"` is a string, not a JSON boolean. */
internal object StrictJsonBoolean : KSerializer<Boolean> {
    override val descriptor: SerialDescriptor = PrimitiveSerialDescriptor("com.cantos.theatre.StrictJsonBoolean", PrimitiveKind.BOOLEAN)
    override fun serialize(encoder: Encoder, value: Boolean) = encoder.encodeBoolean(value)
    override fun deserialize(decoder: Decoder): Boolean {
        val json = decoder as? JsonDecoder ?: return decoder.decodeBoolean()
        val primitive = json.decodeJsonElement() as? JsonPrimitive
        if (primitive == null || primitive.isString) throw SerializationException("Expected a JSON boolean")
        return when (primitive.content) {
            "true" -> true
            "false" -> false
            else -> throw SerializationException("Expected a JSON boolean")
        }
    }
}

@Serializable
internal enum class WorkKindWire {
    @SerialName("novel") Novel,
    @SerialName("story") Story,
    @SerialName("podcast") Podcast,
}

@Serializable
internal enum class UnitKindWire {
    @SerialName("chapter") Chapter,
    @SerialName("episode") Episode,
}

@Serializable
internal enum class CoverContentTypeWire {
    @SerialName("image/png") Png,
    @SerialName("image/jpeg") Jpeg,
    @SerialName("image/avif") Avif,
}

/** `CoverImage` of Discovery v1.2; every field is required and nothing else is admitted. */
@Serializable
internal data class CoverImageWire(
    @SerialName("url") val url: String,
    @SerialName("sha256") val sha256: String,
    @SerialName("content_type") val contentType: CoverContentTypeWire,
    @SerialName("width_px") @Serializable(with = StrictJsonLong::class) val widthPx: Long,
    @SerialName("height_px") @Serializable(with = StrictJsonLong::class) val heightPx: Long,
    @SerialName("byte_length") @Serializable(with = StrictJsonLong::class) val byteLength: Long,
    @SerialName("alt_text") val altText: String,
)

/** The canonical generation representation keeps metadata separate from associated playable audio. */
@Serializable
internal data class MetadataWorkWire(
    @SerialName("work_id") val workId: String,
    @SerialName("kind") val kind: WorkKindWire,
    @SerialName("title") val title: String,
    @SerialName("description") val description: String,
    @SerialName("language") val language: String,
    @SerialName("first_visible_at_ms") @Serializable(with = StrictJsonLong::class) val firstVisibleAtMs: Long,
    @SerialName("unit_count") @Serializable(with = StrictJsonLong::class) val unitCount: Long,
    @SerialName("cover") val cover: CoverImageWire?,
)
@Serializable
internal data class MetadataWorkPageWire(
    @SerialName("items") val items: List<MetadataWorkWire>,
    @SerialName("next_cursor") val nextCursor: String?,
)
@Serializable
internal data class UnitPlaybackWire(
    @SerialName("publication_id") val publicationId: String,
    @SerialName("published_at_ms") @Serializable(with = StrictJsonLong::class) val publishedAtMs: Long,
    @SerialName("duration_ms") @Serializable(with = StrictJsonLong::class) val durationMs: Long,
)
@Serializable
internal data class MetadataUnitWire(
    @SerialName("unit_id") val unitId: String,
    @SerialName("kind") val kind: UnitKindWire,
    @SerialName("position") @Serializable(with = StrictJsonLong::class) val position: Long,
    @SerialName("title") val title: String,
    @SerialName("description") val description: String,
    @SerialName("ordinal") @Serializable(with = StrictJsonLong::class) val ordinal: Long,
    @SerialName("playback") val playback: UnitPlaybackWire?,
)
@Serializable
internal data class MetadataUnitPageWire(
    @SerialName("work_id") val workId: String,
    @SerialName("items") val items: List<MetadataUnitWire>,
    @SerialName("next_cursor") val nextCursor: String?,
)

@Serializable
internal enum class ListenerErrorCodeWire {
    @SerialName("invalid_request") InvalidRequest,
    @SerialName("invalid_cursor") InvalidCursor,
    @SerialName("not_found") NotFound,
    @SerialName("rendition_unavailable") RenditionUnavailable,
    @SerialName("unavailable") Unavailable,
}

@Serializable
internal data class ListenerErrorWire(
    @SerialName("code") val code: ListenerErrorCodeWire,
    @SerialName("retryable") @Serializable(with = StrictJsonBoolean::class) val retryable: Boolean,
)

/** Explicitly strict, so a future default change in kotlinx cannot loosen the boundary. */
internal val catalogJson: Json = Json {
    ignoreUnknownKeys = false
    isLenient = false
    coerceInputValues = false
    explicitNulls = true
    allowSpecialFloatingPointValues = false
    allowTrailingComma = false
    allowComments = false
    decodeEnumsCaseInsensitive = false
}

// --- wire -> domain mapping -------------------------------------------------------------------

private inline fun <T, R> CatalogCheck<T>.then(next: (T) -> CatalogCheck<R>): CatalogCheck<R> = when (this) {
    is CatalogCheck.Valid -> next(value)
    is CatalogCheck.Invalid -> this
}

/** All-or-nothing: the first refused element refuses the whole page. */
private inline fun <T, R> List<T>.checkEach(map: (T) -> CatalogCheck<R>): CatalogCheck<List<R>> {
    val accepted = ArrayList<R>(size)
    for (element in this) {
        when (val checked = map(element)) {
            is CatalogCheck.Valid -> accepted += checked.value
            is CatalogCheck.Invalid -> return checked
        }
    }
    return CatalogCheck.Valid(accepted.toList())
}

private fun WorkKindWire.toDomain(): PublicWorkKind = when (this) {
    WorkKindWire.Novel -> PublicWorkKind.Novel
    WorkKindWire.Story -> PublicWorkKind.Story
    WorkKindWire.Podcast -> PublicWorkKind.Podcast
}

private fun UnitKindWire.toDomain(): PublicUnitKind = when (this) {
    UnitKindWire.Chapter -> PublicUnitKind.Chapter
    UnitKindWire.Episode -> PublicUnitKind.Episode
}

private fun String?.toCursor(): CatalogCheck<CatalogCursor?> =
    if (this == null) CatalogCheck.Valid(null) else CatalogCursor.parse(this)

private fun CoverContentTypeWire.toDomain(): CoverContentType = when (this) {
    CoverContentTypeWire.Png -> CoverContentType.Png
    CoverContentTypeWire.Jpeg -> CoverContentType.Jpeg
    CoverContentTypeWire.Avif -> CoverContentType.Avif
}

private fun CoverImageWire?.toCover(work: PublicWorkId): CatalogCheck<PublicCover?> =
    if (this == null) CatalogCheck.Valid(null)
    else PublicCover.of(work, url, sha256, contentType.toDomain(), widthPx, heightPx, byteLength, altText).then {
        CatalogCheck.Valid(it)
    }

internal fun MetadataWorkWire.toDomain(): CatalogCheck<PublicWork> = PublicWorkId.parse(workId).then { id ->
    if (unitCount !in 0..MaxSafeMillis) CatalogCheck.Invalid(CatalogViolation.InvalidUnitCount)
    else cover.toCover(id).then { checked -> PublicWork.of(id, kind.toDomain(), title, description, language, firstVisibleAtMs, checked) }
}
internal fun MetadataWorkPageWire.toDomain(): CatalogCheck<PublicWorkPage> = items.checkEach { it.toDomain() }.then { works ->
    nextCursor.toCursor().then { next -> PublicWorkPage.of(works, next) }
}
internal fun MetadataUnitWire.toDomain(): CatalogCheck<PublicUnit> = PublicUnitId.parse(unitId).then { id ->
    if (ordinal !in 1..MaxSafeMillis) CatalogCheck.Invalid(CatalogViolation.InvalidUnitCount)
    else if (playback == null) PublicUnit.withoutAudio(id, kind.toDomain(), position, title, description)
    else PublicationId.parse(playback.publicationId).then { publication ->
        PublicUnit.of(id, kind.toDomain(), position, title, description, publication, playback.publishedAtMs, playback.durationMs)
    }
}
internal fun MetadataUnitPageWire.toDomain(requested: PublicWork): CatalogCheck<PublicUnitPage> =
    PublicWorkId.parse(workId).then { pageWork ->
        items.checkEach { it.toDomain() }.then { units -> nextCursor.toCursor().then { next -> PublicUnitPage.of(requested, pageWork, units, next) } }
    }

internal fun <T> CatalogCheck<T>.acceptedOrNull(): T? = (this as? CatalogCheck.Valid)?.value

// --- transport ---------------------------------------------------------------------------------

/**
 * Body bound for one catalog response. Pages are requested with [CatalogPageLimit] = 20 items;
 * a WorkSummary may hold 512 + 16384 schema characters. 20 such items at the worst JSON
 * expansion (6 bytes per `\u` escaped character) is ~2.03 MB, and 50 items of 3-byte Vietnamese
 * UTF-8 is ~2.5 MiB. 4 MiB covers both with headroom, while still refusing unbounded bodies.
 */
const val CatalogBodyLimitBytes: Int = 4 * 1024 * 1024


/** Explicit page size (the server default); a cursor is bound to the limit it was issued for. */
const val CatalogPageLimit: Int = 20

/** Closed anonymous GET plan, built only from a validated origin, IDs and cursors. */
class CatalogHttpRequest internal constructor(val url: String, val maxBodyBytes: Int, val accept: String = "application/json")

class CatalogHttpResponse(val status: Int, val contentType: String?, val body: ByteArray)

/**
 * GET-only, no cookies, credentials or redirects. Implementations throw
 * [CatalogResponseTooLargeException] past the bound, [CookieRuntimeException] when an ambient
 * cookie runtime exists, and any other exception for a transport failure.
 */
fun interface CatalogHttpTransport {
    suspend fun get(request: CatalogHttpRequest): CatalogHttpResponse
}

class CatalogTimeoutException : IllegalStateException("Catalog request deadline exceeded")

class CatalogResponseTooLargeException : IllegalStateException("Catalog response exceeds its bound")

/** Production HTTP adapter for the anonymous Theatre catalog (no Studio route, no session). */
class PublicCatalogHttpAdapter(
    origin: ApiOrigin,
    private val transport: CatalogHttpTransport,
    private val cache: CatalogCachePort? = null,
) : PublicCatalogPort {
    private val base = "${origin.probeUrl}/works"

    override suspend fun works(cursor: CatalogCursor?, query: PublicCatalogQuery): PublicCatalogRead<PublicWorkPage> {
        // /search and /works share the v1 query/response contract. The server searches the full
        // eligible catalog, never only the pages already present on this device.
        val path = if (query.text.isEmpty()) base else "${base.removeSuffix("/works")}/search"
        // One generation representation; the Server binds Unit continuations to their generation.
        val url = pageUrl(path, cursor) +
            (if (query.text.isNotEmpty()) "&q=${query.text.queryEncoded()}" else "") +
            (query.kind?.let { "&kind=${it.name.lowercase()}" } ?: "")
        return read(url, cursor != null, MetadataWorkPageWire.serializer()) { it.toDomain().acceptedOrNull() }
    }

    override suspend fun work(id: PublicWorkId): PublicCatalogRead<PublicWork> =
        read("$base/${id.value}", false, MetadataWorkWire.serializer()) { wire ->
            wire.toDomain().acceptedOrNull()?.takeIf { it.id == id }
        }

    override suspend fun units(work: PublicWork, cursor: CatalogCursor?): PublicCatalogRead<PublicUnitPage> =
        read(pageUrl("$base/${work.id.value}/units", cursor), cursor != null, MetadataUnitPageWire.serializer()) {
            it.toDomain(work).acceptedOrNull()
        }

    private fun pageUrl(path: String, cursor: CatalogCursor?): String =
        "$path?limit=$CatalogPageLimit" + (cursor?.let { "&cursor=${it.value}" } ?: "")

    private suspend fun <W, T> read(
        url: String,
        sentCursor: Boolean,
        serializer: KSerializer<W>,
        map: (W) -> T?,
    ): PublicCatalogRead<T> {
        // Metadata that was public can now be HIDE. A persisted response cannot establish
        // the current Work/Unit policy; every projection goes through the Server again.
        cache?.network()
        val response = try {
            transport.get(CatalogHttpRequest(url, CatalogBodyLimitBytes))
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: CatalogTimeoutException) {
            return PublicCatalogRead.AccessFailure(com.cantos.theatre.ports.CatalogAccessFault.Timeout)
        } catch (_: CatalogResponseTooLargeException) {
            return PublicCatalogRead.Protocol(CatalogProtocolFault.Oversized)
        } catch (_: Exception) {
            return PublicCatalogRead.Transport
        }
        val classified = classifyCatalogResponse(response, sentCursor, serializer, map)
        return classified
    }
}

/** Pure response classification, shared by the adapter and its boundary tests. */
internal fun <W, T> classifyCatalogResponse(
    response: CatalogHttpResponse,
    sentCursor: Boolean,
    serializer: KSerializer<W>,
    map: (W) -> T?,
): PublicCatalogRead<T> {
    when (response.status) {
        401 -> return PublicCatalogRead.AccessFailure(com.cantos.theatre.ports.CatalogAccessFault.Unauthenticated)
        403 -> return PublicCatalogRead.AccessFailure(com.cantos.theatre.ports.CatalogAccessFault.Forbidden)
        408, 504 -> return PublicCatalogRead.AccessFailure(com.cantos.theatre.ports.CatalogAccessFault.Timeout)
    }
    if (response.body.size > CatalogBodyLimitBytes) return PublicCatalogRead.Protocol(CatalogProtocolFault.Oversized)
    val json = response.contentType?.substringBefore(';')?.trim()?.lowercase() == "application/json"
    val text = try {
        response.body.decodeToString(throwOnInvalidSequence = true)
    } catch (_: CharacterCodingException) {
        null
    }
    if (response.status == 200) {
        if (!json) return PublicCatalogRead.Protocol(CatalogProtocolFault.ContentType)
        val wire = text?.let { decodeOrNull(serializer, it) } ?: return PublicCatalogRead.Protocol(CatalogProtocolFault.Malformed)
        return map(wire)?.let { PublicCatalogRead.Ready(it) } ?: PublicCatalogRead.Protocol(CatalogProtocolFault.Invalid)
    }
    // Only these statuses carry the Theatre envelope; a gateway may answer 502/503/504 without it.
    if (response.status !in setOf(400, 404, 500, 503)) {
        return if (response.status in 502..504) PublicCatalogRead.Unavailable(retryable = true)
        else PublicCatalogRead.Protocol(CatalogProtocolFault.UnexpectedStatus)
    }
    val error = if (json && text != null) decodeOrNull(ListenerErrorWire.serializer(), text) else null
    if (error == null) {
        return if (response.status == 503) PublicCatalogRead.Unavailable(retryable = true)
        else PublicCatalogRead.Protocol(if (json) CatalogProtocolFault.Malformed else CatalogProtocolFault.UnexpectedStatus)
    }
    // Exact v1 status/code/retryable table; any other pairing is a protocol fault.
    return when {
        response.status == 404 && error.code == ListenerErrorCodeWire.NotFound && !error.retryable ->
            PublicCatalogRead.NotFound
        response.status == 400 && error.code == ListenerErrorCodeWire.InvalidCursor && !error.retryable && sentCursor ->
            PublicCatalogRead.InvalidCursor
        response.status == 503 && error.retryable &&
            (error.code == ListenerErrorCodeWire.Unavailable || error.code == ListenerErrorCodeWire.RenditionUnavailable) ->
            PublicCatalogRead.Unavailable(retryable = true)
        response.status == 500 && error.code == ListenerErrorCodeWire.Unavailable && !error.retryable ->
            PublicCatalogRead.Unavailable(retryable = false)
        // invalid_request means this client built a request the contract rejects.
        else -> PublicCatalogRead.Protocol(CatalogProtocolFault.UnexpectedStatus)
    }
}

private fun <W> decodeOrNull(serializer: KSerializer<W>, text: String): W? = try {
    catalogJson.decodeFromString(serializer, text)
} catch (_: SerializationException) {
    null
} catch (_: IllegalArgumentException) {
    null
}

/** RFC 3986 UTF-8 query component; never lets user text create parameters or fragments. */
private fun String.queryEncoded(): String = buildString {
    for (byte in this@queryEncoded.encodeToByteArray()) {
        val value = byte.toInt() and 255
        if (value in 65..90 || value in 97..122 || value in 48..57 || value in listOf(45, 46, 95, 126)) append(value.toChar())
        else {
            append('%')
            append("0123456789ABCDEF"[value ushr 4])
            append("0123456789ABCDEF"[value and 15])
        }
    }
}
