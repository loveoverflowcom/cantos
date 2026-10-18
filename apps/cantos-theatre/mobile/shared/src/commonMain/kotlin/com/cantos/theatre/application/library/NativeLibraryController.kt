package com.cantos.theatre.application.library

import com.cantos.theatre.ports.LibrarySessionPort

/** Coordinates one native Library session through a domain-only capability boundary. */
class NativeLibraryController(private val session: LibrarySessionPort) : LibrarySessionPort by session
