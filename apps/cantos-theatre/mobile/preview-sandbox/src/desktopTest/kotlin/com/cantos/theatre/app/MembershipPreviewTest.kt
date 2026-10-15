package com.cantos.theatre.app

import com.cantos.theatre.demo.demoState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import com.cantos.theatre.presentation.model.ShellRoute
import com.cantos.theatre.presentation.model.ShellState
import com.cantos.theatre.domain.membership.MembershipState
import org.junit.Test
import kotlin.test.assertTrue

/** JVM renderer drives the real mock composition. No native or identity acceptance claim. */
@OptIn(ExperimentalTestApi::class)
class MembershipPreviewTest {
    @Test
    fun membership_theme_action_keeps_the_same_draft_and_form_destination() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { MembershipTestRoot() }
            onNodeWithTag("membership-start").performScrollTo().performClick()
            onNodeWithTag("membership-name").performScrollTo().performTextReplacement("Nguyễn Thị Ánh · Ỗ, Ữ")
            onNodeWithTag("membership-email").performScrollTo().performTextReplacement("anh@example.com")
            repeat(2) {
                onNodeWithTag("membership-theme").performClick()
                onNodeWithTag("membership-heading").assert(hasText("Thêm một người đồng hành."))
                onNodeWithTag("membership-name").assert(SemanticsMatcher.expectValue(
                    SemanticsProperties.EditableText, AnnotatedString("Nguyễn Thị Ánh · Ỗ, Ữ")))
                onNodeWithTag("membership-email").assert(SemanticsMatcher.expectValue(
                    SemanticsProperties.EditableText, AnnotatedString("anh@example.com")))
                onNodeWithTag("membership-receipt").assertDoesNotExist()
            }
        }
    }

    @Test
    fun account_entry_opens_a_blank_single_form_with_no_plan_or_consent_selection() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { MembershipTestRoot(initial = demoState(stack = listOf(ShellRoute.Discovery, ShellRoute.Account))) }
            onNodeWithTag("membership-preview-entry").performScrollTo().performClick()
            onNodeWithTag("membership-start").performScrollTo().performClick()
            onNodeWithTag("membership-name").assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("")))
            onNodeWithTag("membership-email").assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("")))
            onAllNodes(hasSetTextAction()).assertCountEquals(2)
            onAllNodes(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Checkbox)).assertCountEquals(0)
            onNodeWithTag("membership-plans").assertDoesNotExist()
            onNodeWithTag("membership-fixture-controls").assertDoesNotExist()
            onNodeWithTag("mini-open").assertDoesNotExist()
        }
    }

    @Test
    fun invalid_form_focuses_first_finding_and_default_valid_submit_never_claims_a_receipt() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { MembershipTestRoot() }
            onNodeWithTag("membership-start").performScrollTo().performClick()
            onNodeWithTag("membership-submit").performScrollTo().performClick()
            onNodeWithTag("membership-name").assertIsFocused()
            onNodeWithText("Nhập tên bạn muốn sử dụng.").assertExists()
            onNodeWithText("Nhập địa chỉ email.").assertExists()
            onNodeWithTag("membership-name").performScrollTo().performTextReplacement("Nguyễn Thị Ánh")
            onNodeWithTag("membership-email").performScrollTo().performTextReplacement("anh@example.com")
            onNodeWithTag("membership-submit").performScrollTo().performClick()
            onNodeWithTag("membership-submission-status").assert(hasText("Đăng ký thật chưa khả dụng. Nội dung đã nhập chỉ được giữ trong phiên xem thử này."))
            onNodeWithTag("membership-receipt").assertDoesNotExist()
            onNodeWithTag("back").performClick()
            onNodeWithTag("membership-start").performScrollTo().performClick()
            onNodeWithTag("membership-email").assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("anh@example.com")))
            val email = onNodeWithTag("membership-email").performScrollTo()
            email.performSemanticsAction(SemanticsActions.RequestFocus) { assertTrue(it()) }
            email.assertIsFocused().performKeyInput { pressKey(Key.Escape) }
            onNodeWithTag("membership-start").assertExists()
        }
    }

    @Test
    fun repeated_invalid_submit_refocuses_the_first_field_even_when_findings_are_unchanged() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { MembershipTestRoot() }
            onNodeWithTag("membership-start").performScrollTo().performClick()
            onNodeWithTag("membership-submit").performScrollTo().performClick()
            onNodeWithTag("membership-name").assertIsFocused()
            val email = onNodeWithTag("membership-email").performScrollTo()
            email.performSemanticsAction(SemanticsActions.RequestFocus) { assertTrue(it()) }
            email.assertIsFocused()
            onNodeWithTag("membership-submit").performScrollTo().performClick()
            onNodeWithTag("membership-name").assertIsFocused()
        }
    }

    @Test
    fun ime_done_refocuses_first_invalid_field_on_each_unchanged_submit() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { MembershipTestRoot() }
            onNodeWithTag("membership-start").performScrollTo().performClick()
            val email = onNodeWithTag("membership-email").performScrollTo()
            email.performSemanticsAction(SemanticsActions.RequestFocus) { assertTrue(it()) }
            email.assertIsFocused().performImeAction()
            onNodeWithTag("membership-name").assertIsFocused()
            onNodeWithText("Nhập tên bạn muốn sử dụng.").assertExists()
            onNodeWithText("Nhập địa chỉ email.").assertExists()
            email.performScrollTo().performSemanticsAction(SemanticsActions.RequestFocus) { assertTrue(it()) }
            email.assertIsFocused().performImeAction()
            onNodeWithTag("membership-name").assertIsFocused()
            onNodeWithTag("membership-receipt").assertDoesNotExist()
        }
    }

    @Test
    fun fixture_requires_manual_receipt_after_unknown_retry_and_plan_failure_keeps_it() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { MembershipTestRoot(fixture = MembershipState.fixture()) }
            onNodeWithTag("membership-start").performScrollTo().performClick()
            onNodeWithTag("membership-name").performScrollTo().performTextReplacement("Nguyễn Thị Ánh")
            onNodeWithTag("membership-email").performScrollTo().performTextReplacement("anh@example.com")
            onNodeWithTag("membership-submit").performScrollTo().performClick().assertIsNotEnabled()
            onNodeWithTag("membership-receipt").assertDoesNotExist()
            onNodeWithTag("membership-fixture-Unknown").performScrollTo().performClick()
            onNodeWithTag("membership-submit").performScrollTo().assert(hasText("Kiểm tra lại yêu cầu")).performClick()
            onNodeWithTag("membership-fixture-Received").performScrollTo().performClick()
            onNodeWithTag("membership-receipt").assertExists()
            onNodeWithTag("membership-plans").performScrollTo().performClick()
            onNodeWithTag("membership-plan-Failed").performScrollTo().performClick()
            onNodeWithTag("back").performClick()
            onNodeWithTag("membership-receipt").assertExists()
            onNodeWithTag("membership-clear-session").performScrollTo().performClick()
            onNodeWithTag("membership-start").performScrollTo().performClick()
            onNodeWithTag("membership-email").assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("")))
            onNodeWithTag("membership-receipt").assertDoesNotExist()
        }
    }

    @Test
    fun account_complete_fixture_opens_plans_by_choice_and_back_returns_to_same_source() {
        runDesktopComposeUiTest(width = 390, height = 844) {
            setContent { MembershipTestRoot(fixture = MembershipState.fixture()) }
            onNodeWithTag("membership-account-registered").performScrollTo().performClick()
            onNodeWithTag("membership-heading").assert(hasText("Đăng ký hoàn tất."))
            onNodeWithTag("membership-plans-status").assertDoesNotExist()
            onNodeWithTag("membership-plans").performScrollTo().performClick()
            onNodeWithTag("membership-plans-status").assert(hasText("Chưa công bố các gói Membership."))
            onNodeWithText("Theo tháng · bố cục minh họa").assertExists()
            onNodeWithText("Theo năm · bố cục minh họa").assertExists()
            onNodeWithTag("back").performClick()
            onNodeWithTag("membership-heading").assert(hasText("Đăng ký hoàn tất."))
        }
    }

    @Test
    fun narrow_double_text_form_and_submit_reflow_inside_the_scrollable_viewport() {
        runDesktopComposeUiTest(width = 320, height = 800) {
            setContent { MembershipTestRoot(width = 320, height = 800, textScale = 2f) }
            onNodeWithTag("membership-start").performScrollTo().performClick()
            val header = onNodeWithTag("header-title").assert(hasContentDescription("Cantos")).fetchSemanticsNode().boundsInRoot
            assertTrue(header.height <= 160f, "The fixed header must leave most of the 800 dp viewport to the scrollable form at double text scale")
            val submit = onNodeWithTag("membership-submit").performScrollTo().assertIsDisplayed()
            val bounds = submit.fetchSemanticsNode().boundsInRoot
            assertTrue(bounds.left >= 0f && bounds.right <= 320f)
            assertTrue(bounds.height >= 60f)
            onNodeWithTag("membership-email").performScrollTo().assertIsDisplayed()
        }
    }
}

@Composable
private fun MembershipTestRoot(
    initial: ShellState = demoState(stack = listOf(ShellRoute.Account, ShellRoute.MembershipPreview)),
    fixture: MembershipState = MembershipState.unavailable(),
    width: Int = 390,
    height: Int = 844,
    textScale: Float = 1f,
) {
    Box(Modifier.requiredSize(width.dp, height.dp)) {
        CantosDemoApp(initialState = initial, membershipPreviewState = fixture, textScale = textScale)
    }
}
