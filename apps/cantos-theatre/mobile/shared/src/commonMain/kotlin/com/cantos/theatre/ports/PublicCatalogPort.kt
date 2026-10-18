package com.cantos.theatre.ports

import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicUnitPage
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.catalog.PublicWorkPage

/** Why a response could not be accepted; for tests and diagnostics, never shown verbatim. */
enum class CatalogProtocolFault {
    /** A status, redirect or error code this anonymous read route never answers with. */
    UnexpectedStatus,
    ContentType,
    Oversized,
    /** Not UTF-8, not JSON, an unknown or missing field, or a wrong JSON type. */
    Malformed,
    /** Well-formed JSON whose values fail the public-catalog domain checks. */
    Invalid,
}

/**
 * Closed result of one anonymous public-catalog read. `NotFound` deliberately merges absent,
 * private, withdrawn and draft (the client never names a reason it was not given).
 */
sealed interface PublicCatalogRead<out T> {
    data class Ready<T>(val value: T) : PublicCatalogRead<T>
    data object NotFound : PublicCatalogRead<Nothing>
    /** The cursor expired, was evicted or no longer binds this walk; restart from page one. */
    data object InvalidCursor : PublicCatalogRead<Nothing>
    data class Unavailable(val retryable: Boolean) : PublicCatalogRead<Nothing>
    data class Protocol(val fault: CatalogProtocolFault) : PublicCatalogRead<Nothing>
    /** No response: connection, TLS, timeout, missing network permission or refused cookie runtime. */
    data object Transport : PublicCatalogRead<Nothing>
}

/**
 * Anonymous read-only Theatre catalog. Reads never consume a cursor, so retrying a failed page
 * with the same cursor is safe. Cancellation propagates; every other failure is a typed value.
 */
interface PublicCatalogPort {
    suspend fun works(cursor: CatalogCursor?): PublicCatalogRead<PublicWorkPage>
    suspend fun work(id: PublicWorkId): PublicCatalogRead<PublicWork>
    /** Units of [work], validated against its id and kind. */
    suspend fun units(work: PublicWork, cursor: CatalogCursor?): PublicCatalogRead<PublicUnitPage>
}
