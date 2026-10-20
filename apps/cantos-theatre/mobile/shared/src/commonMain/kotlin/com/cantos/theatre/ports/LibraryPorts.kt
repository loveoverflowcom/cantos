package com.cantos.theatre.ports

data class LocalLibraryRecord(val resource: String, val key: String, val body: String, val revision: Long)
class LocalLibraryIntent(val operation: String, val key: String, val method: String, val path: String, val body: String, val sent: Boolean = false, val failure: String? = null) {
    override fun toString() = "LocalLibraryIntent([redacted])"
}
interface LibraryTransaction {
    fun records(): List<LocalLibraryRecord>
    fun record(resource: String, key: String): LocalLibraryRecord?
    fun put(record: LocalLibraryRecord)
    fun deleteResource(resource: String)
    fun delete(resource: String, key: String)
    fun intents(): List<LocalLibraryIntent>
    fun enqueue(intent: LocalLibraryIntent)
    fun freeze(operation: String, body: String)
    fun markSent(operation: String)
    fun fail(operation: String, reason: String)
    fun resolve(operation: String)
    /** Persisted install device ID and monotonically increasing per-account/resource sequence. */
    fun nextSequence(key: String, createDevice: () -> String): Pair<String, Long>
}
interface LibraryLocalPort {
    fun <T> transaction(account: String, action: LibraryTransaction.() -> T): T
    fun close()
}
