package com.cantos.theatre.adapters.library

import app.cash.sqldelight.driver.jdbc.sqlite.JdbcSqliteDriver
import com.cantos.theatre.db.LibraryDatabase
import java.util.Properties

/** File-backed SQLite for desktop integration/restart tests; production native hosts use their drivers. */
fun desktopLibraryStore(databaseFile: String): SqlLibraryStore = SqlLibraryStore(
    JdbcSqliteDriver("jdbc:sqlite:$databaseFile", Properties(), LibraryDatabase.Schema),
)
