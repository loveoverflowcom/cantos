package com.cantos.theatre.adapters.library

import app.cash.sqldelight.driver.native.NativeSqliteDriver
import com.cantos.theatre.db.LibraryDatabase

/** Host supplies its backup-excluded Application Support directory, never Caches. */
fun iosLibraryStore(applicationSupportDirectory: String): SqlLibraryStore {
    require(applicationSupportDirectory.startsWith("/") && !applicationSupportDirectory.endsWith("/"))
    return SqlLibraryStore(NativeSqliteDriver(LibraryDatabase.Schema, "cantos-library-v1.sqlite", onConfiguration = { it.copy(extendedConfig = it.extendedConfig.copy(basePath = applicationSupportDirectory)) }))
}
