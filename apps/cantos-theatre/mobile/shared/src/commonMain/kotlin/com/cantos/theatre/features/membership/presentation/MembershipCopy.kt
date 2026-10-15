package com.cantos.theatre.features.membership.presentation

import com.cantos.theatre.demo.DemoLocale
import com.cantos.theatre.domain.membership.MembershipFieldError
import com.cantos.theatre.domain.membership.PlansStatus

internal class MembershipCopy(private val locale: DemoLocale) {
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

internal enum class MembershipText(val vi: String, val en: String) {
    AppBrand("Cantos", "Cantos"),
    Back("Quay lại", "Back"),
    ToggleTheme("Chuyển giao diện sáng tối", "Switch light and dark theme"),
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
    ThankYou("Cảm ơn người đồng hành", "Thank you for joining us"),
    ReceiptBadge("Đã đăng ký · minh họa", "Received · sample"),
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
    MonthAction("Xem gói tháng", "View monthly plan"),
    YearAction("Xem gói năm", "View annual plan"),
    PlanDetailsUnavailable("Chi tiết gói chưa công bố.", "Plan details have not been announced."),
    MonthBody("Tham gia theo từng tháng, theo nhịp nghe của bạn.", "Join month by month, at your listening pace."),
    YearBody("Một chặng đồng hành dài hơi cùng những câu chuyện.", "A longer journey alongside the stories."),
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
