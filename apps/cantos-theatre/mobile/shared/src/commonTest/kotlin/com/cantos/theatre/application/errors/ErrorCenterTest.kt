package com.cantos.theatre.application.errors

import com.cantos.theatre.adapters.http.OriginSource
import com.cantos.theatre.adapters.http.selectPackagedApiOrigin
import com.cantos.theatre.app.HostOrigin
import com.cantos.theatre.app.hostOrigin
import com.cantos.theatre.app.packagedApiOriginText
import com.cantos.theatre.application.catalog.CatalogFailure
import com.cantos.theatre.ports.PlaybackAccessFailure
import com.cantos.theatre.presentation.model.HostConfigurationFault
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNull

class ErrorCenterTest {
    @Test
    fun http_statuses_and_adapter_failures_map_to_the_closed_error_vocabulary() {
        assertEquals(ErrorKind.Unauthenticated, errorKindForHttpStatus(401))
        assertEquals(ErrorKind.Forbidden, errorKindForHttpStatus(403))
        assertEquals(ErrorKind.ContentUnavailable, errorKindForHttpStatus(404))
        assertEquals(ErrorKind.ContentUnavailable, errorKindForHttpStatus(410))
        assertEquals(ErrorKind.Timeout, errorKindForHttpStatus(408))
        assertEquals(ErrorKind.Timeout, errorKindForHttpStatus(504))
        assertEquals(ErrorKind.ServiceUnavailable, errorKindForHttpStatus(429))
        assertEquals(ErrorKind.ServiceUnavailable, errorKindForHttpStatus(500))
        assertEquals(ErrorKind.ServiceUnavailable, errorKindForHttpStatus(503))
        assertEquals(ErrorKind.Protocol, errorKindForHttpStatus(400))
        assertEquals(ErrorKind.Protocol, errorKindForHttpStatus(422))

        val catalogNotFound = CatalogFailure.NotFound.toErrorNotice(ErrorOperation.LoadWork)
        assertEquals(ErrorKind.ContentUnavailable, catalogNotFound.kind)
        assertEquals(ErrorScope.Section, catalogNotFound.scope)
        assertEquals(ErrorMessageKey.ContentUnavailable, catalogNotFound.messageKey)
        assertEquals(listOf(ErrorAction.Close), catalogNotFound.actions)
        assertEquals(ErrorPresentation.Inline, catalogNotFound.presentation(hasContent = false))

        val catalogUnavailableNonRetryable = CatalogFailure.Unavailable(retryable = false).toErrorNotice(ErrorOperation.LoadCatalog)
        assertEquals(listOf(ErrorAction.Close), catalogUnavailableNonRetryable.actions)

        val playbackNetwork = PlaybackAccessFailure.Network.toErrorNotice()
        assertEquals(ErrorKind.Network, playbackNetwork.kind)
        assertEquals(ErrorOperation.StartPlayback, playbackNetwork.operation)
        assertEquals(ErrorScope.Action, playbackNetwork.scope)
        assertEquals(listOf(ErrorAction.Retry, ErrorAction.Close), playbackNetwork.actions)
        assertEquals(ErrorPresentation.Modal, playbackNetwork.presentation(hasContent = false))
        assertEquals(ErrorPresentation.Inline, playbackNetwork.presentation(hasContent = true))

        val playbackChanged = PlaybackAccessFailure.IdentityChanged.toErrorNotice()
        assertEquals(ErrorKind.ContentUnavailable, playbackChanged.kind)
        assertEquals(listOf(ErrorAction.Close), playbackChanged.actions)

        val unauth = ErrorNotice(ErrorKind.Unauthenticated, ErrorOperation.UpdateLibrary)
        assertEquals(listOf(ErrorAction.SignIn, ErrorAction.Close), unauth.actions)
        assertEquals(ErrorPresentation.Modal, unauth.presentation(hasContent = true))

        val config = ErrorNotice(ErrorKind.Configuration, ErrorOperation.LoadCatalog, retryable = false)
        assertEquals(ErrorScope.Host, config.scope)
        assertEquals(listOf(ErrorAction.Close), config.actions)
        assertEquals(ErrorPresentation.Modal, config.presentation(hasContent = true))
    }

    @Test
    fun concurrent_reports_dedupe_by_key_and_bound_the_waiting_queue() {
        val networkCatalog = ErrorNotice(ErrorKind.Network, ErrorOperation.LoadCatalog)
        val networkPlayback = ErrorNotice(ErrorKind.Network, ErrorOperation.StartPlayback)
        val unauthLibrary = ErrorNotice(ErrorKind.Unauthenticated, ErrorOperation.UpdateLibrary)
        val timeoutWork = ErrorNotice(ErrorKind.Timeout, ErrorOperation.LoadWork)
        val storageLibrary = ErrorNotice(ErrorKind.Storage, ErrorOperation.UpdateLibrary)

        var state = ErrorCenterState()
        state = reduceErrors(state, ErrorEvent.Reported(networkCatalog, hasContent = false)).state
        assertEquals(networkCatalog, state.active)
        assertEquals(0, state.queuedCount)

        // Repeated report of the same (kind, operation) is deduplicated.
        state = reduceErrors(state, ErrorEvent.Reported(networkCatalog, hasContent = false)).state
        assertEquals(networkCatalog, state.active)
        assertEquals(0, state.queuedCount)

        // Inline presentation when content is already on screen never opens or queues a modal.
        state = reduceErrors(state, ErrorEvent.Reported(ErrorNotice(ErrorKind.ServiceUnavailable, ErrorOperation.LoadUnits), hasContent = true)).state
        assertEquals(0, state.queuedCount)

        // Up to MaxQueuedErrors distinct modal notices wait in FIFO order; excess storm notices are dropped.
        state = reduceErrors(state, ErrorEvent.Reported(networkPlayback, hasContent = false)).state
        state = reduceErrors(state, ErrorEvent.Reported(unauthLibrary, hasContent = true)).state
        state = reduceErrors(state, ErrorEvent.Reported(timeoutWork, hasContent = false)).state
        state = reduceErrors(state, ErrorEvent.Reported(storageLibrary, hasContent = false)).state
        assertEquals(MaxQueuedErrors, state.queuedCount)

        // Duplicate of an already queued notice is ignored.
        state = reduceErrors(state, ErrorEvent.Reported(unauthLibrary, hasContent = true)).state
        assertEquals(MaxQueuedErrors, state.queuedCount)
    }

