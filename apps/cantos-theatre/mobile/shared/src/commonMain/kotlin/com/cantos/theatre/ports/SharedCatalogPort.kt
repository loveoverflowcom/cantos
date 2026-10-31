package com.cantos.theatre.ports

import com.cantos.theatre.domain.catalog.CatalogCursor
import com.cantos.theatre.domain.catalog.PublicUnitPage
import com.cantos.theatre.domain.catalog.PublicWork
import com.cantos.theatre.domain.catalog.PublicWorkId
import com.cantos.theatre.domain.catalog.PublicWorkPage

/** Local login incarnation, without a credential. A server cursor is never authority. */
@ConsistentCopyVisibility
data class SharedSessionKey internal constructor(val actor: String, val session: String, val epoch: Long) {
    override fun toString(): String = "SharedSessionKey([redacted])"
}

enum class SharedCatalogFailure { SignInRequired, Denied, Restart, Unavailable, Protocol }

sealed interface SharedCatalogRead<out T> {
    class Ready<T>(val value: T) : SharedCatalogRead<T>
    data class Refused(val reason: SharedCatalogFailure) : SharedCatalogRead<Nothing>
}

/**
 * Authenticated shared v1, never an anonymous PublicCatalogPort or a durable cache input.
 * The metadata value validators are reused; access always belongs to the serving session.
 */
interface SharedCatalogPort {
    suspend fun works(session: SharedSessionKey, cursor: CatalogCursor?): SharedCatalogRead<PublicWorkPage>
    suspend fun work(session: SharedSessionKey, id: PublicWorkId): SharedCatalogRead<PublicWork>
    suspend fun units(session: SharedSessionKey, work: PublicWork, cursor: CatalogCursor?): SharedCatalogRead<PublicUnitPage>
}
