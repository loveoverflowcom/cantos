package com.cantos.theatre.adapters.library

import com.cantos.theatre.ports.CatalogCachePort
import com.cantos.theatre.ports.CatalogCacheStatus
import com.cantos.theatre.ports.LibraryLocalPort
import com.cantos.theatre.ports.LocalLibraryRecord
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** Uses the process Library database; no second driver or guest durable state. */
class AccountCatalogCache(private val store: LibraryLocalPort, private val accountScope: () -> String?) : CatalogCachePort {
    private val mutableStatus = MutableStateFlow(CatalogCacheStatus.Idle)
    override val status: StateFlow<CatalogCacheStatus> = mutableStatus.asStateFlow()
    override fun account(): String? = accountScope()
    override fun read(key: String, owner: String?): String? {
        if (owner == null || owner != accountScope()) return null
        return try {
            store.transaction(owner) {
                val generation = record("catalog_meta", "generation")?.revision ?: 0
                record("catalog", key)?.takeIf { it.revision == generation }?.body
            }?.also { mutableStatus.value = CatalogCacheStatus.Cached }
        } catch (_: Exception) { mutableStatus.value = CatalogCacheStatus.StorageUnavailable; null }
    }
    override fun put(key: String, validatedJson: String, owner: String?) {
        if (owner == null || owner != accountScope()) return
        try {
            store.transaction(owner) {
                val generation = record("catalog_meta", "generation")?.revision ?: 0
                put(LocalLibraryRecord("catalog", key, validatedJson, generation))
            }
        } catch (_: Exception) { mutableStatus.value = CatalogCacheStatus.StorageUnavailable }
    }
    override fun network() { mutableStatus.value = CatalogCacheStatus.Network }
    override fun invalidate() {
        val owner = accountScope() ?: return
        try {
            store.transaction(owner) {
                val current = record("catalog_meta", "generation")?.revision ?: 0
                check(current != Long.MAX_VALUE)
                put(LocalLibraryRecord("catalog_meta", "generation", "", current + 1))
            }
        } catch (_: Exception) { mutableStatus.value = CatalogCacheStatus.StorageUnavailable }
    }
}
