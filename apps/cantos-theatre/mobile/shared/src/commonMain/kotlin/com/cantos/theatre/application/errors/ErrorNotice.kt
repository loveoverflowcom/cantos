package com.cantos.theatre.application.errors

import com.cantos.theatre.application.catalog.CatalogFailure
import com.cantos.theatre.ports.PlaybackAccessFailure

/*
 * The one error presentation contract for the native listener (#298). Adapters map HTTP/domain
 * outcomes into an [ErrorNotice] here; the UI renders only this closed vocabulary and never a raw
 * exception, SQL or provider text. Pure values: no Compose, no platform, no retry engine.
 */

/** What went wrong, in the terms a listener can act on. */
enum class ErrorKind {
    /** Confirmed loss from the native account owner, scoped to its nonsecret process epoch. */
    SessionExpired,
    /** No response: DNS, TLS, connection or an unreachable API. */
    Network,
    Timeout,
    /** 401: the action needs a signed-in listener. */
    Unauthenticated,
    /** 403: the signed-in listener is refused. */
    Forbidden,
    /** The content was withdrawn, replaced or never public. Never retried blindly. */
    ContentUnavailable,
    /** A malformed, oversized or contract-violating response. */
    Protocol,
    /** The host configuration is refused; only a different build can change it. */
    Configuration,
    /** Local durable storage failed. */
    Storage,
    /** The API answered that it cannot serve right now (5xx, 429). */
    ServiceUnavailable,
}

/** Where the failure belongs in the listener flow: whole host, current screen section or one user action. */
enum class ErrorScope { Host, Section, Action }

/** Closed localization key for the standard error window; never raw server or exception text. */
enum class ErrorMessageKey {
    SessionExpired,
    Network,
    Timeout,
    Unauthenticated,
    Forbidden,
    ContentUnavailable,
    Protocol,
    Configuration,
    Storage,
    ServiceUnavailable,
}

/** The operation a failure belongs to; it scopes dedupe and names what a retry would repeat. */
enum class ErrorOperation { LoadCatalog, LoadWork, LoadUnits, StartPlayback, UpdateLibrary, SignIn }

/** The only actions a notice can offer. [Close] is always present: one way out, never a trap. */
enum class ErrorAction { Retry, SignIn, Close }

/** Whether a failure needs the standard window or is already honest inline where it happened. */
enum class ErrorPresentation { Inline, Modal }

/** Two notices with the same key are one failure, however many requests report it. */
data class ErrorKey(val kind: ErrorKind, val operation: ErrorOperation, val epoch: Long? = null)

/**
 * A mapped failure. [retryable] is the adapter's knowledge (for example `Unavailable(retryable)`);
 * the actions and presentation are derived, so a caller cannot offer Retry for a configuration
 * refusal or forget Close.
 */
