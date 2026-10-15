package com.cantos.theatre.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.cantos.theatre.domain.membership.MembershipEvent
import com.cantos.theatre.domain.membership.MembershipFinishOutcome
import com.cantos.theatre.domain.membership.MembershipScreen
import com.cantos.theatre.domain.membership.MembershipState
import com.cantos.theatre.domain.membership.PlansStatus
import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.ui.tokenDp

/** Manual diagnostic outcomes belong to the sandbox, while plan labels remain shared. */
@Composable
fun MembershipFixtureControls(
    state: MembershipState,
    locale: ShellLocale,
    dispatch: (MembershipEvent) -> Unit,
    plansLabel: (PlansStatus) -> String,
) {
    Column(Modifier.fillMaxWidth().testTag("membership-fixture-controls"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(MembershipFixtureText.FixtureTools.localized(locale), style = MaterialTheme.typography.labelLarge)
        state.pendingAttempt?.let { attempt ->
            listOf(MembershipFinishOutcome.Received to MembershipFixtureText.ConfirmFixture,
                MembershipFinishOutcome.Failed to MembershipFixtureText.FailFixture,
                MembershipFinishOutcome.Unknown to MembershipFixtureText.TimeoutFixture).forEach { (outcome, label) ->
                TextButton(onClick = { dispatch(MembershipEvent.Finish(attempt, outcome)) }, modifier = Modifier.testTag("membership-fixture-$outcome")) {
                    Text(label.localized(locale))
                }
            }
        }
        if (state.screen is MembershipScreen.Plans) PlansStatus.entries.forEach { status ->
            TextButton(onClick = { dispatch(MembershipEvent.SetPlansStatus(status)) }, modifier = Modifier.testTag("membership-plan-$status")) {
                Text(plansLabel(status))
            }
        }
        TextButton(onClick = { dispatch(MembershipEvent.ShowAccountRegisteredFixture) }, modifier = Modifier.testTag("membership-account-registered")) {
            Text(MembershipFixtureText.AccountFixtureAction.localized(locale))
        }
        TextButton(onClick = { dispatch(MembershipEvent.ClearSession) }, modifier = Modifier.testTag("membership-clear-session")) {
            Text(MembershipFixtureText.ClearSession.localized(locale))
        }
    }
}

private enum class MembershipFixtureText(val vi: String, val en: String) {
    FixtureTools("Công cụ fixture · Chọn kết quả thủ công", "Fixture tools · Choose an outcome manually"),
    ConfirmFixture("Fixture: xác nhận đã nhận đăng ký", "Fixture: confirm receipt"),
    FailFixture("Fixture: lỗi chưa nhận đăng ký", "Fixture: request failed"),
    TimeoutFixture("Fixture: timeout, kết quả chưa biết", "Fixture: timeout, outcome unknown"),
    AccountFixtureAction("Fixture: đăng ký tài khoản hoàn tất", "Fixture: account registration complete"),
    ClearSession("Fixture: đăng xuất / đổi tài khoản / hết phiên", "Fixture: logout / switch account / session expired");

    fun localized(locale: ShellLocale): String = if (locale == ShellLocale.English) en else vi
}
