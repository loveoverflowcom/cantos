package com.cantos.theatre.app

import com.cantos.theatre.domain.publication.WorkKey

sealed interface TheatreRoute {
    data object Landing : TheatreRoute
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
class NavigationState(
    stack: List<TheatreRoute> = listOf(TheatreRoute.Landing),
    val returnTo: PrivateDestination? = null,
) {
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
        if (stackSnapshot.none { it.isAuth }) this else NavigationState(stackSnapshot.filterNot { it.isAuth }, returnTo)

    /** Every destination other than the verification screen itself leaves it first. */
    internal fun leavingVerification(): NavigationState =
        if (route != TheatreRoute.VerifyEmail) this else NavigationState(stackSnapshot.dropLast(1), returnTo)

    override fun equals(other: Any?): Boolean = other is NavigationState && stackSnapshot == other.stackSnapshot && returnTo == other.returnTo
    override fun hashCode(): Int = 31 * stackSnapshot.hashCode() + (returnTo?.hashCode() ?: 0)
    override fun toString(): String = "NavigationState(stack=$stackSnapshot, returnTo=$returnTo)"
}

/** A projection supplied by the account owner on every reduction, never a second session authority. */
enum class NavigationAdmission { Guest, Unresolved, ConfirmedLoss, SignedIn }

/** The closed private return allowlist contains no URL, credential or account presentation. */
enum class PrivateDestination(val route: TheatreRoute) {
    Library(TheatreRoute.Library), History(TheatreRoute.History), Account(TheatreRoute.Account);

    companion object {
        fun forRoute(route: TheatreRoute): PrivateDestination? = entries.firstOrNull { it.route == route }
    }
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
    /** Re-read current admission, preserving unresolved intended destinations. */
    data object ReconcileSession : NavigationEvent
    /** Confirmed loss or explicit logout has fenced the old account; discard its private stack. */
    data object SessionEnded : NavigationEvent
}

fun reduceNavigation(
    state: NavigationState,
    event: NavigationEvent,
    admission: NavigationAdmission = NavigationAdmission.Guest,
): NavigationState = when (event) {
    is NavigationEvent.Navigate -> navigate(reconcile(state, admission).leavingVerification(), event.route, admission)
    NavigationEvent.OpenLogin -> openLogin(reconcile(state, admission).leavingVerification())
    NavigationEvent.OpenRegister -> openRegister(reconcile(state, admission).leavingVerification())
    // A late completion has no routing authority after its auth suffix was cancelled.
    NavigationEvent.AuthCompleted -> if (state.route.isAuth && admission == NavigationAdmission.SignedIn) completeAuth(state) else state
    NavigationEvent.ReconcileSession -> if (state.route.isAuth && admission == NavigationAdmission.SignedIn) completeAuth(state) else reconcile(state, admission)
    NavigationEvent.SessionEnded -> guestSafe(state.withoutAuth(), keepReturn = false)
    NavigationEvent.Back -> {
        val current = reconcile(state, admission)
        if (current.canGoBack) NavigationState(current.stack.dropLast(1), current.returnTo.takeUnless { current.route == TheatreRoute.Login }) else current
    }
    NavigationEvent.OpenPlayer -> {
        val current = reconcile(state, admission).leavingVerification()
        val parent = if (current.route == TheatreRoute.Queue && current.canGoBack) {
            NavigationState(current.stack.dropLast(1))
        } else {
            current
        }
        navigate(parent, TheatreRoute.Player, admission)
    }
    NavigationEvent.Reset -> NavigationState(listOf(root(admission)))
    NavigationEvent.OpenVerifyEmail -> reconcile(state, admission).withoutAuth().let {
        if (it.route == TheatreRoute.VerifyEmail) it else NavigationState(it.stack + TheatreRoute.VerifyEmail)
    }
}

private fun root(admission: NavigationAdmission): TheatreRoute =
    if (admission == NavigationAdmission.SignedIn) TheatreRoute.Discovery else TheatreRoute.Landing

private fun completeAuth(state: NavigationState): NavigationState {
    val source = reconcile(state.withoutAuth(), NavigationAdmission.SignedIn)
    return state.returnTo?.let { navigate(source, it.route, NavigationAdmission.SignedIn) }
        ?: NavigationState(source.stack)
}

private fun reconcile(state: NavigationState, admission: NavigationAdmission): NavigationState = when (admission) {
    NavigationAdmission.Guest, NavigationAdmission.ConfirmedLoss -> guestSafe(state, keepReturn = true)
    NavigationAdmission.Unresolved -> state // Preserve the destination; the renderer shows account recovery for private routes.
    NavigationAdmission.SignedIn -> NavigationState(state.stack.map { if (it == TheatreRoute.Landing) TheatreRoute.Discovery else it }, state.returnTo)
}

private fun guestSafe(state: NavigationState, keepReturn: Boolean): NavigationState {
    val intended = if (keepReturn) state.returnTo ?: state.stack.asReversed().firstNotNullOfOrNull(PrivateDestination::forRoute) else null
    val public = state.stack.filter { PrivateDestination.forRoute(it) == null }
        .map { if (it == TheatreRoute.Discovery) TheatreRoute.Landing else it }
    val rooted = if (public.firstOrNull() == TheatreRoute.Landing) public else listOf(TheatreRoute.Landing) + public
    return NavigationState(rooted.distinct(), intended)
}

private fun openLogin(state: NavigationState): NavigationState {
    val index = state.stack.indexOf(TheatreRoute.Login)
    return if (index >= 0) NavigationState(state.stack.take(index + 1), state.returnTo) else NavigationState(state.stack + TheatreRoute.Login, state.returnTo)
}

private fun openRegister(state: NavigationState): NavigationState {
    val login = openLogin(state)
    return if (state.stack.contains(TheatreRoute.Register)) state else NavigationState(login.stack + TheatreRoute.Register, state.returnTo)
}

private fun navigate(state: NavigationState, route: TheatreRoute, admission: NavigationAdmission): NavigationState {
    val destination = PrivateDestination.forRoute(route)
    if (destination != null && (admission == NavigationAdmission.Guest || admission == NavigationAdmission.ConfirmedLoss)) {
        return NavigationState(returnTo = destination)
    }
    return when (route) {
        TheatreRoute.Login -> openLogin(state)
        TheatreRoute.Register -> openRegister(state)
        // Only a received link opens completion, through [NavigationEvent.OpenVerifyEmail].
        TheatreRoute.VerifyEmail -> state
        TheatreRoute.Landing, TheatreRoute.Discovery -> NavigationState(listOf(root(admission)))
        TheatreRoute.Library -> NavigationState(listOf(route))
        TheatreRoute.Search -> NavigationState(listOf(root(admission), TheatreRoute.Search))
        TheatreRoute.History -> NavigationState(listOf(TheatreRoute.Library, TheatreRoute.History))
        TheatreRoute.Account, TheatreRoute.MembershipPreview, TheatreRoute.Settings, TheatreRoute.Queue, TheatreRoute.Player, TheatreRoute.Unavailable, is TheatreRoute.Work, is TheatreRoute.SharedWork -> {
            // Leaving auth closes it first, so no destination is ever stacked above a Login intent.
            val base = state.withoutAuth()
            val existingIndex = base.stack.indexOf(route)
            NavigationState(if (existingIndex >= 0) base.stack.take(existingIndex + 1) else base.stack + route)
        }
    }
}
