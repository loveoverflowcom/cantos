package com.cantos.theatre.adapters.preview

import com.cantos.theatre.adapters.library.desktopLibraryStore
import com.cantos.theatre.ports.LibraryLocalPort

/**
 * The production SQLite Library store on an in-memory database: the real transactions, outbox and
 * account partitions, discarded with the mount. It is not evidence of disk persistence or of
 * process-death recovery; the workbench "Restored" fixture is a seed, never a reopened file.
 */
fun previewLibraryStore(): LibraryLocalPort = desktopLibraryStore(":memory:")
