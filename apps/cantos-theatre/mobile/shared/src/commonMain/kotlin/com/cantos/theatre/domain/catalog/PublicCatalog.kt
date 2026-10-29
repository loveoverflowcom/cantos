package com.cantos.theatre.domain.catalog

import kotlin.jvm.JvmInline

/*
 * The public Theatre catalog as released by the API (Theatre Discovery v1, ADR 0035).
 *
 * These values are separate from the sandbox-oriented `domain/publication` model, whose
 * `EpisodeKey(work, index)` is a mock playback index. Here every identity is the server's
 * canonical lower-case UUID; a list index or `position` is never an identity.
 * Nothing here is a playback, rights or entitlement claim: a listed unit is "listenable now"
 * according to the server's anonymous eligibility, and playback remains a separate gate.
 */

/** Why a server value was refused at the public-catalog boundary. */
enum class CatalogViolation {
    InvalidSearch,
    MalformedWorkId,
    MalformedUnitId,
    MalformedPublicationId,
    MalformedCursor,
    BlankTitle,
    PositionOutOfRange,
    NonPositiveDuration,
    TimestampOutOfRange,
    UnitKindMismatch,
    WorkMismatch,
    DuplicateWork,
    InvalidUnitCount,
    InvalidCover,
    CoverUrlMismatch,
    CoverWorkMismatch,
    DuplicateUnit,
    DuplicatePublication,
    PageTooLarge,
}

/** Typed refusal for a boundary factory; never a nullable or exception-driven protocol. */
sealed interface CatalogCheck<out T> {
    data class Valid<T>(val value: T) : CatalogCheck<T>
    data class Invalid(val violation: CatalogViolation) : CatalogCheck<Nothing>
}

