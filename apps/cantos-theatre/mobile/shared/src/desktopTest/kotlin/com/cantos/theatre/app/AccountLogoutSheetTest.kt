package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.application.identity.AccountView
import com.cantos.theatre.features.auth.presentation.AccountContent
import com.cantos.theatre.features.auth.presentation.AuthIntent
import com.cantos.theatre.features.shell.presentation.BackOverlay
import com.cantos.theatre.features.shell.presentation.BackOverlayRegistry
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.ShellTheme
import com.cantos.ui.CantosTheme
import com.cantos.ui.Theme
import com.cantos.ui.UiPreferences
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.Test

/**
 * The signed-in Account and its modal sign-out sheet over a recording intent sink. A fixed
 * [AccountView] stands in for the owner, so these JVM interactions prove the presentation contract
 * (which intents leave the screen, and when); the owner's cleanup, retry and stale-result behavior
 * is exercised through the real owner in the preview-sandbox journeys, and nothing here is native
 * secure storage, a screen reader, system Back or an Android/iOS run.
 */
@OptIn(ExperimentalTestApi::class)
class AccountLogoutSheetTest {
    private class Host(val locale: ShellLocale = ShellLocale.Vietnamese, val account: AccountView = AccountView.SignedIn) {
        val intents = mutableListOf<AuthIntent>()
        val overlays = BackOverlayRegistry()
    }

