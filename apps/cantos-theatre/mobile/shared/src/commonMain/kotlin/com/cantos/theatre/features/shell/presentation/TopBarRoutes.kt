package com.cantos.theatre.features.shell.presentation

import com.cantos.theatre.presentation.model.ShellRoute

/**
 * Top-level destinations carry the official brand mark before the wordmark, with Back first when
 * the stack has one; every drill-down route keeps Back (Close for the player) and the title only.
 */
internal fun topBarShowsBrandMark(route: ShellRoute): Boolean = when (route) {
    ShellRoute.Discovery, ShellRoute.Search, ShellRoute.Library, ShellRoute.History -> true
    else -> false
}
