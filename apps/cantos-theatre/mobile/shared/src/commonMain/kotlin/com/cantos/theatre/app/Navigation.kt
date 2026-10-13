package com.cantos.theatre.app

import com.cantos.theatre.domain.publication.WorkKey

sealed interface TheatreRoute {
    data object Discovery : TheatreRoute
    data object Search : TheatreRoute
    data object Library : TheatreRoute
    data object Account : TheatreRoute
    data object History : TheatreRoute
    data object Settings : TheatreRoute
    data object Queue : TheatreRoute
    data object Player : TheatreRoute
    data object Unavailable : TheatreRoute
    data class Work(val key: WorkKey) : TheatreRoute
}

class NavigationState(stack: List<TheatreRoute> = listOf(TheatreRoute.Discovery)) {
    private val stackSnapshot = stack.toList()
    val stack: List<TheatreRoute> get() = stackSnapshot.toList()
    val route: TheatreRoute get() = stackSnapshot.last()
    val canGoBack: Boolean get() = stackSnapshot.size > 1

    init {
        require(stackSnapshot.isNotEmpty()) { "Navigation must have a root" }
    }

    override fun equals(other: Any?): Boolean = other is NavigationState && stackSnapshot == other.stackSnapshot
    override fun hashCode(): Int = stackSnapshot.hashCode()
    override fun toString(): String = "NavigationState(stack=$stackSnapshot)"
}

sealed interface NavigationEvent {
    data class Navigate(val route: TheatreRoute) : NavigationEvent
    data object Back : NavigationEvent
    data object OpenPlayer : NavigationEvent
    data object Reset : NavigationEvent
}

fun reduceNavigation(state: NavigationState, event: NavigationEvent): NavigationState = when (event) {
    is NavigationEvent.Navigate -> navigate(state, event.route)
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

private fun navigate(state: NavigationState, route: TheatreRoute): NavigationState = when (route) {
    TheatreRoute.Discovery, TheatreRoute.Library -> NavigationState(listOf(route))
    TheatreRoute.Search -> NavigationState(listOf(TheatreRoute.Discovery, TheatreRoute.Search))
    TheatreRoute.History -> NavigationState(listOf(TheatreRoute.Library, TheatreRoute.History))
    TheatreRoute.Account, TheatreRoute.Settings, TheatreRoute.Queue, TheatreRoute.Player, TheatreRoute.Unavailable, is TheatreRoute.Work -> {
        val existingIndex = state.stack.indexOf(route)
        NavigationState(if (existingIndex >= 0) state.stack.take(existingIndex + 1) else state.stack + route)
    }
}
