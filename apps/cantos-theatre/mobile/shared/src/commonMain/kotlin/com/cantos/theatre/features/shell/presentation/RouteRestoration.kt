package com.cantos.theatre.features.shell.presentation

import androidx.compose.foundation.ScrollState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.focus.FocusRequester
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.isAuth
import com.cantos.theatre.domain.membership.MembershipScreen

/** Entry controls that Back returns to after closing an auth intent. */
internal class AuthEntryFocus(val account: FocusRequester, val login: FocusRequester)

/**
 * Back that closes auth restores focus to the control that opened it: the header avatar on a
 * Home origin, or Sign in on Account. Success also closes auth, but the entry may no longer exist
 * (signed-in Account has no Sign in), so only a shrinking stack is restored and a missing target
 * is a no-op rather than a stolen or invented focus.
 */
@Composable
internal fun rememberAuthEntryFocus(stack: List<ShellRoute>): AuthEntryFocus {
    val entry = remember { AuthEntryFocus(FocusRequester(), FocusRequester()) }
    var previous by remember { mutableStateOf(stack) }
    LaunchedEffect(stack) {
        val before = previous
        previous = stack
        if (before.last().isAuth() && !stack.last().isAuth() && stack.size < before.size) {
            withFrameNanos { }
            runCatching { (if (stack.last() == ShellRoute.Account) entry.login else entry.account).requestFocus() }
        }
    }
    return entry
}

/**
 * Each stack entry keeps its scroll position, so Back (including from auth) returns to the place
 * the listener left. A destination that leaves the stack is forgotten and reopens at the top.
 */
@Composable
internal fun rememberRouteScroll(stack: List<ShellRoute>, membershipScreen: MembershipScreen): ScrollState {
    val scrolls = remember { mutableMapOf<Pair<ShellRoute, MembershipScreen?>, ScrollState>() }
    val route = stack.last()
    val key = route to (if (route == ShellRoute.MembershipPreview) membershipScreen else null)
    val scroll = scrolls.getOrPut(key) { ScrollState(0) }
    SideEffect { scrolls.keys.retainAll { it.first in stack } }
    return scroll
}