class ErrorNotice(
    val kind: ErrorKind,
    val operation: ErrorOperation,
    val retryable: Boolean = true,
    parameters: Map<String, String> = emptyMap(),
    epoch: Long? = null,
) {
    val key: ErrorKey = ErrorKey(kind, operation, epoch)
    val parameters: Map<String, String> = parameters.toMap()

    val scope: ErrorScope = when {
        kind == ErrorKind.Configuration -> ErrorScope.Host
        operation == ErrorOperation.LoadCatalog || operation == ErrorOperation.LoadWork || operation == ErrorOperation.LoadUnits -> ErrorScope.Section
        else -> ErrorScope.Action
    }

    val messageKey: ErrorMessageKey = when (kind) {
        ErrorKind.SessionExpired -> ErrorMessageKey.SessionExpired
        ErrorKind.Network -> ErrorMessageKey.Network
        ErrorKind.Timeout -> ErrorMessageKey.Timeout
        ErrorKind.Unauthenticated -> ErrorMessageKey.Unauthenticated
        ErrorKind.Forbidden -> ErrorMessageKey.Forbidden
        ErrorKind.ContentUnavailable -> ErrorMessageKey.ContentUnavailable
        ErrorKind.Protocol -> ErrorMessageKey.Protocol
        ErrorKind.Configuration -> ErrorMessageKey.Configuration
        ErrorKind.Storage -> ErrorMessageKey.Storage
        ErrorKind.ServiceUnavailable -> ErrorMessageKey.ServiceUnavailable
    }

    /** Primary first; always ends with [ErrorAction.Close]. */
    val actions: List<ErrorAction> = when (kind) {
        ErrorKind.SessionExpired, ErrorKind.Unauthenticated -> listOf(ErrorAction.SignIn, ErrorAction.Close)
        ErrorKind.Forbidden, ErrorKind.ContentUnavailable, ErrorKind.Configuration -> listOf(ErrorAction.Close)
        ErrorKind.Network, ErrorKind.Timeout, ErrorKind.Storage, ErrorKind.Protocol, ErrorKind.ServiceUnavailable ->
            if (retryable && operation != ErrorOperation.UpdateLibrary && kind != ErrorKind.Storage) listOf(ErrorAction.Retry, ErrorAction.Close) else listOf(ErrorAction.Close)
    }

    val primary: ErrorAction get() = actions.first()

    /**
     * Listener decisions and configuration need the window. A transport/service failure with content
     * already on screen stays inline with its stale/offline label; without content a Retry is needed.
     * Withdrawn content has its own not-found state.
     */
    fun presentation(hasContent: Boolean): ErrorPresentation = when {
        kind == ErrorKind.SessionExpired || kind == ErrorKind.Configuration || kind == ErrorKind.Unauthenticated || kind == ErrorKind.Forbidden -> ErrorPresentation.Modal
        hasContent || kind == ErrorKind.ContentUnavailable -> ErrorPresentation.Inline
        else -> ErrorPresentation.Modal
    }

    override fun equals(other: Any?): Boolean =
        other is ErrorNotice && key == other.key && retryable == other.retryable && parameters == other.parameters
    override fun hashCode(): Int = 31 * (31 * key.hashCode() + retryable.hashCode()) + parameters.hashCode()
    override fun toString(): String = "ErrorNotice($kind, $operation, retryable=$retryable)"
}

/** HTTP status → kind. An unlisted status is a contract violation, not a generic success or retry. */
fun errorKindForHttpStatus(status: Int): ErrorKind = when (status) {
    401 -> ErrorKind.Unauthenticated
    403 -> ErrorKind.Forbidden
    404, 410 -> ErrorKind.ContentUnavailable
    408, 504 -> ErrorKind.Timeout
    429 -> ErrorKind.ServiceUnavailable
    in 500..599 -> ErrorKind.ServiceUnavailable
    else -> ErrorKind.Protocol
}

/** Catalog failure → notice. [CatalogFailure.NotFound] is content state, not an outage. */
fun CatalogFailure.toErrorNotice(operation: ErrorOperation): ErrorNotice = when (this) {
    is CatalogFailure.Access -> ErrorNotice(when (fault) {
        com.cantos.theatre.ports.CatalogAccessFault.Unauthenticated -> ErrorKind.Unauthenticated
        com.cantos.theatre.ports.CatalogAccessFault.Forbidden -> ErrorKind.Forbidden
        com.cantos.theatre.ports.CatalogAccessFault.Timeout -> ErrorKind.Timeout
    }, operation)
    CatalogFailure.NotFound -> ErrorNotice(ErrorKind.ContentUnavailable, operation, retryable = false)
    is CatalogFailure.Unavailable -> ErrorNotice(ErrorKind.ServiceUnavailable, operation, retryable)
    CatalogFailure.Protocol -> ErrorNotice(ErrorKind.Protocol, operation)
    CatalogFailure.Transport -> ErrorNotice(ErrorKind.Network, operation)
    // Repeated cursor rejection is the server's contract not holding, not a network problem.
    CatalogFailure.CursorRejected -> ErrorNotice(ErrorKind.Protocol, operation)
}

/** Playback access failure → notice. Withdrawn or mismatched release identity is not retryable. */
fun PlaybackAccessFailure.toErrorNotice(operation: ErrorOperation = ErrorOperation.StartPlayback): ErrorNotice = when (this) {
    PlaybackAccessFailure.Network -> ErrorNotice(ErrorKind.Network, operation)
    PlaybackAccessFailure.Unavailable -> ErrorNotice(ErrorKind.ServiceUnavailable, operation)
    PlaybackAccessFailure.NotFound, PlaybackAccessFailure.IdentityChanged ->
        ErrorNotice(ErrorKind.ContentUnavailable, operation, retryable = false)
    PlaybackAccessFailure.Protocol -> ErrorNotice(ErrorKind.Protocol, operation)
}
