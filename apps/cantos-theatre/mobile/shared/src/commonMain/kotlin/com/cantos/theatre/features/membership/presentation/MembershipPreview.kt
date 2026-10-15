package com.cantos.theatre.features.membership.presentation

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.domain.membership.MembershipEvent
import com.cantos.theatre.domain.membership.MembershipFieldError
import com.cantos.theatre.domain.membership.MembershipFinishOutcome
import com.cantos.theatre.domain.membership.MembershipMode
import com.cantos.theatre.domain.membership.MembershipScreen
import com.cantos.theatre.domain.membership.MembershipState
import com.cantos.theatre.domain.membership.PlansStatus
import com.cantos.theatre.domain.membership.SubmissionStatus
import com.cantos.theatre.resources.Res
import com.cantos.theatre.resources.son_ha_moon
import com.cantos.ui.ButtonState
import com.cantos.ui.ButtonVariant
import com.cantos.ui.DisabledReason
import com.cantos.ui.FieldState
import com.cantos.ui.TextField
import com.cantos.ui.TextFieldVariant
import com.cantos.ui.tokenDp
import org.jetbrains.compose.resources.painterResource

/** Controlled preview of the actual pure core. It never dispatches HTTP, auth or audio. */
@Composable
fun MembershipPreview(
    state: MembershipState,
    locale: DemoLocale,
    dispatch: (MembershipEvent) -> Unit,
    onExplore: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val copy = MembershipCopy(locale)
    Column(modifier.fillMaxWidth().testTag("membership-preview"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.lg"))) {
        when (state.screen) {
            MembershipScreen.Intro -> {
                Image(painterResource(Res.drawable.son_ha_moon), null,
                    // Reference-specific artwork crop; canonical container geometry stays shared.
                    Modifier.fillMaxWidth().aspectRatio(2f).clip(RoundedCornerShape(tokenDp("shape.membershipContainer"))),
                    contentScale = ContentScale.Crop)
                MembershipHeading(copy, MembershipText.IntroTitle, MembershipText.IntroBody)
                MembershipCard {
                    Text(copy.text(MembershipText.Brand), style = MaterialTheme.typography.titleLarge)
                    Text(copy.text(MembershipText.ComingSoon), style = MaterialTheme.typography.labelLarge)
                    Text(copy.text(MembershipText.IntroCardBody), style = MaterialTheme.typography.bodyMedium)
                }
                MembershipAction(copy.text(if (state.receipt == null) MembershipText.Submit else MembershipText.ViewReceipt), "membership-start") {
                    dispatch(if (state.receipt == null) MembershipEvent.OpenForm else MembershipEvent.ShowReceived)
                }
                MembershipAction(copy.text(MembershipText.Plans), "membership-plans", variant = ButtonVariant.AuthSecondary) { dispatch(MembershipEvent.OpenPlans) }
                TextButton(onClick = onExplore) { Text(copy.text(MembershipText.Explore)) }
            }
            MembershipScreen.Form -> {
                MembershipHeading(copy, MembershipText.FormTitle, MembershipText.FormBody)
                MembershipForm(state, copy, dispatch)
            }
            MembershipScreen.Received -> {
                MembershipSuccessSymbol()
                MembershipHeading(copy, MembershipText.ReceivedTitle, MembershipText.ReceivedBody, MembershipText.ThankYou)
                MembershipReceipt(copy, state.receipt?.payload?.email)
                MembershipAction(copy.text(MembershipText.Plans), "membership-plans") { dispatch(MembershipEvent.OpenPlans) }
                MembershipAction(copy.text(MembershipText.Explore), "membership-explore", variant = ButtonVariant.AuthSecondary, onPress = onExplore)
            }
            MembershipScreen.AccountRegistered -> {
                MembershipSuccessSymbol()
                MembershipHeading(copy, MembershipText.AccountRegisteredTitle, MembershipText.AccountRegisteredBody)
                Text(copy.text(MembershipText.AccountFixture), style = MaterialTheme.typography.bodySmall)
                MembershipAction(copy.text(MembershipText.Plans), "membership-plans") { dispatch(MembershipEvent.OpenPlans) }
                MembershipAction(copy.text(MembershipText.Explore), "membership-explore", variant = ButtonVariant.AuthSecondary, onPress = onExplore)
            }
            MembershipScreen.Account -> {
                MembershipHeading(copy, MembershipText.AccountTitle, MembershipText.AccountBody)
                MembershipAction(copy.text(MembershipText.Brand), "membership-intro") { dispatch(MembershipEvent.OpenIntro) }
                MembershipAction(copy.text(MembershipText.Plans), "membership-plans", variant = ButtonVariant.AuthSecondary) { dispatch(MembershipEvent.OpenPlans) }
            }
            is MembershipScreen.Plans -> {
                MembershipHeading(copy, MembershipText.PlansTitle, MembershipText.PlansBody)
                if (state.plansStatus == PlansStatus.Unpublished) {
                    BoxWithConstraints(Modifier.fillMaxWidth()) {
                        if (maxWidth >= tokenDp("breakpoint.medium")) {
                            Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                                MembershipPlanCard(copy, MembershipPeriod.Monthly, Modifier.weight(1f))
                                MembershipPlanCard(copy, MembershipPeriod.Annual, Modifier.weight(1f))
                            }
                        } else {
                            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                                MembershipPlanCard(copy, MembershipPeriod.Monthly)
                                MembershipPlanCard(copy, MembershipPeriod.Annual)
                            }
                        }
                    }
                }
                Text(copy.plans(state.plansStatus), modifier = Modifier.testTag("membership-plans-status")
                    .semantics { liveRegion = LiveRegionMode.Polite }, style = MaterialTheme.typography.bodySmall)
                Text(copy.text(MembershipText.NoCheckout), style = MaterialTheme.typography.bodySmall)
                MembershipAction(copy.text(MembershipText.Explore), "membership-explore", variant = ButtonVariant.AuthSecondary, onPress = onExplore)
            }
        }
        Text(copy.text(MembershipText.PreviewNotice), style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.testTag("membership-preview-notice"))
        if (state.mode == MembershipMode.Unavailable) {
            Text(copy.text(MembershipText.Unavailable), modifier = Modifier.testTag("membership-unavailable"),
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        } else {
            FixtureControls(state, copy, dispatch)
        }
    }
}

