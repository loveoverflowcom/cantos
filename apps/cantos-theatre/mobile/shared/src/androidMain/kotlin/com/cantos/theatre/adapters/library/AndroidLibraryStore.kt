package com.cantos.theatre.adapters.library

import android.content.Context
import app.cash.sqldelight.driver.android.AndroidSqliteDriver
import com.cantos.theatre.db.LibraryDatabase

/** Database, rollback journal and WAL all remain in the app-private no-backup directory. */
fun androidLibraryStore(context: Context): SqlLibraryStore {
    val database = java.io.File(context.noBackupFilesDir, "cantos-library-v1.sqlite")
    return SqlLibraryStore(AndroidSqliteDriver(LibraryDatabase.Schema, context, database.absolutePath))
}
