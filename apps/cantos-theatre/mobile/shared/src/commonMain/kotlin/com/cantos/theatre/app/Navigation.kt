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
    /** Neutral locator only: no title, credential, cursor or cached private metadata. */
    data class SharedWork(val id: com.cantos.theatre.domain.catalog.PublicWorkId) : TheatreRoute

    /** Optional account intents. A route confers no identity; see [NavigationState]. */
    data object Login : TheatreRoute
    data object Register : TheatreRoute

    /**
     * Completing an emailed signup. Only a received link opens it; it is a transient top route that
     * any other navigation removes, and it is reachable while signed in.
     */
    data object VerifyEmail : TheatreRoute
}

val TheatreRoute.isAuth: Boolean get() = this == TheatreRoute.Login || this == TheatreRoute.Register

/**
 * Auth is a closed, optional suffix: `[…, Login]` or `[…, Login, Register]`, never the root.
 * The routes below it are the typed return destination, so Back needs no caller-supplied URL
 * and a cold start can never restore a login wall. [TheatreRoute.VerifyEmail] is only ever the
 * top above a non-auth root, so leaving it never reveals a screen whose link was forgotten.
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
        val verification = stackSnapshot.indexOf(TheatreRoute.VerifyEmail)
        if (verification >= 0) {
            require(verification > 0 && verification == stackSnapshot.lastIndex && firstAuth < 0) {
                "Email verification is only the top above a non-auth stack"
            }
        }
    }

    internal fun withoutAuth(): NavigationState =
        if (stackSnapshot.none { it.isAuth }) this else NavigationState(stackSnapshot.filterNot { it.isAuth })

    /** Every destination other than the verification screen itself leaves it first. */
    internal fun leavingVerification(): NavigationState =
        if (route != TheatreRoute.VerifyEmail) this else NavigationState(stackSnapshot.dropLast(1))

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
    /** A received signup link: closes an open auth intent and shows completion on top. Idempotent. */
    data object OpenVerifyEmail : NavigationEvent
}

fun reduceNavigation(state: NavigationState, event: NavigationEvent): NavigationState = when (event) {
    is NavigationEvent.Navigate -> navigate(state.leavingVerification(), event.route)
    NavigationEvent.OpenLogin -> openLogin(state.leavingVerification())
    NavigationEvent.OpenRegister -> openRegister(state.leavingVerification())
    // Signing in elsewhere keeps an open completion: it is allowed while signed in.
    NavigationEvent.AuthCompleted -> state.withoutAuth()
    NavigationEvent.Back -> if (state.canGoBack) NavigationState(state.stack.dropLast(1)) else state
    NavigationEvent.OpenPlayer -> {
        val current = state.leavingVerification()
        val parent = if (current.route == TheatreRoute.Queue && current.canGoBack) {
            NavigationState(current.stack.dropLast(1))
        } else {
            current
        }
        navigate(parent, TheatreRoute.Player)
    }
    NavigationEvent.Reset -> NavigationState()
    NavigationEvent.OpenVerifyEmail -> state.withoutAuth().let {
        if (it.route == TheatreRoute.VerifyEmail) it else NavigationState(it.stack + TheatreRoute.VerifyEmail)
    }
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
    // Only a received link opens completion, through [NavigationEvent.OpenVerifyEmail].
    TheatreRoute.VerifyEmail -> state
    TheatreRoute.Discovery, TheatreRoute.Library -> NavigationState(listOf(route))
    TheatreRoute.Search -> NavigationState(listOf(TheatreRoute.Discovery, TheatreRoute.Search))
    TheatreRoute.History -> NavigationState(listOf(TheatreRoute.Library, TheatreRoute.History))
    TheatreRoute.Account, TheatreRoute.MembershipPreview, TheatreRoute.Settings, TheatreRoute.Queue, TheatreRoute.Player, TheatreRoute.Unavailable, is TheatreRoute.Work, is TheatreRoute.SharedWork -> {
        // Leaving auth closes it first, so no destination is ever stacked above a Login intent.
        val base = state.withoutAuth()
        val existingIndex = base.stack.indexOf(route)
        NavigationState(if (existingIndex >= 0) base.stack.take(existingIndex + 1) else base.stack + route)
    }
}