@Composable
private fun MembershipForm(state: MembershipState, copy: MembershipCopy, dispatch: (MembershipEvent) -> Unit) {
    val nameFocus = remember { FocusRequester() }
    val emailFocus = remember { FocusRequester() }
    // An equal finding list is still a new failed submit; focus follows that intent too.
    var submitFocusTrigger by remember { mutableStateOf(false) }
    val submit = {
        dispatch(MembershipEvent.Submit)
        submitFocusTrigger = !submitFocusTrigger
    }
    val locked = state.submission == SubmissionStatus.Pending || state.submission == SubmissionStatus.Unknown || state.submission == SubmissionStatus.Received
    val nameError = state.fieldErrors.firstOrNull { it == MembershipFieldError.NameRequired || it == MembershipFieldError.NameTooLong || it == MembershipFieldError.NameInvalid }
    val emailError = state.fieldErrors.firstOrNull { it == MembershipFieldError.EmailRequired || it == MembershipFieldError.EmailTooLong || it == MembershipFieldError.EmailInvalid }
    LaunchedEffect(state.fieldErrors, submitFocusTrigger) {
        if (state.fieldErrors.isNotEmpty()) {
            withFrameNanos { }
            if (nameError != null) nameFocus.requestFocus() else emailFocus.requestFocus()
        }
    }
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(tokenDp("space.lg"))) {
        TextField(label = copy.text(MembershipText.Name), value = state.draft.name, onValueChange = { dispatch(MembershipEvent.SetName(it)) },
            controlModifier = Modifier.focusRequester(nameFocus).testTag("membership-name"),
            state = if (locked) FieldState.ReadOnly else FieldState.Editable,
            error = nameError?.let(copy::error), variant = TextFieldVariant.AuthTonal,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
            keyboardActions = KeyboardActions(onNext = { emailFocus.requestFocus() }))
        TextField(label = copy.text(MembershipText.Email), value = state.draft.email, onValueChange = { dispatch(MembershipEvent.SetEmail(it)) },
            controlModifier = Modifier.focusRequester(emailFocus).testTag("membership-email"),
            state = if (locked) FieldState.ReadOnly else FieldState.Editable,
            error = emailError?.let(copy::error), variant = TextFieldVariant.AuthTonal,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { if (state.submission != SubmissionStatus.Pending) submit() }))
        val status = when (state.submission) {
            SubmissionStatus.Idle -> null
            SubmissionStatus.Pending -> MembershipText.Pending
            SubmissionStatus.Unknown -> MembershipText.Unknown
            SubmissionStatus.Failed -> if (state.mode == MembershipMode.Unavailable) MembershipText.Unavailable else MembershipText.Failed
            SubmissionStatus.Received -> MembershipText.ReceivedFixture
        }
        status?.let { Text(copy.text(it), modifier = Modifier.testTag("membership-submission-status")
            .semantics { liveRegion = LiveRegionMode.Polite }, style = MaterialTheme.typography.bodySmall) }
        val label = when (state.submission) {
            SubmissionStatus.Pending -> MembershipText.PendingAction
            SubmissionStatus.Unknown -> MembershipText.Reconcile
            SubmissionStatus.Failed -> MembershipText.Retry
            SubmissionStatus.Idle, SubmissionStatus.Received -> MembershipText.Submit
        }
        val actionState = when (state.submission) {
            SubmissionStatus.Pending -> ButtonState.Loading(DisabledReason.of(copy.text(MembershipText.Pending)))
            SubmissionStatus.Received -> ButtonState.Disabled(DisabledReason.of(copy.text(MembershipText.ReceivedFixture)))
            SubmissionStatus.Idle, SubmissionStatus.Unknown, SubmissionStatus.Failed -> ButtonState.Enabled
        }
        MembershipAction(copy.text(label), "membership-submit", state = actionState) {
            submit()
        }
    }
}

