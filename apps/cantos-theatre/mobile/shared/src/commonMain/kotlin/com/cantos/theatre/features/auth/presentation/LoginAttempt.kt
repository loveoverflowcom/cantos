package com.cantos.theatre.features.auth.presentation

import com.cantos.theatre.application.identity.AccountView

/**
 * Whether the owner's failure is feedback for a credential attempt made on the Login entry. The
 * owner's `Failed` state outlives the route, but that message belongs to Login: the guest Account
 * keeps every other failure (a restore, a lost session) and every cleanup or pending state. Only
 * the app root advances it; nothing here touches the owner or its session.
 */
sealed interface LoginAttempt {
    /** No attempt from Login is waiting for, or holding, a result. */
    data object None : LoginAttempt

    /** A login was accepted from the Login entry and has not settled. */
    data object Open : LoginAttempt

    /** That attempt settled as [failure]; it remains Login feedback while the owner still shows it. */
    data class Settled(val failure: AccountView.Failed) : LoginAttempt

    /** Follows the owner: anything but the attempt's own pending state or failure ends the scope. */
    fun after(account: AccountView): LoginAttempt = when (this) {
        None -> None
        Open -> when (account) {
            is AccountView.Failed -> Settled(account)
            AccountView.Pending -> Open
            AccountView.SignedOut, AccountView.SignedIn, AccountView.CleanupRequired, AccountView.Unavailable -> None
        }
        is Settled -> if (account == failure) this else None
    }

    /** True only for this attempt's own failure, which Login shows and the guest Account does not. */
    fun owns(account: AccountView): Boolean = this is Settled && account == failure
}
