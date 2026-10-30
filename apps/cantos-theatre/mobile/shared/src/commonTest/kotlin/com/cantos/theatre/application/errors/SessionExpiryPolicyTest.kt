package com.cantos.theatre.application.errors

import com.cantos.theatre.application.identity.ConfirmedSessionLoss
import com.cantos.theatre.application.identity.SessionLossReason
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class SessionExpiryPolicyTest {
    private val loss = ConfirmedSessionLoss(7, SessionLossReason.SessionRejected)
    private fun step(state: ErrorCenterState, event: ErrorEvent) = reduceErrors(state, event).state

    @Test fun passive_loss_is_a_banner_and_only_private_intent_opens_the_existing_window() {
        val observed = step(ErrorCenterState(), ErrorEvent.SessionObserved(loss))
        assertEquals(SessionExpiryPresentation.Banner, observed.sessionBanner?.presentation)
        assertNull(observed.active)
        val private = step(observed, ErrorEvent.PrivateAccessRequested)
        assertNull(private.sessionBanner)
        assertEquals(ErrorKey(ErrorKind.SessionExpired, ErrorOperation.SignIn, 7), private.active?.key)
        assertEquals(listOf(ErrorAction.SignIn, ErrorAction.Close), private.active?.actions)
        assertEquals(0, private.queuedCount)
        assertEquals(private, step(private, ErrorEvent.PrivateAccessRequested))
    }

    @Test fun dismissal_and_repeated_observations_cannot_reopen_one_epoch_but_a_new_loss_can() {
        var state = step(ErrorCenterState(), ErrorEvent.SessionObserved(loss))
        state = step(state, ErrorEvent.PrivateAccessRequested)
        state = step(state, ErrorEvent.Dismissed)
        repeat(20) {
            state = step(state, ErrorEvent.SessionObserved(loss))
            state = step(state, ErrorEvent.PrivateAccessRequested)
            assertNull(state.active)
            assertNull(state.sessionBanner)
        }
        state = step(state, ErrorEvent.SessionObserved(loss.copy(epoch = 9)))
        assertEquals(9L, state.sessionBanner?.loss?.epoch)
    }

    @Test fun new_account_or_cleanup_clears_queued_and_visible_old_loss_without_dropping_other_errors() {
        val network = ErrorNotice(ErrorKind.Network, ErrorOperation.StartPlayback)
        var state = step(ErrorCenterState(), ErrorEvent.Reported(network, false))
        state = step(state, ErrorEvent.SessionObserved(loss))
        state = step(state, ErrorEvent.PrivateAccessRequested)
        assertEquals(1, state.queuedCount)
        assertNull(state.sessionBanner)
        state = step(state, ErrorEvent.SessionObserved(null))
        assertEquals(network, state.active)
        assertNull(state.sessionExpiry)
        assertEquals(0, state.queuedCount)
    }

    @Test fun a_full_error_queue_does_not_consume_an_unshown_session_prompt() {
        var state = step(ErrorCenterState(), ErrorEvent.SessionObserved(loss))
        for (operation in listOf(ErrorOperation.LoadCatalog, ErrorOperation.LoadWork, ErrorOperation.LoadUnits, ErrorOperation.StartPlayback)) {
            state = step(state, ErrorEvent.Reported(ErrorNotice(ErrorKind.Network, operation), false))
        }
        val full = state
        state = step(state, ErrorEvent.PrivateAccessRequested)
        assertEquals(full, state)
        repeat(4) { state = step(state, ErrorEvent.Dismissed) }
        assertEquals(loss, state.sessionBanner?.loss)
        state = step(state, ErrorEvent.PrivateAccessRequested)
        assertEquals(ErrorKind.SessionExpired, state.active?.kind)
    }

    @Test fun both_signin_paths_emit_only_navigation_and_never_retry_a_mutation() {
        val banner = step(ErrorCenterState(), ErrorEvent.SessionObserved(loss))
        val bannerAction = reduceErrors(banner, ErrorEvent.SessionSignInChosen)
        assertEquals(ErrorEffect.OpenSignIn, bannerAction.effect)
        assertNull(bannerAction.state.sessionBanner)
        val modalAction = reduceErrors(step(banner, ErrorEvent.PrivateAccessRequested), ErrorEvent.ActionChosen(ErrorAction.SignIn))
        assertEquals(ErrorEffect.OpenSignIn, modalAction.effect)
        assertNull(modalAction.state.active)
        assertNull(modalAction.state.sessionBanner)
        assertNull(reduceErrors(modalAction.state, ErrorEvent.ActionChosen(ErrorAction.Retry)).effect)
    }
}
