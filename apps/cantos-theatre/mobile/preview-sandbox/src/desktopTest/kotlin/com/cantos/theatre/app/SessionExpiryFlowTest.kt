package com.cantos.theatre.app

import com.cantos.theatre.adapters.preview.PreviewLoginMode
import com.cantos.theatre.adapters.preview.previewLibraryStore
import com.cantos.theatre.application.errors.ErrorAction
import com.cantos.theatre.application.errors.ErrorEvent
import com.cantos.theatre.application.errors.ErrorKind
import com.cantos.theatre.application.identity.NativeAccountFailure
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.domain.identity.RegistrationEmail
import com.cantos.theatre.domain.identity.RegistrationResult
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellWorkId
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class SessionExpiryFlowTest {
    @Test fun private_save_after_confirmed_loss_prompts_once_and_signin_never_replays_the_mutation() = runTest {
        val h = PreviewHarness(backgroundScope, PreviewSeed(previewState(), signedIn = true))
        try {
            runCurrent()
            h.identity.sessionFailure = IdentityV1Failure.Unauthorized
            h.account.revalidate(assertNotNull(h.account.verifiedSession.value).session.session)
            runCurrent()
            assertNotNull(h.shell.errors.value.sessionBanner)
            assertNull(h.shell.errors.value.active)
            val work = ShellWorkId(h.owners.catalog.state.value.works.items.first().id.value)
            h.shell.dispatch(ShellEvent.SetSaved(work, true)); runCurrent()
            assertEquals(ErrorKind.SessionExpired, h.shell.errors.value.active?.kind)
            repeat(10) { h.shell.dispatch(ShellEvent.SetSaved(work, true)) }
            assertEquals(0, h.shell.errors.value.queuedCount)
            h.shell.onErrorEvent(ErrorEvent.ActionChosen(ErrorAction.SignIn)); runCurrent()
            assertEquals(listOf(TheatreRoute.Discovery, TheatreRoute.Login), h.stack)
            h.identity.sessionFailure = null
            h.identity.mode = PreviewLoginMode.Hold
            val email = assertIs<RegistrationResult.Accepted<RegistrationEmail>>(RegistrationEmail.parse("listener@example.test")).value
            assertTrue(h.shell.login(email, "synthetic-password")); runCurrent()
            h.identity.completeLogin(true); runCurrent()
            assertEquals(TheatreRoute.Discovery, h.route)
            assertIs<NativeAccountState.SignedIn>(h.account.state.value)
            assertNull(h.shell.errors.value.active)
            assertNull(h.shell.errors.value.sessionBanner)
            assertTrue(h.shell.library.value.saved.isEmpty())
            assertEquals(0, h.owners.libraryApi.mutations)
        } finally { h.close() }
    }

    @Test fun expired_authority_discovered_by_the_private_intent_never_enqueues_save() = runTest {
        var now = PreviewOwners.FixedNowMs
        val h = PreviewOwners(backgroundScope, PreviewSeed(previewState(), signedIn = true), previewLibraryStore(), nowMs = { now })
        try {
            runCurrent()
            val work = ShellWorkId(h.catalog.state.value.works.items.first().id.value)
            now += 901_000
            h.shell.dispatch(ShellEvent.SetSaved(work, true)); runCurrent()
            assertEquals(ErrorKind.SessionExpired, h.shell.errors.value.active?.kind)
            assertEquals(TheatreRoute.Discovery, h.shell.navigation.value.route)
            assertEquals(0, h.libraryApi.mutations)
            assertNull(h.shell.library.value.account)
        } finally { h.close() }
    }

    @Test fun unknown_or_unavailable_private_refresh_has_no_expiry_label_no_retry_and_no_mutation() = runTest {
        for (failure in listOf(IdentityV1Failure.UnknownOutcome, IdentityV1Failure.Transport,
            IdentityV1Failure.ServiceUnavailable, IdentityV1Failure.SessionChanged, IdentityV1Failure.RateLimited(8))) {
            var now = PreviewOwners.FixedNowMs
            val h = PreviewOwners(backgroundScope, PreviewSeed(previewState(), signedIn = true), previewLibraryStore(), nowMs = { now })
            try {
                runCurrent()
                h.identity.refreshFailure = failure
                now += 110_000
                h.shell.dispatch(ShellEvent.SetSaved(ShellWorkId(h.catalog.state.value.works.items.first().id.value), true))
                runCurrent()
                val state = assertIs<NativeAccountState.Failed>(h.account.state.value)
                assertEquals(NativeAccountFailure.Api, state.reason)
                assertNull(state.sessionLoss)
                assertNull(h.shell.errors.value.sessionExpiry)
                assertTrue(h.shell.errors.value.active?.kind != ErrorKind.SessionExpired)
                assertTrue(h.shell.errors.value.active?.actions?.contains(ErrorAction.Retry) != true)
                assertEquals(0, h.libraryApi.mutations)
            } finally { h.close() }
        }
    }
}