    @Composable
    private fun Mounted(host: Host, textScale: Float) {
        CantosTheme(UiPreferences(theme = Theme.Light, textScale = textScale)) {
            Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                AccountContent(host.locale, ShellTheme.Light, host.account, { host.intents += it }, FocusRequester()) { onBack -> BackOverlay(host.overlays, onBack) }
            }
        }
    }

    private fun run(
        host: Host = Host(),
        width: Int = 390,
        height: Int = 844,
        textScale: Float = 1f,
        body: ComposeUiTest.(Host) -> Unit,
    ) = runDesktopComposeUiTest(width = width, height = height) {
        setContent { Box(Modifier.requiredSize(width.dp, height.dp)) { Mounted(host, textScale) } }
        body(host)
    }

    private fun ComposeUiTest.openSheet() {
        onNodeWithTag("logout").performScrollTo().performClick()
        waitUntil(timeoutMillis = 5_000) { onAllNodesWithTagCount("logout-sheet") > 0 }
    }

    private fun ComposeUiTest.onAllNodesWithTagCount(tag: String) =
        onAllNodes(hasTestTag(tag)).fetchSemanticsNodes().size

    private fun ComposeUiTest.sheetGone() = waitUntil(timeoutMillis = 5_000) { onAllNodesWithTagCount("logout-sheet") == 0 }

    @Test
    fun opening_and_cancelling_the_sheet_sends_nothing_and_returns_focus_to_the_entry() = run { host ->
        onNodeWithTag("logout").performScrollTo().assertIsDisplayed()
        assertTrue(host.intents.isEmpty(), "rendering the entry sends nothing")

        openSheet()
        assertTrue(host.intents.isEmpty(), "opening the sheet sends nothing")
        onNodeWithTag("logout-sheet-title").assertTextEquals("Đăng xuất khỏi Cantos?")
        onNodeWithTag("logout-sheet-body").assertTextEquals("Bạn luôn có thể quay lại Cantos để tiếp tục câu chuyện của mình.")
        onNodeWithTag("logout-cancel").assertIsDisplayed().assertIsEnabled()
        onNodeWithTag("logout-confirm").assertIsDisplayed().assertIsEnabled()

        onNodeWithTag("logout-cancel").performClick()
        sheetGone()
        waitForIdle()
        assertTrue(host.intents.isEmpty(), "Go back sends nothing")
        onNodeWithTag("logout").assertIsFocused()
    }

    @Test
    fun system_back_closes_the_sheet_before_the_route_and_sends_nothing() = run { host ->
        assertFalse(host.overlays.active)
        assertFalse(host.overlays.dismiss(), "with no sheet, Back belongs to the route")

        openSheet()
        assertTrue(host.overlays.active, "an open sheet owns Back")
        assertTrue(host.overlays.dismiss())
        sheetGone()
        waitForIdle()
        assertFalse(host.overlays.active, "a closed sheet releases Back to the route")
        assertTrue(host.intents.isEmpty())
        onNodeWithTag("logout").assertIsFocused()
    }

    @Test
    fun confirm_sends_one_sign_out_and_a_pending_sheet_cannot_send_a_second() = run { host ->
        openSheet()
        onNodeWithTag("logout-confirm").performClick()
        waitForIdle()
        assertEquals(listOf<AuthIntent>(AuthIntent.Logout), host.intents)

        // The owner has not changed the fixed view: the sheet reports progress and refuses another request.
        onNodeWithTag("logout-confirm").assertIsNotEnabled().assertTextContains("Đang đăng xuất…")
        onNodeWithTag("logout-confirm").performClick()
        waitForIdle()
        assertEquals(1, host.intents.size, "pending and recomposition never replay the request")
    }

    @Test
    fun dismissing_after_dispatch_does_not_pretend_to_cancel_it() = run { host ->
        openSheet()
        onNodeWithTag("logout-confirm").performClick()
        waitForIdle()
        assertTrue(host.overlays.dismiss())
        sheetGone()
        waitForIdle()
        assertEquals(1, host.intents.size, "closing is not a second request and not an undo")
        // The signed-in view is still what the owner says, so the entry is still there for a new decision.
        onNodeWithTag("logout").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun english_copy_is_complete_and_the_actions_sit_side_by_side_at_phone_width() = run(Host(ShellLocale.English)) { _ ->
        openSheet()
        onNodeWithTag("logout-sheet-title").assertTextEquals("Sign out of Cantos?")
        onNodeWithTag("logout-sheet-body").assertTextEquals("You can always return to Cantos to continue your story.")
        onNodeWithTag("logout-cancel").assertTextContains("Go back")
        onNodeWithTag("logout-confirm").assertTextContains("Sign out")
        val cancel = onNodeWithTag("logout-cancel").fetchSemanticsNode().boundsInRoot
        val confirm = onNodeWithTag("logout-confirm").fetchSemanticsNode().boundsInRoot
        assertTrue(cancel.right <= confirm.left, "two actions share a row at normal text size")
        assertEquals(cancel.top, confirm.top)
        assertTrue(cancel.height >= 56f * density.density - 1f && confirm.height >= 56f * density.density - 1f)
    }

    @Test
    fun large_text_on_a_narrow_screen_stacks_full_width_actions_inside_the_viewport() =
        run(width = 320, height = 568, textScale = 2f) { _ ->
            openSheet()
            onNodeWithTag("logout-cancel").assertIsDisplayed()
            onNodeWithTag("logout-confirm").assertIsDisplayed()
            val cancel = onNodeWithTag("logout-cancel").fetchSemanticsNode().boundsInRoot
            val confirm = onNodeWithTag("logout-confirm").fetchSemanticsNode().boundsInRoot
            assertTrue(cancel.bottom <= confirm.top, "actions stack instead of clipping labels")
            assertEquals(cancel.width, confirm.width)
            assertTrue(confirm.right <= 320f * density.density && cancel.left >= 0f)
        }

    @Test
    fun without_a_profile_projection_the_account_is_the_honest_fallback() = run { host ->
        onNodeWithTag("account-name").assertTextEquals("Người nghe Cantos")
        onNodeWithTag("account-profile").assertIsDisplayed()
        onNodeWithText("Đã đăng nhập").assertIsDisplayed()
        // No email, no member name from the design sample, no invented plan or contact address.
        listOf("Minh Anh", "minhanh@example.com", "@").forEach {
            assertEquals(0, onAllNodes(hasText(it, substring = true)).fetchSemanticsNodes().size, it)
        }
        onNodeWithTag("account-nav-group").performScrollTo().assertIsDisplayed()
        // Every navigation row meets the minimum hit target; support is information, not a control.
        listOf("membership-preview-entry", "open-library", "theme-toggle", "settings", "logout").forEach { tag ->
            val node = onNodeWithTag(tag).performScrollTo().fetchSemanticsNode()
            assertTrue(node.boundsInRoot.height >= 48f * density.density - 1f, tag)
            assertNotNull(node.config.getOrNull(SemanticsActions.OnClick), tag)
        }
        val support = onNodeWithTag("account-support-info").performScrollTo().fetchSemanticsNode()
        assertNull(support.config.getOrNull(SemanticsActions.OnClick))
        assertNull(support.config.getOrNull(SemanticsProperties.Role))
        onNodeWithTag("account-support-info").assertTextContains("Thông tin hỗ trợ đang cập nhật")
        assertTrue(host.intents.isEmpty())
    }

    @Test
    fun rows_send_their_own_intents_and_never_a_sign_out() = run { host ->
        listOf(
            "membership-preview-entry" to AuthIntent.OpenMembership,
            "open-library" to AuthIntent.OpenLibrary,
            "settings" to AuthIntent.OpenSettings,
            "theme-toggle" to AuthIntent.ToggleTheme,
        ).forEach { (tag, intent) ->
            onNodeWithTag(tag).performScrollTo().performClick()
            assertEquals(intent, host.intents.last(), tag)
        }
        assertFalse(host.intents.contains(AuthIntent.Logout))
    }

    @Test
    fun a_guest_has_no_sign_out_entry() = run(Host(account = AccountView.SignedOut)) { host ->
        onNodeWithTag("sign-in").performScrollTo().assertIsDisplayed()
        assertEquals(0, onAllNodesWithTagCount("logout"))
        assertEquals(0, onAllNodesWithTagCount("logout-sheet"))
        assertTrue(host.intents.isEmpty())
    }
}
