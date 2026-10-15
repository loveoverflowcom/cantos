use crate::messages::Locale;

macro_rules! resources {
    ($($key:ident => ($vi:literal, $en:literal)),+ $(,)?) => {
        #[derive(Clone, Copy)]
        pub(super) enum CopyKey { $($key),+ }
        pub(super) fn text(key: CopyKey, locale: Locale) -> &'static str {
            match (key, locale) {
                $((CopyKey::$key, Locale::Vietnamese) => $vi, (CopyKey::$key, Locale::English) => $en),+
            }
        }
    };
}

resources! {
    Exit => ("Về tài khoản mẫu", "Back to mock account"),
    Dark => ("Giao diện tối", "Dark theme"),
    Language => ("Ngôn ngữ", "Language"),
    PreviewNotice => ("Bản xem thử Membership · Không gửi thông tin, tạo tài khoản, thu tiền hay cấp quyền nghe. Dữ liệu chỉ trong phiên; tải lại sẽ xóa.", "Membership preview · No contact submission, account creation, payment or listening access. Session only; reloading clears data."),
    StoryTitle => ("Một chỗ ngồi trong thế giới Cantos.", "A place in the world of Cantos."),
    StoryBody => ("Cùng nuôi dưỡng những câu chuyện và những thanh âm đáng nhớ.", "Support stories and voices worth remembering."),
    IntroTitle => ("Lắng nghe. Và đồng hành.", "Listen. And come along."),
    IntroBody => ("Một góc dành cho những người muốn đi xa hơn cùng câu chuyện.", "A place for those who want to travel further with stories."),
    Register => ("Đăng ký Membership", "Register Membership interest"),
    ViewPlans => ("Xem các gói Membership", "View Membership plans"),
    ReceivedTitle => ("Đăng ký đã hoàn tất.", "Registration complete."),
    ReceivedBody => ("Receipt minh họa đã xác nhận thông tin quan tâm. Bạn có thể xem gói hoặc tiếp tục khám phá.", "A fixture receipt confirms interest information. You can view plans or keep exploring."),
    InterestOnly => ("Đã ghi nhận quan tâm · chưa có quyền hội viên trả phí", "Interest received · no paid membership access"),
    AccountCreated => ("Đăng ký tài khoản hoàn tất · mẫu", "Account registration complete · fixture"),
    AccountFixture => ("Đây là trạng thái minh họa, không tạo tài khoản hay tự gửi thông tin Membership.", "This is a fixture. It creates no account and submits no Membership interest."),
    Account => ("Tài khoản", "Account"),
    Unavailable => ("Đăng ký Membership chưa khả dụng. Chưa có dịch vụ tiếp nhận được chấp nhận; thông tin không được gửi.", "Membership registration is unavailable. No accepted intake service exists; information is not sent."),
    About => ("Tìm hiểu Membership", "About Membership"),
    Tools => ("Điều khiển bản xem thử", "Preview controls"),
    ToolsHelp => ("Chọn chế độ fixture để thử trạng thái. Kết quả được chọn bằng tay, không có API hay bộ hẹn giờ.", "Choose fixture mode to exercise states. Outcomes are selected manually; there is no API or timer."),
    EnableFixture => ("Bắt đầu phiên fixture trống", "Start empty fixture session"),
    AccountFixtureEntry => ("Mẫu tài khoản vừa tạo", "New account fixture"),
    ClearSession => ("Xóa phiên mẫu / đổi tài khoản", "Clear fixture session / switch account"),
    FixtureMode => ("Chế độ fixture · chỉ dùng thông tin mẫu", "Fixture mode · use sample information only"),
    Later => ("Để sau, tiếp tục khám phá", "Later, keep exploring"),
    FormTitle => ("Thêm một người đồng hành.", "One more companion."),
    FormBody => ("Để Cantos gửi thông tin hội viên đến bạn.", "Let Cantos send you membership information."),
    Name => ("Tên của bạn", "Your name"),
    Email => ("Email", "Email"),
    Reconcile => ("Kiểm tra lại cùng đăng ký", "Reconcile the same registration"),
    FixtureOutcome => ("Chọn kết quả fixture cho lần gửi đang chờ", "Choose a fixture outcome for the pending request"),
    Confirm => ("Fixture: xác nhận receipt", "Fixture: confirm receipt"),
    Fail => ("Fixture: lỗi chưa nhận", "Fixture: not received error"),
    Timeout => ("Fixture: timeout chưa rõ kết quả", "Fixture: outcome unknown timeout"),
    Pending => ("Đang chờ kết quả fixture. Không gửi lặp; thông tin được giữ nguyên.", "Awaiting fixture outcome. Duplicate submission is blocked; inputs are preserved."),
    Failed => ("Chưa nhận đăng ký trong fixture. Giữ nguyên thông tin để sửa hoặc thử lại.", "The fixture did not receive registration. Inputs remain available to correct or retry."),
    Unknown => ("Chưa rõ đăng ký đã được nhận hay chưa. Kiểm tra lại cùng operation và thông tin; không tạo đăng ký mới.", "Receipt is unknown. Reconcile the same operation and information; do not create a new registration."),
    NameRequired => ("Nhập tên của bạn.", "Enter your name."),
    NameTooLong => ("Tên tối đa 120 ký tự.", "Name must be at most 120 characters."),
    NameInvalid => ("Tên không chứa ký tự điều khiển.", "Name cannot contain control characters."),
    EmailRequired => ("Nhập email của bạn.", "Enter your email."),
    EmailInvalid => ("Kiểm tra email: một dấu @, không có khoảng trắng.", "Check email: one @ with no whitespace."),
    EmailTooLong => ("Email tối đa 254 ký tự.", "Email must be at most 254 characters."),
    BackIntro => ("Về giới thiệu Membership", "Back to Membership introduction"),
    BackReceived => ("Về đăng ký đã nhận", "Back to received registration"),
    BackRegistered => ("Về tài khoản vừa tạo", "Back to newly created account"),
    BackAccount => ("Về tài khoản", "Back to account"),
    PlansTitle => ("Chọn nhịp đồng hành.", "Choose your rhythm."),
    PlansBody => ("Các gói ở màn riêng. Bạn có thể quyết định sau.", "Plans have their own screen. You can decide later."),
    Unpublished => ("Chưa công bố", "Not announced"),
    PlansLoading => ("Đang tải các gói · fixture", "Loading plans · fixture"),
    PlansEmpty => ("Chưa có gói để hiển thị · fixture", "No plans to display · fixture"),
    PlansUnavailable => ("Dữ liệu gói chưa khả dụng · fixture", "Plan data unavailable · fixture"),
    PlansFailed => ("Không tải được gói · fixture; thông tin đã nhận được giữ nguyên", "Plan loading failed · fixture; received interest is preserved"),
    Month => ("Theo tháng", "Monthly"),
    Year => ("Theo năm", "Annual"),
    NoApprovedPlans => ("Cấu trúc minh họa; chưa có giá và quyền lợi được duyệt.", "Illustrative structure; no approved prices or benefits."),
    PlansFixture => ("Trạng thái dữ liệu gói mẫu", "Fixture plan data status"),
}
