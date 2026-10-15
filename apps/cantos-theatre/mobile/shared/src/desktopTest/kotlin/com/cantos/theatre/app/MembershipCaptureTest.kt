package com.cantos.theatre.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.domain.membership.MembershipEvent
import com.cantos.theatre.domain.membership.MembershipFinishOutcome
import com.cantos.theatre.domain.membership.MembershipResult
import com.cantos.theatre.domain.membership.MembershipState
import com.cantos.theatre.domain.membership.PlansStatus
import java.io.File
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

/** Declared captures are not visual evidence until executed and opened. JVM only. */
@OptIn(ExperimentalTestApi::class)
class MembershipCaptureTest {
    @Test
    fun membership_intro_form_and_explicit_fixture_receipt_have_named_capture_scenarios() {
        capture("membership-jvm-light-intro-390x844", MembershipState.unavailable())
        capture("membership-jvm-light-form-390x844", MembershipState.unavailable().next(MembershipEvent.OpenForm))
        capture("membership-jvm-dark-form-390x844", MembershipState.unavailable().next(MembershipEvent.OpenForm), theme = DemoTheme.Dark)
        capture("membership-jvm-light-form-en-390x844", MembershipState.unavailable().next(MembershipEvent.OpenForm), locale = DemoLocale.English)
        capture("membership-jvm-light-form-text2-320x800", MembershipState.unavailable().next(MembershipEvent.OpenForm),
            width = 320, height = 800, textScale = 2f, scrollTo = "membership-submit")
        val receipt = receivedFixture()
        capture("membership-jvm-dark-receipt-390x844", receipt, theme = DemoTheme.Dark)
        capture("membership-jvm-light-receipt-text2-top-320x800", receipt,
            width = 320, height = 800, textScale = 2f)
        capture("membership-jvm-light-receipt-text2-actions-320x800", receipt,
            width = 320, height = 800, textScale = 2f, scrollTo = "membership-explore")
        capture("membership-jvm-light-plans-text2-top-320x800", receipt.next(MembershipEvent.OpenPlans),
            width = 320, height = 800, textScale = 2f)
        capture("membership-jvm-light-plans-text2-actions-320x800", receipt.next(MembershipEvent.OpenPlans),
            width = 320, height = 800, textScale = 2f, scrollTo = "membership-explore")
        capture("membership-jvm-light-plans-unpublished-390x844", receipt.next(MembershipEvent.OpenPlans))
        capture("membership-jvm-light-plans-failed-390x844", receipt.next(MembershipEvent.OpenPlans)
            .next(MembershipEvent.SetPlansStatus(PlansStatus.Failed)))
    }

    private fun receivedFixture(): MembershipState {
        val form = MembershipState.fixture().next(MembershipEvent.OpenForm)
            .next(MembershipEvent.SetName("Nguyễn Thị Ánh")).next(MembershipEvent.SetEmail("anh@example.com"))
        val started = form.reduce(MembershipEvent.Submit)
        val attempt = assertIs<MembershipResult.Started>(started.result).attempt
        return started.state.next(MembershipEvent.Finish(attempt, MembershipFinishOutcome.Received))
    }

    private fun capture(
        name: String,
        membership: MembershipState,
        theme: DemoTheme = DemoTheme.Light,
        locale: DemoLocale = DemoLocale.Vietnamese,
        width: Int = 390,
        height: Int = 844,
        textScale: Float = 1f,
        scrollTo: String? = null,
    ) {
        runDesktopComposeUiTest(width = width, height = height) {
            setContent {
                Box(Modifier.requiredSize(width.dp, height.dp).testTag("membership-capture-root")) {
                    CantosDemoApp(initialState = DemoState(stack = listOf(DemoRoute.Account, DemoRoute.MembershipPreview), theme = theme, locale = locale),
                        membershipPreviewState = membership, textScale = textScale)
                }
            }
            scrollTo?.let { onNodeWithTag(it).performScrollTo() }
            waitForIdle()
            val image = onNodeWithTag("membership-capture-root").captureToImage()
            assertEquals(width, image.width)
            assertEquals(height, image.height)
            val output = File(System.getenv("CANTOS_THEATRE_DEMO_CAPTURE_DIR") ?: "build/reports/theatre-demo")
            check(output.isDirectory || output.mkdirs())
            val bytes = requireNotNull(Image.makeFromBitmap(image.asSkiaBitmap()).encodeToData(EncodedImageFormat.PNG)).bytes
            File(output, "$name.png").writeBytes(bytes)
            File(output, "$name.semantics.txt").writeText(onNodeWithTag("membership-capture-root", useUnmergedTree = true).printToString())
            File(output, "$name.txt").writeText("JVM Desktop Compose UI test; CantosDemoApp; $width x $height dp; " +
                "theme=$theme; locale=$locale; textScale=$textScale; mode=${membership.mode}; screen=${membership.screen}; " +
                "fixture data only; screenshot-captured; inspection pending\n")
        }
    }

    private fun MembershipState.next(event: MembershipEvent): MembershipState = reduce(event).state
}