    @Test
    fun dismissal_suppresses_reopening_until_resolved_while_explicit_retry_allows_a_fresh_failure() {
        val notice = ErrorNotice(ErrorKind.Network, ErrorOperation.StartPlayback)
        val next = ErrorNotice(ErrorKind.Unauthenticated, ErrorOperation.UpdateLibrary)

        var state = reduceErrors(ErrorCenterState(), ErrorEvent.Reported(notice, hasContent = false)).state
        state = reduceErrors(state, ErrorEvent.Reported(next, hasContent = false)).state

        // Dismissing promotes the next queued notice and suppresses repeated reports of the dismissed key.
        state = reduceErrors(state, ErrorEvent.Dismissed).state
        assertEquals(next, state.active)
        state = reduceErrors(state, ErrorEvent.Reported(notice, hasContent = false)).state
        assertEquals(0, state.queuedCount, "dismissed notice does not re-queue on polling/recomposition")

        // Choosing SignIn on the active unauthenticated notice emits OpenSignIn and closes the window.
        val signInTransition = reduceErrors(state, ErrorEvent.ActionChosen(ErrorAction.SignIn))
        assertEquals(ErrorEffect.OpenSignIn, signInTransition.effect)
        assertNull(signInTransition.state.active)

        // Once the operation resolves, suppression is cleared so a future failure can open again.
        state = reduceErrors(signInTransition.state, ErrorEvent.Resolved(notice.key)).state
        state = reduceErrors(state, ErrorEvent.Reported(notice, hasContent = false)).state
        assertEquals(notice, state.active)

        // Choosing an action not offered by the active notice is ignored.
        val ignored = reduceErrors(state, ErrorEvent.ActionChosen(ErrorAction.SignIn))
        assertNull(ignored.effect)
        assertEquals(notice, ignored.state.active)

        // Choosing Retry closes the window without suppressing the key, so if the retry fails it reopens.
        val retryTransition = reduceErrors(state, ErrorEvent.ActionChosen(ErrorAction.Retry))
        assertEquals(ErrorEffect.Retry(ErrorOperation.StartPlayback), retryTransition.effect)
        assertNull(retryTransition.state.active)
        state = reduceErrors(retryTransition.state, ErrorEvent.Reported(notice, hasContent = false)).state
        assertEquals(notice, state.active)
    }

    @Test
    fun host_origin_uses_production_default_when_unset_and_refuses_invalid_overrides_with_typed_faults() {
        val defaultReady = assertIs<HostOrigin.Ready>(hostOrigin(selectPackagedApiOrigin(null, allowLoopbackHttp = false), networkDeclared = true))
        assertEquals("https://api.cantos.cc", defaultReady.origin.toString())
        assertEquals(OriginSource.Default, defaultReady.source)
        assertEquals("https://api.cantos.cc", packagedApiOriginText(null))
        assertEquals(null, packagedApiOriginText(""))
        assertEquals("https://api.cantos.cc", packagedApiOriginText(com.cantos.theatre.adapters.http.PackagedDefaultApiOrigin))

        assertEquals(HostOrigin.Refused(HostConfigurationFault.OriginEmpty),
            hostOrigin(selectPackagedApiOrigin("", allowLoopbackHttp = false)))


        assertEquals(
            HostOrigin.Refused(HostConfigurationFault.NetworkNotDeclared),
            hostOrigin(selectPackagedApiOrigin(null, allowLoopbackHttp = false), networkDeclared = false),
        )
        assertEquals(
            HostOrigin.Refused(HostConfigurationFault.OriginEmpty),
            hostOrigin(selectPackagedApiOrigin("   ", allowLoopbackHttp = false), networkDeclared = true),
        )
        assertEquals(
            HostOrigin.Refused(HostConfigurationFault.OriginMalformed),
            hostOrigin(selectPackagedApiOrigin("https://api.cantos.cc/extra", allowLoopbackHttp = false), networkDeclared = true),
        )
        assertEquals(
            HostOrigin.Refused(HostConfigurationFault.OriginNotHttps),
            hostOrigin(selectPackagedApiOrigin("ftp://api.cantos.cc", allowLoopbackHttp = false), networkDeclared = true),
        )
        assertEquals(
            HostOrigin.Refused(HostConfigurationFault.OriginNotHttps),
            hostOrigin(selectPackagedApiOrigin("http://127.0.0.1:8080", allowLoopbackHttp = false), networkDeclared = true),
        )
        assertEquals(
            HostOrigin.Refused(HostConfigurationFault.OriginCleartextHostNotAllowed),
            hostOrigin(selectPackagedApiOrigin("http://192.168.1.20:8080", allowLoopbackHttp = true), networkDeclared = true),
        )
    }
}
