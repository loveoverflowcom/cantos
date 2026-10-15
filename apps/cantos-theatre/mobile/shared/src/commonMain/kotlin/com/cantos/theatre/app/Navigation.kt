package com.cantos.theatre.app

import com.cantos.theatre.domain.publication.WorkKey

sealed interface TheatreRoute {
    data object Discovery : TheatreRoute
    data object Search : TheatreRoute
    data object Library : TheatreRoute
    data object Account : TheatreRoute
    data object MembershipPreview : TheatreRoute
    data object History : TheatreRoute
    data object Settings : TheatreRoute
    data object Queue : TheatreRoute
    data object Player : TheatreRoute
    data object Unavailable : TheatreRoute
    data class Work(val key: WorkKey) : TheatreRoute

    /** Optional account intents. A route confers no identity; see [NavigationState]. */
    data object Login : TheatreRoute
    data object Register : TheatreRoute
}

val TheatreRoute.isAuth: Boolean get() = this == TheatreRoute.Login || this == TheatreRoute.Register

/**
 * Auth is a closed, optional suffix: `[…, Login]` or `[…, Login, Register]`, never the root.
 * The routes below it are the typed return destination, so Back needs no caller-supplied URL
 * and a cold start can never restore a login wall.
 */
class NavigationState(stack: List<TheatreRoute> = listOf(TheatreRoute.Discovery)) {
    private val stackSnapshot = stack.toList()
    val stack: List<TheatreRoute> get() = stackSnapshot.toList()
    val route: TheatreRoute get() = stackSnapshot.last()
    val canGoBack: Boolean get() = stackSnapshot.size > 1

    init {
        require(stackSnapshot.isNotEmpty()) { "Navigation must have a root" }
        val firstAuth = stackSnapshot.indexOfFirst { it.isAuth }
        if (firstAuth >= 0) {
            require(firstAuth > 0) { "Auth cannot be the navigation root" }
            val suffix = stackSnapshot.drop(firstAuth)
            require(suffix == listOf(TheatreRoute.Login) || suffix == listOf(TheatreRoute.Login, TheatreRoute.Register)) {
                "Auth routes are only the closed Login[, Register] suffix"
            }
        }
    }

    internal fun withoutAuth(): NavigationState =
        if (stackSnapshot.none { it.isAuth }) this else NavigationState(stackSnapshot.filterNot { it.isAuth })

    override fun equals(other: Any?): Boolean = other is NavigationState && stackSnapshot == other.stackSnapshot
    override fun hashCode(): Int = stackSnapshot.hashCode()
    override fun toString(): String = "NavigationState(stack=$stackSnapshot)"
}

sealed interface NavigationEvent {
    data class Navigate(val route: TheatreRoute) : NavigationEvent
    data object Back : NavigationEvent
    data object OpenPlayer : NavigationEvent
    data object Reset : NavigationEvent

    /** An explicit listener intent; the stack below is the return destination. Idempotent. */
    data object OpenLogin : NavigationEvent
    /** Registration without a Login context first supplies Login, so Back never loops. */
    data object OpenRegister : NavigationEvent
    /** A confirmed session closes only an auth intent that is still open; otherwise inert. */
    data object AuthCompleted : NavigationEvent
}

fun reduceNavigation(state: NavigationState, event: NavigationEvent): NavigationState = when (event) {
    is NavigationEvent.Navigate -> navigate(state, event.route)
    NavigationEvent.OpenLogin -> openLogin(state)
    NavigationEvent.OpenRegister -> openRegister(state)
    NavigationEvent.AuthCompleted -> state.withoutAuth()
    NavigationEvent.Back -> if (state.canGoBack) NavigationState(state.stack.dropLast(1)) else state
    NavigationEvent.OpenPlayer -> {
        val parent = if (state.route == TheatreRoute.Queue && state.canGoBack) {
            NavigationState(state.stack.dropLast(1))
        } else {
            state
        }
        navigate(parent, TheatreRoute.Player)
    }
    NavigationEvent.Reset -> NavigationState()
}

private fun openLogin(state: NavigationState): NavigationState {
    val index = state.stack.indexOf(TheatreRoute.Login)
    return if (index >= 0) NavigationState(state.stack.take(index + 1)) else NavigationState(state.stack + TheatreRoute.Login)
}

private fun openRegister(state: NavigationState): NavigationState {
    val login = openLogin(state)
    return if (state.stack.contains(TheatreRoute.Register)) state else NavigationState(login.stack + TheatreRoute.Register)
}

private fun navigate(state: NavigationState, route: TheatreRoute): NavigationState = when (route) {
    TheatreRoute.Login -> openLogin(state)
    TheatreRoute.Register -> openRegister(state)
    TheatreRoute.Discovery, TheatreRoute.Library -> NavigationState(listOf(route))
    TheatreRoute.Search -> NavigationState(listOf(TheatreRoute.Discovery, TheatreRoute.Search))
    TheatreRoute.History -> NavigationState(listOf(TheatreRoute.Library, TheatreRoute.History))
    TheatreRoute.Account, TheatreRoute.MembershipPreview, TheatreRoute.Settings, TheatreRoute.Queue, TheatreRoute.Player, TheatreRoute.Unavailable, is TheatreRoute.Work -> {
        // Leaving auth closes it first, so no destination is ever stacked above a Login intent.
        val base = state.withoutAuth()
        val existingIndex = base.stack.indexOf(route)
        NavigationState(if (existingIndex >= 0) base.stack.take(existingIndex + 1) else base.stack + route)
    }
}
