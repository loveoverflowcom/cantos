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
    MalformedWorkId,
    MalformedUnitId,
    MalformedPublicationId,
    MalformedCursor,
    InvalidSearchQuery,
    BlankTitle,
    PositionOutOfRange,
    NonPositiveDuration,
    TimestampOutOfRange,
    UnitKindMismatch,
    WorkMismatch,
    DuplicateWork,
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

/**
 * A submitted discovery query, bounded by the existing API's 512 UTF-8 byte/control rule.
 * Case folding and Unicode normalization belong to the server; the UI locale is unrelated.
 */
@JvmInline
value class CatalogSearchQuery private constructor(val value: String) {
    companion object {
        val Empty: CatalogSearchQuery = CatalogSearchQuery("")

        fun parse(raw: String): CatalogCheck<CatalogSearchQuery> =
            if (raw.encodeToByteArray().size > 512 || raw.any { it.code in 0..31 || it.code in 127..159 }) {
                CatalogCheck.Invalid(CatalogViolation.InvalidSearchQuery)
            } else CatalogCheck.Valid(CatalogSearchQuery(raw.trim()))
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

class PublicWork private constructor(
    val id: PublicWorkId,
    val kind: PublicWorkKind,
    val title: String,
    val description: String,
    val language: String,
    val firstPublishedAtMs: Long,
) {
    override fun equals(other: Any?): Boolean = other is PublicWork && id == other.id && kind == other.kind &&
        title == other.title && description == other.description && language == other.language &&
        firstPublishedAtMs == other.firstPublishedAtMs

    override fun hashCode(): Int = listOf(id, kind, title, description, language, firstPublishedAtMs).hashCode()
    override fun toString(): String = "PublicWork(id=${id.value}, kind=$kind)"

    companion object {
        fun of(
            id: PublicWorkId,
            kind: PublicWorkKind,
            title: String,
            description: String,
            language: String,
            firstPublishedAtMs: Long,
        ): CatalogCheck<PublicWork> = when {
            title.isBlank() -> CatalogCheck.Invalid(CatalogViolation.BlankTitle)
            firstPublishedAtMs !in 0..MaxSafeMillis -> CatalogCheck.Invalid(CatalogViolation.TimestampOutOfRange)
            else -> CatalogCheck.Valid(PublicWork(id, kind, title, description, language, firstPublishedAtMs))
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
    val publicationId: PublicationId,
    val publishedAtMs: Long,
    val durationMs: Long,
) {
    override fun equals(other: Any?): Boolean = other is PublicUnit && id == other.id && kind == other.kind &&
        position == other.position && title == other.title && description == other.description &&
        publicationId == other.publicationId && publishedAtMs == other.publishedAtMs && durationMs == other.durationMs

    override fun hashCode(): Int =
        listOf(id, kind, position, title, description, publicationId, publishedAtMs, durationMs).hashCode()
    override fun toString(): String = "PublicUnit(id=${id.value}, publication=${publicationId.value})"

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
                copy.map { it.publicationId }.toSet().size != copy.size ->
                    CatalogCheck.Invalid(CatalogViolation.DuplicatePublication)
                else -> CatalogCheck.Valid(PublicUnitPage(requested.id, copy, next))
            }
        }
    }
}
