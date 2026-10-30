package com.cantos.theatre.application.errors

import com.cantos.theatre.application.identity.ConfirmedSessionLoss

enum class SessionExpiryPresentation { Banner, Modal, Dismissed }
data class SessionExpiryNotice(val loss: ConfirmedSessionLoss, val presentation: SessionExpiryPresentation)

/*
 * Pure state machine behind the one standard error window (#298). It owns three promises:
 *
 * - at most one notice is [ErrorCenterState.active]: a different failure waits in a bounded queue
 *   and a repeated report of the same [ErrorKey] (many requests, polling, recomposition) is one failure;
 * - a dismissed failure does not reopen while the same failure keeps being reported; it can only
 *   come back after [ErrorEvent.Resolved] (the operation succeeded) or a new user action;
 * - an action is an effect returned as data (Retry, SignIn); the window never repeats a request itself.
 */

const val MaxQueuedErrors: Int = 3

class ErrorCenterState internal constructor(
    val active: ErrorNotice? = null,
    internal val queued: List<ErrorNotice> = emptyList(),
    /** Failure episodes the listener already closed; cleared by a resolution or an explicit action. */
    internal val dismissed: Set<ErrorKey> = emptySet(),
    val sessionExpiry: SessionExpiryNotice? = null,
) {
    val queuedCount: Int get() = queued.size
    val sessionBanner: SessionExpiryNotice? get() = sessionExpiry?.takeIf {
        it.presentation == SessionExpiryPresentation.Banner && active == null
    }

    internal fun withExpiry(expiry: SessionExpiryNotice?) = ErrorCenterState(active, queued, dismissed, expiry)

    override fun equals(other: Any?): Boolean =
        other is ErrorCenterState && active == other.active && queued == other.queued && dismissed == other.dismissed && sessionExpiry == other.sessionExpiry
    override fun hashCode(): Int = listOf(active, queued, dismissed, sessionExpiry).hashCode()
    override fun toString(): String = "ErrorCenterState(active=$active, queued=$queued, dismissed=$dismissed)"
}

sealed interface ErrorEvent {
    /** Only a current, cleaned terminal loss may be shown. Null clears all old epoch notices. */
    data class SessionObserved(val loss: ConfirmedSessionLoss?) : ErrorEvent
    data object PrivateAccessRequested : ErrorEvent
    data object SessionBannerDismissed : ErrorEvent
    data object SessionSignInChosen : ErrorEvent
    /** A failure was observed. [hasContent] says whether usable (possibly stale) content is on screen. */
    data class Reported(val notice: ErrorNotice, val hasContent: Boolean) : ErrorEvent
    /** The operation succeeded, so its failure episode is over. */
    data class Resolved(val key: ErrorKey) : ErrorEvent
    /** The listener closed the window (Close, Back or Escape). */
    data object Dismissed : ErrorEvent
    /** The listener chose an offered action; an action the active notice does not offer is ignored. */
    data class ActionChosen(val action: ErrorAction) : ErrorEvent
}

/** What the host must do next. Nothing is repeated or navigated by the reducer itself. */
sealed interface ErrorEffect {
    data class Retry(val operation: ErrorOperation) : ErrorEffect
    data object OpenSignIn : ErrorEffect
}

class ErrorTransition(val state: ErrorCenterState, val effect: ErrorEffect? = null)

fun reduceErrors(state: ErrorCenterState, event: ErrorEvent): ErrorTransition = when (event) {
    is ErrorEvent.SessionObserved -> ErrorTransition(observeSession(state, event.loss))
    ErrorEvent.PrivateAccessRequested -> ErrorTransition(promptSession(state))
    ErrorEvent.SessionBannerDismissed -> ErrorTransition(state.withExpiry(state.sessionExpiry?.copy(presentation = SessionExpiryPresentation.Dismissed)))
    ErrorEvent.SessionSignInChosen -> if (state.sessionBanner != null) {
        ErrorTransition(state.withExpiry(state.sessionExpiry?.copy(presentation = SessionExpiryPresentation.Dismissed)), ErrorEffect.OpenSignIn)
    } else ErrorTransition(state)
    is ErrorEvent.Reported -> ErrorTransition(report(state, event.notice, event.hasContent))
    is ErrorEvent.Resolved -> ErrorTransition(resolve(state, event.key))
    ErrorEvent.Dismissed -> ErrorTransition(close(state, suppress = true))
    is ErrorEvent.ActionChosen -> choose(state, event.action)
}

