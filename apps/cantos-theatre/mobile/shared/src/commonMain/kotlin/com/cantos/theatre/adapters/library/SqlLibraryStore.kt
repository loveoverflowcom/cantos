package com.cantos.theatre.adapters.library

import app.cash.sqldelight.db.SqlDriver
import com.cantos.theatre.db.LibraryDatabase
import com.cantos.theatre.ports.LibraryLocalPort
import com.cantos.theatre.ports.LibraryTransaction
import com.cantos.theatre.ports.LocalLibraryIntent
import com.cantos.theatre.ports.LocalLibraryRecord

/** The same SQLite transaction commits the local candidate, device sequence and exact outbox body. */
class SqlLibraryStore(private val driver: SqlDriver) : LibraryLocalPort {
    private val db = LibraryDatabase(driver)
    override fun <T> transaction(account: String, action: LibraryTransaction.() -> T): T = db.transactionWithResult {
        action(object : LibraryTransaction {
            override fun records() = db.libraryQueries.allRecords(account).executeAsList().map { LocalLibraryRecord(it.resource, it.resource_key, it.body, it.revision) }
            override fun record(resource: String, key: String) = db.libraryQueries.record(account, resource, key).executeAsOneOrNull()?.let { LocalLibraryRecord(it.resource, it.resource_key, it.body, it.revision) }
            override fun put(record: LocalLibraryRecord) { db.libraryQueries.putRecord(account, record.resource, record.key, record.body, record.revision) }
            override fun deleteResource(resource: String) { db.libraryQueries.deleteResource(account, resource) }
            override fun intents() = db.libraryQueries.intents(account).executeAsList().map { LocalLibraryIntent(it.operation, it.resource_key, it.method, it.path, it.body, it.sent != 0L, it.failure) }
            override fun enqueue(intent: LocalLibraryIntent) { db.libraryQueries.insertIntent(account, intent.operation, intent.key, intent.method, intent.path, intent.body) }
            override fun freeze(operation: String, body: String) { db.libraryQueries.freezeIntent(body, account, operation) }
            override fun markSent(operation: String) { db.libraryQueries.markSent(account, operation) }
            override fun fail(operation: String, reason: String) { db.libraryQueries.failIntent(reason, account, operation) }
            override fun resolve(operation: String) { db.libraryQueries.deleteIntent(account, operation) }
            override fun nextSequence(key: String, createDevice: () -> String): Pair<String, Long> {
                val current = db.libraryQueries.sequence(account, key).executeAsOneOrNull()
                check(current?.sequence != Long.MAX_VALUE) { "device sequence exhausted" }
                val installation = db.libraryQueries.record(account, "device", "install").executeAsOneOrNull()?.body
                val device = current?.device ?: installation ?: createDevice()
                if (installation == null) db.libraryQueries.putRecord(account, "device", "install", device, 0)
                check(Regex("[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}").matches(device))
                val sequence = (current?.sequence ?: 0L) + 1
                db.libraryQueries.putSequence(account, key, device, sequence)
                return device to sequence
            }
        })
    }
    override fun close() { driver.close() }
}
