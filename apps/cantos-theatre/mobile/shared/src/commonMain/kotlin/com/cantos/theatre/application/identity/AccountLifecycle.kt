package com.cantos.theatre.application.identity

import com.cantos.theatre.ports.CredentialFailure
import com.cantos.theatre.ports.CredentialStore

class AccountSession private constructor(val actor: String, val session: String) {
    companion object {
        fun fromServer(actor: String, session: String): AccountSession? =
            if (actor.isNotEmpty() && actor.length <= 128 && actor.none { it.isISOControl() } && Regex("[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}").matches(session)) AccountSession(actor, session) else null
    }
}
class AccountFence internal constructor(internal val generation: Long, internal val session: AccountSession)
class AccountIntent internal constructor(val owner: String, val operation: String, val body: String, val readReceiptFirst: Boolean)
enum class AccountFailure { SignedOut, WrongAccount, InvalidIntent, DuplicateIntent, Exhausted }
sealed interface IntentDispatch {
    class Ready(val fence: AccountFence, val intent: AccountIntent) : IntentDispatch
    data class Refused(val reason: AccountFailure) : IntentDispatch
}
/** Memory lifecycle helper, not a durable offline store. Never stores credentials. */
class AccountLifecycle {
    private var generation = 0L
    private var current: AccountSession? = null
    private var pending: Map<String, AccountIntent> = emptyMap()
    var projection: String? = null
        private set
    fun leave(): AccountFailure? {
        current = null
        projection = null
        if (generation == Long.MAX_VALUE) return AccountFailure.Exhausted
        generation += 1
        return null
    }
    fun enter(session: AccountSession): AccountFailure? {
        leave()?.let { return it }
        current = session
        return null
    }
    fun fence(): AccountFence? = current?.let { AccountFence(generation, it) }
    fun accepts(fence: AccountFence): Boolean = generation == fence.generation && current === fence.session
    fun project(fence: AccountFence, value: String): Boolean {
        if (!accepts(fence)) return false
        projection = value
        return true
    }
    fun enqueue(operation: String, body: String): AccountFailure? {
        val owner = current ?: return AccountFailure.SignedOut
        if (!Regex("[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}").matches(operation)) return AccountFailure.InvalidIntent
        if (operation in pending) return AccountFailure.DuplicateIntent
        pending = pending + (operation to AccountIntent(owner.actor, operation, body, false))
        return null
    }
    fun dispatch(operation: String): IntentDispatch {
        val fence = fence() ?: return IntentDispatch.Refused(AccountFailure.SignedOut)
        val intent = pending[operation] ?: return IntentDispatch.Refused(AccountFailure.InvalidIntent)
        if (intent.owner != fence.session.actor) return IntentDispatch.Refused(AccountFailure.WrongAccount)
        pending = pending + (operation to AccountIntent(intent.owner, intent.operation, intent.body, true))
        return IntentDispatch.Ready(fence, intent)
    }
    fun acknowledge(fence: AccountFence, operation: String): Boolean {
        if (!accepts(fence) || pending[operation]?.owner != fence.session.actor) return false
        pending = pending - operation
        return true
    }
    fun logout(store: CredentialStore): CredentialFailure? {
        leave()
        // UI is already signed out even if OS deletion fails; do not silently restore afterward.
        return store.clear()
    }
}