private fun report(state: ErrorCenterState, notice: ErrorNotice, hasContent: Boolean): ErrorCenterState {
    if (notice.presentation(hasContent) != ErrorPresentation.Modal) return state
    val key = notice.key
    if (key in state.dismissed || key == state.active?.key || state.queued.any { it.key == key }) return state
    if (state.active == null) return ErrorCenterState(notice, state.queued, state.dismissed, state.sessionExpiry)
    // A storm keeps the window and the bounded queue: the oldest waiting notice is the one kept longest.
    if (state.queued.size >= MaxQueuedErrors) return state
    return ErrorCenterState(state.active, state.queued + notice, state.dismissed, state.sessionExpiry)
}

private fun resolve(state: ErrorCenterState, key: ErrorKey): ErrorCenterState {
    val remaining = state.queued.filterNot { it.key == key }
    val dismissed = state.dismissed - key
    return if (state.active?.key == key) promote(remaining, dismissed, state.sessionExpiry)
    else ErrorCenterState(state.active, remaining, dismissed, state.sessionExpiry)
}

private fun close(state: ErrorCenterState, suppress: Boolean): ErrorCenterState {
    val active = state.active ?: return state
    val dismissed = if (suppress) state.dismissed + active.key else state.dismissed
    val expiry = if (active.kind == ErrorKind.SessionExpired) state.sessionExpiry?.copy(presentation = SessionExpiryPresentation.Dismissed) else state.sessionExpiry
    return promote(state.queued, dismissed, expiry)
}

private fun promote(queued: List<ErrorNotice>, dismissed: Set<ErrorKey>, expiry: SessionExpiryNotice?): ErrorCenterState =
    ErrorCenterState(queued.firstOrNull(), queued.drop(1), dismissed, expiry)

private fun observeSession(state: ErrorCenterState, loss: ConfirmedSessionLoss?): ErrorCenterState {
    if (loss == state.sessionExpiry?.loss) return state
    val pending = state.queued.filterNot { it.kind == ErrorKind.SessionExpired }
    val closed = state.dismissed.filterNot { it.kind == ErrorKind.SessionExpired }.toSet()
    val next = if (state.active?.kind == ErrorKind.SessionExpired) promote(pending, closed, null)
        else ErrorCenterState(state.active, pending, closed)
    return next.withExpiry(loss?.let { SessionExpiryNotice(it, SessionExpiryPresentation.Banner) })
}

private fun promptSession(state: ErrorCenterState): ErrorCenterState {
    val expiry = state.sessionExpiry ?: return state
    if (expiry.presentation != SessionExpiryPresentation.Banner) return state
    val notice = ErrorNotice(ErrorKind.SessionExpired, ErrorOperation.SignIn, retryable = false, epoch = expiry.loss.epoch)
    val reported = report(state, notice, hasContent = true)
    // A full queue did not accept the prompt. Keep the banner available after the other errors close.
    if (reported.active?.key != notice.key && reported.queued.none { it.key == notice.key }) return state
    return reported.withExpiry(expiry.copy(presentation = SessionExpiryPresentation.Modal))
}

private fun choose(state: ErrorCenterState, action: ErrorAction): ErrorTransition {
    val active = state.active ?: return ErrorTransition(state)
    if (action !in active.actions) return ErrorTransition(state)
    return when (action) {
        ErrorAction.Close -> ErrorTransition(close(state, suppress = true))
        // An explicit retry is a new attempt: a failure after it is a new error and may reopen the window.
        ErrorAction.Retry -> ErrorTransition(close(state, suppress = false), ErrorEffect.Retry(active.operation))
        ErrorAction.SignIn -> ErrorTransition(close(state, suppress = true), ErrorEffect.OpenSignIn)
    }
}