@Composable
private fun FixtureControls(state: MembershipState, copy: MembershipCopy, dispatch: (MembershipEvent) -> Unit) {
    Column(Modifier.fillMaxWidth().testTag("membership-fixture-controls"), verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(copy.text(MembershipText.FixtureTools), style = MaterialTheme.typography.labelLarge)
        state.pendingAttempt?.let { attempt ->
            listOf(MembershipFinishOutcome.Received to MembershipText.ConfirmFixture,
                MembershipFinishOutcome.Failed to MembershipText.FailFixture,
                MembershipFinishOutcome.Unknown to MembershipText.TimeoutFixture).forEach { (outcome, label) ->
                TextButton(onClick = { dispatch(MembershipEvent.Finish(attempt, outcome)) }, modifier = Modifier.testTag("membership-fixture-$outcome")) {
                    Text(copy.text(label))
                }
            }
        }
        if (state.screen is MembershipScreen.Plans) PlansStatus.entries.forEach { status ->
            TextButton(onClick = { dispatch(MembershipEvent.SetPlansStatus(status)) }, modifier = Modifier.testTag("membership-plan-$status")) {
                Text(copy.plans(status))
            }
        }
        TextButton(onClick = { dispatch(MembershipEvent.ShowAccountRegisteredFixture) }, modifier = Modifier.testTag("membership-account-registered")) {
            Text(copy.text(MembershipText.AccountFixtureAction))
        }
        TextButton(onClick = { dispatch(MembershipEvent.ClearSession) }, modifier = Modifier.testTag("membership-clear-session")) {
            Text(copy.text(MembershipText.ClearSession))
        }
    }
}
