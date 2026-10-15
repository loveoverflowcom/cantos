package com.cantos.theatre.app

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import com.cantos.theatre.demo.DemoRoute
import com.cantos.theatre.demo.DemoState
import com.cantos.theatre.demo.DemoTheme
import com.cantos.theatre.domain.membership.MembershipEvent
import com.cantos.theatre.domain.membership.MembershipFinishOutcome
import com.cantos.theatre.domain.membership.MembershipResult
import com.cantos.theatre.domain.membership.MembershipState

/** Android layout-renderer fixtures around the mounted app, never an alternate screen. */
@Preview(name = "Membership form · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreMembershipFormLightPreview() = MembershipSpecimen(MembershipState.unavailable().reduce(MembershipEvent.OpenForm).state)

@Preview(name = "Membership form · vi · dark", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreMembershipFormDarkPreview() = MembershipSpecimen(MembershipState.unavailable().reduce(MembershipEvent.OpenForm).state, DemoTheme.Dark)

@Preview(name = "Membership receipt · fixture · vi · light", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreMembershipReceiptLightPreview() = MembershipSpecimen(receivedFixture())

@Preview(name = "Membership receipt · fixture · vi · dark", widthDp = 390, heightDp = 844, locale = "vi")
@Composable
fun TheatreMembershipReceiptDarkPreview() = MembershipSpecimen(receivedFixture(), DemoTheme.Dark)

@Preview(name = "Membership periods · unpublished · vi · light", widthDp = 390, heightDp = 1320, locale = "vi")
@Composable
fun TheatreMembershipPlansLightPreview() = MembershipSpecimen(receivedFixture().reduce(MembershipEvent.OpenPlans).state)

@Preview(name = "Membership periods · unpublished · vi · dark", widthDp = 390, heightDp = 1320, locale = "vi")
@Composable
fun TheatreMembershipPlansDarkPreview() = MembershipSpecimen(receivedFixture().reduce(MembershipEvent.OpenPlans).state, DemoTheme.Dark)

@Preview(name = "Membership form · vi · narrow · 2x", widthDp = 320, heightDp = 1000, locale = "vi", fontScale = 2f)
@Composable
fun TheatreMembershipFormLargePreview() = MembershipSpecimen(MembershipState.unavailable().reduce(MembershipEvent.OpenForm).state)

@Composable
private fun MembershipSpecimen(membership: MembershipState, theme: DemoTheme = DemoTheme.Light) {
    CantosDemoApp(
        initialState = DemoState(stack = listOf(DemoRoute.Account, DemoRoute.MembershipPreview), theme = theme),
        membershipPreviewState = membership,
    )
}

private fun receivedFixture(): MembershipState {
    val form = listOf(MembershipEvent.OpenForm, MembershipEvent.SetName("Nguyễn Thị Ánh"), MembershipEvent.SetEmail("anh@example.com"))
        .fold(MembershipState.fixture()) { state, event -> state.reduce(event).state }
    val started = form.reduce(MembershipEvent.Submit)
    val result = started.result
    check(result is MembershipResult.Started)
    return started.state.reduce(MembershipEvent.Finish(result.attempt, MembershipFinishOutcome.Received)).state
}
