package com.cantos.theatre.ports

import kotlinx.coroutines.flow.StateFlow

enum class CatalogCacheStatus { Idle, Cached, Network, StorageUnavailable }
/** Catalog metadata only. The HTTP adapter validates reads and strips opaque cursors before writes. */
interface CatalogCachePort {
    val status: StateFlow<CatalogCacheStatus>
    fun account(): String?
    fun read(key: String, owner: String?): String?
    fun put(key: String, validatedJson: String, owner: String?)
    fun network()
    /** Marks retained entries stale; failed refresh never erases the last persisted valid content. */
    fun invalidate()
}