private val canonicalUuid = Regex("^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
private val cursorShape = Regex("^lc1_[0-9a-f]{64}$")

/** JSON safe-integer ceiling used by every v1 `*_ms` field. */
const val MaxSafeMillis: Long = 9_007_199_254_740_991L

/** The schema's `maxItems` for one page. */
const val MaxPageItems: Int = 50

@JvmInline
value class PublicWorkId private constructor(val value: String) {
    companion object {
        fun parse(raw: String): CatalogCheck<PublicWorkId> =
            if (canonicalUuid.matches(raw)) CatalogCheck.Valid(PublicWorkId(raw))
            else CatalogCheck.Invalid(CatalogViolation.MalformedWorkId)
    }
}

@JvmInline
value class PublicUnitId private constructor(val value: String) {
    companion object {
        fun parse(raw: String): CatalogCheck<PublicUnitId> =
            if (canonicalUuid.matches(raw)) CatalogCheck.Valid(PublicUnitId(raw))
            else CatalogCheck.Invalid(CatalogViolation.MalformedUnitId)
    }
}

/** The exact immutable AudioRelease ID; a change for the same unit is a new release. */
@JvmInline
value class PublicationId private constructor(val value: String) {
    companion object {
        fun parse(raw: String): CatalogCheck<PublicationId> =
            if (canonicalUuid.matches(raw)) CatalogCheck.Valid(PublicationId(raw))
            else CatalogCheck.Invalid(CatalogViolation.MalformedPublicationId)
    }
}

/** Opaque server walk reference: not identity, not authorization, never parsed or persisted. */
@JvmInline
value class CatalogCursor private constructor(val value: String) {
    companion object {
        fun parse(raw: String): CatalogCheck<CatalogCursor> =
            if (cursorShape.matches(raw)) CatalogCheck.Valid(CatalogCursor(raw))
            else CatalogCheck.Invalid(CatalogViolation.MalformedCursor)
    }
}

enum class PublicWorkKind { Novel, Story, Podcast }

enum class PublicUnitKind { Chapter, Episode }

/** Novels and stories are told in chapters; podcasts in episodes. */
val PublicWorkKind.unitKind: PublicUnitKind
    get() = when (this) {
        PublicWorkKind.Novel, PublicWorkKind.Story -> PublicUnitKind.Chapter
        PublicWorkKind.Podcast -> PublicUnitKind.Episode
    }

/** The three image types the Server admits for a public cover. */
enum class CoverContentType(val mime: String) { Png("image/png"), Jpeg("image/jpeg"), Avif("image/avif") }

/** Bounds the Server enforces on every cover (private Artwork v1 admission); no resize exists. */
const val MaxCoverDimensionPx: Long = 4096
const val MaxCoverBytes: Long = 8_388_608
const val MaxCoverAltTextBytes: Int = 1024

private val sha256Hex = Regex("^[0-9a-f]{64}$")

/**
 * The public cover the Server authorized for exactly one [work] (Discovery v1.2, ADR 0033).
 *
 * [url] is used exactly as given: it is accepted only when it is the HTTPS cover route of this
 * Work for this [sha256] (no userinfo, query or fragment), and no other URL is ever derived. The
 * value carries no asset ID, rights, provenance or storage facts because the contract has none.
 * A revoked, expired or republished image is a 404 at [url], never a state of this value.
 */
class PublicCover private constructor(
    val work: PublicWorkId,
    val url: String,
    val sha256: String,
    val contentType: CoverContentType,
    val widthPx: Int,
    val heightPx: Int,
    val byteLength: Int,
    val altText: String,
) {
    override fun equals(other: Any?): Boolean = other is PublicCover && work == other.work && url == other.url &&
        sha256 == other.sha256 && contentType == other.contentType && widthPx == other.widthPx &&
        heightPx == other.heightPx && byteLength == other.byteLength && altText == other.altText

    override fun hashCode(): Int = listOf(work, url, sha256, contentType, widthPx, heightPx, byteLength, altText).hashCode()
    override fun toString(): String = "PublicCover(work=${work.value}, sha256=$sha256, $contentType ${widthPx}x$heightPx)"

    companion object {
        fun of(
            work: PublicWorkId,
            url: String,
            sha256: String,
            contentType: CoverContentType,
            widthPx: Long,
            heightPx: Long,
            byteLength: Long,
            altText: String,
        ): CatalogCheck<PublicCover> = when {
            !sha256Hex.matches(sha256) || widthPx !in 1..MaxCoverDimensionPx || heightPx !in 1..MaxCoverDimensionPx ||
                byteLength !in 1..MaxCoverBytes || altText.isBlank() ||
                altText.encodeToByteArray().size > MaxCoverAltTextBytes -> CatalogCheck.Invalid(CatalogViolation.InvalidCover)
            !isCoverRoute(url, work, sha256) -> CatalogCheck.Invalid(CatalogViolation.CoverUrlMismatch)
            else -> CatalogCheck.Valid(
                PublicCover(work, url, sha256, contentType, widthPx.toInt(), heightPx.toInt(), byteLength.toInt(), altText),
            )
        }

        /** `https://{authority}/api/v1/theatre/works/{work}/cover/{sha256}` and nothing else. */
        private fun isCoverRoute(url: String, work: PublicWorkId, sha256: String): Boolean {
            if (url.length > 4096 || !url.startsWith("https://")) return false
            val rest = url.removePrefix("https://")
            val slash = rest.indexOf('/')
            if (slash <= 0) return false
            val authority = rest.substring(0, slash)
            if (authority.any { it.isWhitespace() || it.isISOControl() || it == '?' || it == '#' || it == '@' || it == '\\' }) return false
            return rest.substring(slash) == "/api/v1/theatre/works/${work.value}/cover/$sha256"
        }
    }
}

class PublicWork private constructor(
    val id: PublicWorkId,
    val kind: PublicWorkKind,
    val title: String,
    val description: String,
    val language: String,
    val firstVisibleAtMs: Long,
    /** The Server-authorized cover, or null: the one legitimate no-cover state (never a reason). */
    val cover: PublicCover?,
) {
    override fun equals(other: Any?): Boolean = other is PublicWork && id == other.id && kind == other.kind &&
        title == other.title && description == other.description && language == other.language &&
        firstVisibleAtMs == other.firstVisibleAtMs && cover == other.cover

    override fun hashCode(): Int = listOf(id, kind, title, description, language, firstVisibleAtMs, cover).hashCode()
    override fun toString(): String = "PublicWork(id=${id.value}, kind=$kind)"

    companion object {
        fun of(
            id: PublicWorkId,
            kind: PublicWorkKind,
            title: String,
            description: String,
            language: String,
            firstVisibleAtMs: Long,
            cover: PublicCover? = null,
        ): CatalogCheck<PublicWork> = when {
            title.isBlank() -> CatalogCheck.Invalid(CatalogViolation.BlankTitle)
            firstVisibleAtMs !in 0..MaxSafeMillis -> CatalogCheck.Invalid(CatalogViolation.TimestampOutOfRange)
            cover != null && cover.work != id -> CatalogCheck.Invalid(CatalogViolation.CoverWorkMismatch)
            else -> CatalogCheck.Valid(PublicWork(id, kind, title, description, language, firstVisibleAtMs, cover))
        }
    }
}

/** `position` is display order only (gaps and duplicates are possible), never an episode number. */
class PublicUnit private constructor(
    val id: PublicUnitId,
    val kind: PublicUnitKind,
    val position: Int,
    val title: String,
    val description: String,
    val publicationId: PublicationId?,
    val publishedAtMs: Long?,
    val durationMs: Long?,
) {
    override fun equals(other: Any?): Boolean = other is PublicUnit && id == other.id && kind == other.kind &&
        position == other.position && title == other.title && description == other.description &&
        publicationId == other.publicationId && publishedAtMs == other.publishedAtMs && durationMs == other.durationMs

    override fun hashCode(): Int =
        listOf(id, kind, position, title, description, publicationId, publishedAtMs, durationMs).hashCode()
    override fun toString(): String = "PublicUnit(id=${id.value}, publication=${publicationId?.value})"

    companion object {
        fun of(
            id: PublicUnitId,
            kind: PublicUnitKind,
            position: Long,
            title: String,
            description: String,
            publicationId: PublicationId,
            publishedAtMs: Long,
            durationMs: Long,
        ): CatalogCheck<PublicUnit> = when {
            position !in 0..Int.MAX_VALUE.toLong() -> CatalogCheck.Invalid(CatalogViolation.PositionOutOfRange)
            title.isBlank() -> CatalogCheck.Invalid(CatalogViolation.BlankTitle)
            // The server emits 1..=3_600_000; 0 is never reinterpreted as "unknown" (ADR 0035 §3).
            durationMs !in 1..MaxSafeMillis -> CatalogCheck.Invalid(CatalogViolation.NonPositiveDuration)
            publishedAtMs !in 0..MaxSafeMillis -> CatalogCheck.Invalid(CatalogViolation.TimestampOutOfRange)
            else -> CatalogCheck.Valid(
                PublicUnit(id, kind, position.toInt(), title, description, publicationId, publishedAtMs, durationMs),
            )
        }

        /** An explicitly disclosed row without an eligible release has no playback identity. */
        fun withoutAudio(id: PublicUnitId, kind: PublicUnitKind, position: Long, title: String, description: String): CatalogCheck<PublicUnit> = when {
            position !in 0..Int.MAX_VALUE.toLong() -> CatalogCheck.Invalid(CatalogViolation.PositionOutOfRange)
            title.isBlank() -> CatalogCheck.Invalid(CatalogViolation.BlankTitle)
            else -> CatalogCheck.Valid(PublicUnit(id, kind, position.toInt(), title, description, null, null, null))
        }
    }
}

/** One validated page of the work walk. `next == null` is the end; an empty page is not. */
class PublicWorkPage private constructor(items: List<PublicWork>, val next: CatalogCursor?) {
    private val snapshot = items.toList()
    val items: List<PublicWork> get() = snapshot.toList()

    override fun equals(other: Any?): Boolean = other is PublicWorkPage && snapshot == other.snapshot && next == other.next
    override fun hashCode(): Int = 31 * snapshot.hashCode() + next.hashCode()
    override fun toString(): String = "PublicWorkPage(items=${snapshot.size}, more=${next != null})"

    companion object {
        fun of(items: List<PublicWork>, next: CatalogCursor?): CatalogCheck<PublicWorkPage> {
            val copy = items.toList()
            return when {
                copy.size > MaxPageItems -> CatalogCheck.Invalid(CatalogViolation.PageTooLarge)
                copy.map { it.id }.toSet().size != copy.size -> CatalogCheck.Invalid(CatalogViolation.DuplicateWork)
                else -> CatalogCheck.Valid(PublicWorkPage(copy, next))
            }
        }
    }
}

/** One validated page of a work's unit walk, bound to the work whose kind it was checked against. */
class PublicUnitPage private constructor(val work: PublicWorkId, items: List<PublicUnit>, val next: CatalogCursor?) {
    private val snapshot = items.toList()
    val items: List<PublicUnit> get() = snapshot.toList()

    override fun equals(other: Any?): Boolean = other is PublicUnitPage && work == other.work &&
        snapshot == other.snapshot && next == other.next
    override fun hashCode(): Int = listOf(work, snapshot, next).hashCode()
    override fun toString(): String = "PublicUnitPage(work=${work.value}, items=${snapshot.size}, more=${next != null})"

    companion object {
        /** [requested] is the work whose detail was read; the page must belong to it. */
        fun of(
            requested: PublicWork,
            pageWork: PublicWorkId,
            items: List<PublicUnit>,
            next: CatalogCursor?,
        ): CatalogCheck<PublicUnitPage> {
            val copy = items.toList()
            return when {
                pageWork != requested.id -> CatalogCheck.Invalid(CatalogViolation.WorkMismatch)
                copy.size > MaxPageItems -> CatalogCheck.Invalid(CatalogViolation.PageTooLarge)
                copy.any { it.kind != requested.kind.unitKind } -> CatalogCheck.Invalid(CatalogViolation.UnitKindMismatch)
                copy.map { it.id }.toSet().size != copy.size -> CatalogCheck.Invalid(CatalogViolation.DuplicateUnit)
                copy.mapNotNull { it.publicationId }.let { it.toSet().size != it.size } ->
                    CatalogCheck.Invalid(CatalogViolation.DuplicatePublication)
                else -> CatalogCheck.Valid(PublicUnitPage(requested.id, copy, next))
            }
        }
    }
}

/** Immutable server walk identity; a different query must start without a previous cursor. */
@ConsistentCopyVisibility
data class PublicCatalogQuery private constructor(val text: String, val kind: PublicWorkKind?) {
    init {
        require(text.encodeToByteArray().size <= 512 && text.none { it.isISOControl() })
    }
    val isDiscovery: Boolean get() = text.isEmpty() && kind == null

    companion object {
        val All = PublicCatalogQuery("", null)
        fun of(text: String, kind: PublicWorkKind? = null): CatalogCheck<PublicCatalogQuery> =
            if (text.encodeToByteArray().size > 512 || text.any { it.isISOControl() }) {
                CatalogCheck.Invalid(CatalogViolation.InvalidSearch)
            } else CatalogCheck.Valid(PublicCatalogQuery(text.trim(), kind))
    }
}
