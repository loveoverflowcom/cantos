package com.cantos.theatre.features.membership.presentation

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
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
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
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
import com.cantos.ui.LocalEditorialFontFamily
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
        Text(copy.text(MembershipText.PreviewNotice), style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        when (state.screen) {
            MembershipScreen.Intro -> {
                Image(painterResource(Res.drawable.son_ha_moon), null,
                    // Reference-specific artwork crop; canonical container geometry stays shared.
                    Modifier.fillMaxWidth().height(160.dp).clip(RoundedCornerShape(tokenDp("shape.membershipContainer"))),
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
                MembershipAction(copy.text(MembershipText.Plans), "membership-plans", tonal = true) { dispatch(MembershipEvent.OpenPlans) }
                TextButton(onClick = onExplore) { Text(copy.text(MembershipText.Explore)) }
            }
            MembershipScreen.Form -> {
                MembershipHeading(copy, MembershipText.FormTitle, MembershipText.FormBody)
                MembershipForm(state, copy, dispatch)
            }
            MembershipScreen.Received -> {
                MembershipHeading(copy, MembershipText.ReceivedTitle, MembershipText.ReceivedBody)
                MembershipCard {
                    Text(copy.text(MembershipText.ReceivedFixture), style = MaterialTheme.typography.labelLarge,
                        modifier = Modifier.testTag("membership-receipt"))
                    state.receipt?.payload?.email?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }
                }
                MembershipAction(copy.text(MembershipText.Plans), "membership-plans") { dispatch(MembershipEvent.OpenPlans) }
                MembershipAction(copy.text(MembershipText.Explore), "membership-explore", tonal = true, onPress = onExplore)
            }
            MembershipScreen.AccountRegistered -> {
                MembershipHeading(copy, MembershipText.AccountRegisteredTitle, MembershipText.AccountRegisteredBody)
                Text(copy.text(MembershipText.AccountFixture), style = MaterialTheme.typography.bodySmall)
                MembershipAction(copy.text(MembershipText.Plans), "membership-plans") { dispatch(MembershipEvent.OpenPlans) }
                MembershipAction(copy.text(MembershipText.Explore), "membership-explore", tonal = true, onPress = onExplore)
            }
            MembershipScreen.Account -> {
                MembershipHeading(copy, MembershipText.AccountTitle, MembershipText.AccountBody)
                MembershipAction(copy.text(MembershipText.Brand), "membership-intro") { dispatch(MembershipEvent.OpenIntro) }
                MembershipAction(copy.text(MembershipText.Plans), "membership-plans", tonal = true) { dispatch(MembershipEvent.OpenPlans) }
            }
            is MembershipScreen.Plans -> {
                MembershipHeading(copy, MembershipText.PlansTitle, MembershipText.PlansBody)
                MembershipCard {
                    Text(copy.plans(state.plansStatus), modifier = Modifier.testTag("membership-plans-status")
                        .semantics { liveRegion = LiveRegionMode.Polite })
                    Text(copy.text(MembershipText.NoCheckout), style = MaterialTheme.typography.bodySmall)
                }
                if (state.plansStatus == PlansStatus.Unpublished) {
                    BoxWithConstraints(Modifier.fillMaxWidth()) {
                        if (maxWidth >= tokenDp("breakpoint.medium")) {
                            Row(horizontalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                                MembershipPeriod(copy, MembershipText.Monthly, MembershipText.MonthTitle, MembershipText.MonthRate, Modifier.weight(1f))
                                MembershipPeriod(copy, MembershipText.Annual, MembershipText.YearTitle, MembershipText.YearRate, Modifier.weight(1f))
                            }
                        } else {
                            Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
                                MembershipPeriod(copy, MembershipText.Monthly, MembershipText.MonthTitle, MembershipText.MonthRate)
                                MembershipPeriod(copy, MembershipText.Annual, MembershipText.YearTitle, MembershipText.YearRate)
                            }
                        }
                    }
                }
                MembershipAction(copy.text(MembershipText.Explore), "membership-explore", tonal = true, onPress = onExplore)
            }
        }
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
    MembershipCard {
        TextField(state.draft.name, { dispatch(MembershipEvent.SetName(it)) },
            modifier = Modifier.fillMaxWidth().focusRequester(nameFocus).testTag("membership-name"),
            label = { Text(copy.text(MembershipText.Name)) }, readOnly = locked, singleLine = true,
            isError = nameError != null, supportingText = nameError?.let { { Text(copy.error(it)) } },
            shape = RoundedCornerShape(tokenDp("shape.authField")),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
            keyboardActions = KeyboardActions(onNext = { emailFocus.requestFocus() }))
        TextField(state.draft.email, { dispatch(MembershipEvent.SetEmail(it)) },
            modifier = Modifier.fillMaxWidth().focusRequester(emailFocus).testTag("membership-email"),
            label = { Text(copy.text(MembershipText.Email)) }, readOnly = locked, singleLine = true,
            isError = emailError != null, supportingText = emailError?.let { { Text(copy.error(it)) } },
            shape = RoundedCornerShape(tokenDp("shape.authField")),
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
        MembershipAction(copy.text(label), "membership-submit", enabled = state.submission != SubmissionStatus.Pending && state.submission != SubmissionStatus.Received) {
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

@Composable
private fun MembershipHeading(copy: MembershipCopy, title: MembershipText, body: MembershipText) {
    Column(verticalArrangement = Arrangement.spacedBy(tokenDp("space.sm"))) {
        Text(copy.text(MembershipText.Brand), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
        Text(copy.text(title), modifier = Modifier.testTag("membership-heading").semantics { heading() },
            style = MaterialTheme.typography.headlineMedium.copy(fontFamily = LocalEditorialFontFamily.current))
        Text(copy.text(body), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Illustrative reference periods only: no plan identity, accepted benefits, price or action. */
@Composable
private fun MembershipPeriod(copy: MembershipCopy, period: MembershipText, title: MembershipText, rate: MembershipText, modifier: Modifier = Modifier) {
    Surface(modifier.fillMaxWidth().semantics(mergeDescendants = true) {},
        shape = RoundedCornerShape(tokenDp("shape.membershipContainer")), color = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.padding(tokenDp("space.lg")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) {
            Text(copy.text(period), style = MaterialTheme.typography.labelLarge)
            Text(copy.text(title), style = MaterialTheme.typography.headlineSmall.copy(fontFamily = LocalEditorialFontFamily.current))
            Text(copy.text(MembershipText.PriceUnpublished), style = MaterialTheme.typography.titleLarge)
            Text(copy.text(rate), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun MembershipCard(content: @Composable () -> Unit) {
    Surface(Modifier.fillMaxWidth(), shape = RoundedCornerShape(tokenDp("shape.authContainer")), color = MaterialTheme.colorScheme.surfaceContainer) {
        Column(Modifier.padding(tokenDp("space.lg")), verticalArrangement = Arrangement.spacedBy(tokenDp("space.md"))) { content() }
    }
}

@Composable
private fun MembershipAction(label: String, tag: String, tonal: Boolean = false, enabled: Boolean = true, onPress: () -> Unit) {
    Button(onClick = onPress, enabled = enabled,
        modifier = Modifier.fillMaxWidth().heightIn(min = tokenDp(if (tonal) "size.authSecondary" else "size.authPrimary"))
            .testTag(tag).semantics { if (!enabled) stateDescription = label },
        shape = RoundedCornerShape(tokenDp("shape.pill")),
        colors = if (tonal) ButtonDefaults.filledTonalButtonColors() else ButtonDefaults.buttonColors()) { Text(label) }
}

private class MembershipCopy(private val locale: DemoLocale) {
    fun text(key: MembershipText): String = if (locale == DemoLocale.English) key.en else key.vi
    fun plans(status: PlansStatus): String = text(when (status) {
        PlansStatus.Unpublished -> MembershipText.Unpublished
        PlansStatus.Loading -> MembershipText.PlansLoading
        PlansStatus.Empty -> MembershipText.PlansEmpty
        PlansStatus.Unavailable -> MembershipText.PlansUnavailable
        PlansStatus.Failed -> MembershipText.PlansFailed
    })
    fun error(error: MembershipFieldError): String = text(when (error) {
        MembershipFieldError.NameRequired -> MembershipText.NameRequired
        MembershipFieldError.NameTooLong -> MembershipText.NameTooLong
        MembershipFieldError.NameInvalid -> MembershipText.NameInvalid
        MembershipFieldError.EmailRequired -> MembershipText.EmailRequired
        MembershipFieldError.EmailTooLong -> MembershipText.EmailTooLong
        MembershipFieldError.EmailInvalid -> MembershipText.EmailInvalid
    })
}

private enum class MembershipText(val vi: String, val en: String) {
    Brand("Cantos Membership", "Cantos Membership"),
    PreviewNotice("Bản xem thử · Không gửi đăng ký, tạo tài khoản hoặc thanh toán.", "Preview · No registration, account creation or payment is sent."),
    IntroTitle("Lắng nghe. Và đồng hành.", "Listen. And belong."),
    IntroBody("Một góc dành cho những người muốn đi xa hơn cùng câu chuyện.", "A place for those who want to go further with a story."),
    ComingSoon("Sắp ra mắt", "Coming soon"),
    IntroCardBody("Đồng hành cùng những câu chuyện bạn yêu. Chỉ cần tên và email để bắt đầu.", "Support the stories you love. Start with your name and email."),
    FormTitle("Thêm một người đồng hành.", "One more companion."),
    FormBody("Để Cantos gửi thông tin hội viên đến bạn.", "Let Cantos send membership information to you."),
    Name("Tên của bạn", "Your name"), Email("Email", "Email"),
    Submit("Đăng ký Membership", "Register interest in Membership"),
    Plans("Xem các gói Membership", "View Membership plans"),
    Explore("Để sau, tiếp tục khám phá", "Later, continue exploring"),
    ViewReceipt("Xem xác nhận minh họa", "View sample receipt"),
    ReceivedTitle("Đăng ký đã hoàn tất.", "Registration received."),
    ReceivedBody("Bây giờ bạn có thể xem các gói Membership, hoặc tiếp tục khám phá và quyết định sau.", "You can view Membership plans now, or keep exploring and decide later."),
    ReceivedFixture("Xác nhận từ fixture · Chưa gửi đăng ký thật", "Fixture receipt · No real request was sent"),
    AccountRegisteredTitle("Đăng ký hoàn tất.", "Account registration complete."),
    AccountRegisteredBody("Bạn có thể bắt đầu khám phá hoặc xem các gói Membership khi muốn.", "Start exploring or view Membership plans whenever you want."),
    AccountFixture("Tài khoản minh họa được chọn thủ công, không phải kết quả xác thực.", "Manually selected account fixture, not an authentication result."),
    AccountTitle("Không gian của bạn", "Your space"),
    AccountBody("Điểm quay lại của bản xem thử. Tài khoản thật không bị thay đổi.", "Preview return destination. Your real account is unchanged."),
    PlansTitle("Chọn nhịp đồng hành.", "Find your rhythm."),
    PlansBody("Xem thông tin các gói ở màn riêng. Bạn có thể quyết định sau.", "Explore plans on their own screen. You can decide later."),
    Unpublished("Chưa công bố các gói Membership.", "Membership plans have not been announced."),
    Monthly("Theo tháng · bố cục minh họa", "Monthly · illustrative layout"),
    Annual("Theo năm · bố cục minh họa", "Annual · illustrative layout"),
    MonthTitle("Một tháng.", "One month."), YearTitle("Một năm.", "One year."),
    PriceUnpublished("Chưa công bố", "Not announced"),
    MonthRate("Mức phí / tháng", "Price / month"), YearRate("Mức phí / năm", "Price / year"),
    PlansLoading("Fixture: đang tải thông tin gói…", "Fixture: loading plan information…"),
    PlansEmpty("Fixture: chưa có gói để hiển thị.", "Fixture: no plans to display."),
    PlansUnavailable("Thông tin các gói hiện chưa khả dụng.", "Plan information is currently unavailable."),
    PlansFailed("Fixture: chưa tải được thông tin gói. Xác nhận đăng ký vẫn được giữ.", "Fixture: plan information could not load. Your receipt is retained."),
    NoCheckout("Giá và quyền lợi sẽ được công bố trước khi bạn quyết định. Hiện chưa mở chọn gói hoặc thanh toán.", "Prices and benefits will be announced before you decide. Plan selection and checkout are currently unavailable."),
    Unavailable("Đăng ký thật chưa khả dụng. Nội dung đã nhập chỉ được giữ trong phiên xem thử này.", "Real registration is unavailable. Entered text is retained only in this preview session."),
    Pending("Fixture đang chờ kết quả thủ công. Không gửi yêu cầu lần hai.", "Fixture awaiting a manual outcome. A duplicate request is blocked."),
    PendingAction("Đang chờ xác nhận…", "Awaiting confirmation…"),
    Unknown("Chưa biết kết quả. Giữ nguyên tên, email và ý định; kiểm tra lại trước khi gửi khác.", "The outcome is unknown. Name, email and intent stay locked until reconciliation."),
    Failed("Fixture chưa xác nhận đăng ký. Bạn có thể thử lại hoặc sửa nội dung.", "The fixture did not confirm registration. Retry or correct your details."),
    Retry("Thử lại đăng ký", "Retry registration"), Reconcile("Kiểm tra lại yêu cầu", "Check the same request"),
    FixtureTools("Công cụ fixture · Chọn kết quả thủ công", "Fixture tools · Choose an outcome manually"),
    ConfirmFixture("Fixture: xác nhận đã nhận đăng ký", "Fixture: confirm receipt"),
    FailFixture("Fixture: lỗi chưa nhận đăng ký", "Fixture: request failed"),
    TimeoutFixture("Fixture: timeout, kết quả chưa biết", "Fixture: timeout, outcome unknown"),
    AccountFixtureAction("Fixture: đăng ký tài khoản hoàn tất", "Fixture: account registration complete"),
    ClearSession("Fixture: đăng xuất / đổi tài khoản / hết phiên", "Fixture: logout / switch account / session expired"),
    NameRequired("Nhập tên bạn muốn sử dụng.", "Enter the name you want to use."),
    NameTooLong("Tên tối đa 120 ký tự Unicode.", "Use no more than 120 Unicode characters for your name."),
    NameInvalid("Tên không được chứa ký tự điều khiển.", "Your name must not contain control characters."),
    EmailRequired("Nhập địa chỉ email.", "Enter your email address."),
    EmailTooLong("Email tối đa 254 ký tự Unicode.", "Use no more than 254 Unicode characters for email."),
    EmailInvalid("Nhập email hợp lệ với một dấu @ và không có khoảng trắng.", "Enter an email with one @ and no whitespace."),
}
