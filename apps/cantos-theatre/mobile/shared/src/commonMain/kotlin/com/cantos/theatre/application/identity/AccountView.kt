package com.cantos.theatre.application.identity

/**
 * Immutable presentation of the account boundary. It carries no actor, session, credential,
 * expiry or page count: nothing here can be shown as a name or a library total.
 */
sealed interface AccountView {
    /** No identity boundary is configured. The guest shell stays complete; this is not a failure. */
    data object Unavailable : AccountView
    data object SignedOut : AccountView
    data object Pending : AccountView
    data object SignedIn : AccountView
    /** An earlier credential is not yet acknowledged as removed; login stays blocked. */
    data object CleanupRequired : AccountView
    data class Failed(val reason: NativeAccountFailure) : AccountView

    /** Only a resting guest state may start a login; the owner still refuses anything else. */
    val canBeginLogin: Boolean get() = this == SignedOut || this is Failed
}

fun NativeAccountState?.toAccountView(): AccountView = when (this) {
    null -> AccountView.Unavailable
    NativeAccountState.SignedOut -> AccountView.SignedOut
    NativeAccountState.Pending -> AccountView.Pending
    is NativeAccountState.SignedIn -> AccountView.SignedIn
    NativeAccountState.CleanupRequired -> AccountView.CleanupRequired
    is NativeAccountState.Failed -> AccountView.Failed(reason)
}
