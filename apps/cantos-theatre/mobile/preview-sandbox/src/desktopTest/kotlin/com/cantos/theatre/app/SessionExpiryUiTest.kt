package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.identity.NativeAccountState
import com.cantos.theatre.ports.IdentityV1Failure
import com.cantos.theatre.presentation.model.ShellEvent
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellTheme
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/** Interaction and semantics on the shipped composition, using deterministic external ports. */
@OptIn(ExperimentalTestApi::class)
class SessionExpiryUiTest {
    @Test fun passive_loss_banner_dismissal_does_not_navigate_or_reopen() = runDesktopComposeUiTest(width = 390, height = 900) {
        lateinit var owners: PreviewOwners
        setContent { PreviewTestRoot(PreviewSeed(previewState(), signedIn = true), onOwners = { owners = it }) }
        waitUntil { owners.account.state.value is NativeAccountState.SignedIn }
        runOnIdle {
            owners.identity.sessionFailure = IdentityV1Failure.Unauthorized
            owners.account.revalidate(requireNotNull(owners.account.verifiedSession.value).session.session)
        }
        waitForIdle()
        onNodeWithTag("session-expiry-banner").assertIsDisplayed()
        onNodeWithTag("error-dialog").assertDoesNotExist()
        onNodeWithTag("session-expiry-dismiss").performClick()
        waitForIdle()
        onNodeWithTag("session-expiry-banner").assertDoesNotExist()
        runOnIdle { repeat(10) { owners.account.revalidate("00000000-0000-4000-8000-0000000000b1") } }
        waitForIdle()
        onNodeWithTag("error-dialog").assertDoesNotExist()
        assertEquals(TheatreRoute.Discovery, owners.shell.navigation.value.route)
    }

    @Test fun contextual_dialog_is_localized_focusable_and_back_closes_before_navigation() {
        for (locale in ShellLocale.entries) for (theme in ShellTheme.entries) {
            runDesktopComposeUiTest(width = 320, height = 568) {
                lateinit var owners: PreviewOwners
                var systemBack: (() -> Unit)? = null
                setContent {
                    Box(Modifier.requiredSize(320.dp, 568.dp)) {
                        PreviewTestRoot(PreviewSeed(previewState(theme = theme, locale = locale), signedIn = true), textScale = 2f,
                            onOwners = { owners = it }, backHandler = { enabled, back -> SideEffect { systemBack = back.takeIf { enabled } } })
                    }
                }
                waitUntil { owners.account.state.value is NativeAccountState.SignedIn }
                runOnIdle {
                    owners.identity.sessionFailure = IdentityV1Failure.Unauthorized
                    owners.account.revalidate(requireNotNull(owners.account.verifiedSession.value).session.session)
                }
                waitForIdle()
                // Public-to-private intent, through the production event entrypoint.
                runOnIdle { owners.shell.dispatch(ShellEvent.Navigate(ShellRoute.Library)) }
                waitForIdle()
                val title = if (locale == ShellLocale.Vietnamese) "Phiên đăng nhập đã hết hạn" else "Your session has expired"
                val later = if (locale == ShellLocale.Vietnamese) "Để sau" else "Later"
                val signIn = if (locale == ShellLocale.Vietnamese) "Đăng nhập" else "Sign in"
                onNodeWithTag("error-dialog-title").assertTextEquals(title).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Heading))
                onNodeWithTag("error-dialog-primary").assertTextEquals(signIn).assertIsFocused().assertIsDisplayed()
                onNodeWithTag("error-dialog-close").assertTextEquals(later).assertIsDisplayed()
                assertTrue(onNodeWithTag("error-dialog").fetchSemanticsNode().boundsInRoot.height <= 568f)
                onNodeWithTag("error-dialog-body").assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.VerticalScrollAxisRange))
                onNodeWithTag("session-expiry-banner").assertDoesNotExist()
                onNodeWithTag("auth-status").assertDoesNotExist()
                for (tag in listOf("error-dialog-primary", "error-dialog-close")) {
                    assertTrue(onNodeWithTag(tag).fetchSemanticsNode().boundsInRoot.height >= 48f)
                }
                runOnIdle { assertNotNull(systemBack).invoke() }
                waitForIdle()
                onNodeWithTag("error-dialog").assertDoesNotExist()
                assertEquals(TheatreRoute.Library, owners.shell.navigation.value.route)
                // Dismiss is scoped to the loss epoch, including another private request.
                runOnIdle { owners.shell.dispatch(ShellEvent.Navigate(ShellRoute.Library)) }
                waitForIdle()
                onNodeWithTag("error-dialog").assertDoesNotExist()
            }
        }
    }

    @Test fun signin_from_a_private_prompt_keeps_the_typed_return_destination() =
        runDesktopComposeUiTest(width = 390, height = 900) {
            lateinit var owners: PreviewOwners
            setContent { PreviewTestRoot(PreviewSeed(previewState(), signedIn = true), onOwners = { owners = it }) }
            waitUntil { owners.account.state.value is NativeAccountState.SignedIn }
            runOnIdle {
                owners.identity.sessionFailure = IdentityV1Failure.Unauthorized
                owners.account.revalidate(requireNotNull(owners.account.verifiedSession.value).session.session)
            }
            waitForIdle()
            runOnIdle { owners.shell.dispatch(ShellEvent.Navigate(ShellRoute.Library)) }
            waitForIdle()
            onNodeWithTag("error-dialog-primary").performClick()
            waitForIdle()
            assertEquals(listOf(TheatreRoute.Library, TheatreRoute.Login), owners.shell.navigation.value.stack)
            onNodeWithTag("error-dialog").assertDoesNotExist()
            onNodeWithTag("auth-status").assertDoesNotExist()
            onNodeWithTag("back").performClick()
            waitForIdle()
            assertEquals(TheatreRoute.Library, owners.shell.navigation.value.route)
        }
}
