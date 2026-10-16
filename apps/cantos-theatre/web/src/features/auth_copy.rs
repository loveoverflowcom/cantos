//! Bilingual resources for the app-owned auth/account compositions.
use crate::messages::Locale;

macro_rules! resources {
    ($($key:ident => ($vi:literal, $en:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum CopyKey { $($key),+ }
        pub(crate) fn text(key: CopyKey, locale: Locale) -> &'static str {
            match (key, locale) {
                $((CopyKey::$key, Locale::Vietnamese) => $vi,
                  (CopyKey::$key, Locale::English) => $en),+
            }
        }
        #[cfg(test)]
        const ALL: &[CopyKey] = &[$(CopyKey::$key),+];
    };
}

resources! {
    LoginEyebrow => ("CANTOS · MỘT LẦN TRỞ LẠI", "CANTOS · A PLACE TO RETURN"),
    LoginHeading => ("Câu chuyện đang đợi bạn.", "Your story is waiting."),
    LoginHelp => ("Đăng nhập để trở lại thư viện của bạn. Nội dung công khai vẫn có thể khám phá từ Trang chủ.", "Sign in to return to your library. Public stories are still available to explore from Home."),
    CantosIdLogin => ("Đăng nhập với Cantos ID", "Sign in with Cantos ID"),
    CreateAccount => ("Tạo tài khoản Cantos", "Create a Cantos account"),
    ProviderHelp => ("Mở bước xác thực an toàn, rồi quay lại Cantos. Bạn có thể quay lại mà chưa đăng nhập.", "Open secure authentication, then return to Cantos. You can return without signing in."),
    BrandHeading => ("Nhắm mắt lại. Mở một thế giới.", "Close your eyes. Open a world."),
    BrandHelp => ("Có những câu chuyện chỉ cần lắng nghe. Có những miền tưởng tượng luôn chờ bạn.", "Some stories only ask you to listen. Worlds of imagination are waiting for you."),
    BrandGenres => ("Truyện · Podcast · Kịch truyền thanh", "Stories · Podcasts · Audio drama"),
    RegisterEyebrow => ("CANTOS ID · TÀI KHOẢN CỦA BẠN", "CANTOS ID · YOUR ACCOUNT"),
    RegisterHeading => ("Một nơi để trở lại.", "A place to return to."),
    RegisterHelp => ("Xác minh email rồi tự thiết lập thông tin đăng nhập tại dịch vụ tài khoản. Cantos không nhận mật khẩu của bạn.", "Verify your email, then set up your sign-in details at the account service. Cantos does not collect your password."),
    SignupChecking => ("Đang kiểm tra khả năng tạo tài khoản…", "Checking account creation availability…"),
    SignupNotConfigured => ("Tạo tài khoản chưa khả dụng vì dịch vụ tài khoản chưa được cấu hình. Bạn vẫn có thể khám phá nội dung công khai.", "Account creation is unavailable because the account service is not configured. You can still explore public stories."),
    SignupProviderUnsupported => ("Tạo tài khoản chưa khả dụng vì dịch vụ tài khoản hiện chưa hỗ trợ tự đăng ký. Bạn vẫn có thể khám phá nội dung công khai.", "Account creation is unavailable because the account service does not currently support self-registration. You can still explore public stories."),
    SignupRegistrationUnconfigured => ("Tạo tài khoản hiện chưa được mở. Bạn vẫn có thể đăng nhập bằng tài khoản đã có hoặc khám phá nội dung công khai.", "Account creation is not open yet. You can still sign in with an existing account or explore public stories."),
    SignupEmail => ("Email của bạn", "Your email"),
    SignupEmailHelp => ("Chúng tôi dùng email này để xác minh yêu cầu đăng ký và gửi bước thiết lập tài khoản.", "We use this email to verify your sign-up request and send the account setup step."),
    SignupSendEmail => ("Gửi email xác minh", "Send verification email"),
    SignupSubmitting => ("Đang gửi yêu cầu…", "Sending request…"),
    SignupEmailAccepted => ("Yêu cầu đã được tiếp nhận. Nếu email đủ điều kiện, bạn sẽ nhận được link xác minh. Hãy kiểm tra cả thư rác. Bước này chưa xác nhận tài khoản hay đăng nhập.", "Your request has been accepted. If the email is eligible, you will receive a verification link. Check your spam folder too. This does not confirm an account or sign you in."),
    SignupInvalidEmail => ("Nhập một địa chỉ email hợp lệ.", "Enter a valid email address."),
    SignupRequestUnavailable => ("Chưa gửi được yêu cầu lúc này. Bạn có thể thử gửi lại sau.", "The request is unavailable right now. You can try sending it again later."),
    SignupRequestUnknown => ("Chưa xác nhận được kết quả gửi yêu cầu. Hãy kiểm tra email trước khi yêu cầu link mới; thư có thể đến chậm.", "The request outcome could not be confirmed. Check your email before requesting a new link; delivery may be delayed."),
    SignupConfirmHelp => ("Link đã được mở an toàn. Chọn xác nhận để tiếp tục yêu cầu thiết lập tài khoản. Chỉ mở link chưa tạo tài khoản và chưa đăng nhập.", "The link has been opened safely. Confirm to continue the account setup request. Opening the link alone does not create an account or sign you in."),
    SignupConfirm => ("Xác nhận email và tiếp tục", "Confirm email and continue"),
    SignupVerifying => ("Đang xử lý xác minh…", "Processing verification…"),
    SignupSetupLinkSent => ("Link thiết lập tài khoản đã được chuyển cho dịch vụ gửi email. Chưa xác nhận thư đã vào hộp thư hoặc bạn đã hoàn tất tài khoản. Mở email để tự thiết lập thông tin đăng nhập, rồi trở lại Đăng nhập.", "The account setup link was accepted by the email service. Inbox delivery and completed account setup have not been confirmed. Open the email to set up your sign-in details, then return to sign in."),
    SignupLinkUnavailable => ("Link này không còn khả dụng; có thể đã hết hạn hoặc đã được sử dụng. Bạn có thể yêu cầu email mới. Nếu đã có tài khoản, hãy trở lại Đăng nhập.", "This link is unavailable; it may have expired or already been used. You can request another email. If you already have an account, return to sign in."),
    SignupVerificationUnknown => ("Chưa xác nhận được kết quả xử lý link. Hãy kiểm tra email thiết lập tài khoản trước. Không tự gửi lại bước xác nhận; nếu cần, mở lại link email hoặc yêu cầu link mới.", "The link outcome could not be confirmed. Check for the account setup email first. This confirmation is not retried automatically; if needed, reopen the email link or request a new one."),
    SignupVerificationUnavailable => ("Chưa hoàn tất được bước xác minh lúc này. Hãy kiểm tra email trước, sau đó mở lại link hoặc yêu cầu email mới.", "Verification could not be completed right now. Check your email first, then reopen the link or request another email."),
    SignupCaptureFailed => ("Chưa đọc được link xác minh an toàn. Hãy mở lại link từ email; nếu vẫn lỗi, bạn có thể yêu cầu email mới.", "The verification link could not be read safely. Reopen it from your email; if the problem continues, you can request another email."),
    SignupHistoryBlocked => ("Trình duyệt chưa xóa được link xác minh khỏi thanh địa chỉ. Đăng ký đang tạm dừng để bảo vệ link. Thử xóa link an toàn trước khi tiếp tục; nếu vẫn lỗi, hãy đóng thẻ này.", "The browser could not remove the verification link from the address bar. Sign-up is paused to protect the link. Try clearing it safely before continuing; if that fails, close this tab."),
    SignupClearLink => ("Xóa link an toàn và bắt đầu lại", "Clear link safely and start again"),
    SignupNewRequest => ("Yêu cầu email mới", "Request another email"),
    SignupCancel => ("Hủy và về Đăng nhập", "Cancel and return to sign-in"),
    SignupCancelHelp => ("Rời bước này sẽ xóa email và link khỏi phiên trình duyệt. Việc hủy không thu hồi bước thiết lập mà dịch vụ tài khoản đã bắt đầu.", "Leaving clears the email and link from this browser session. Cancellation does not undo account setup that the account service has already started."),
    SignupFixtureNotice => ("Kiểm thử đăng ký cô lập: dữ liệu giả, không gửi email, không gọi dịch vụ tài khoản, không tạo tài khoản.", "Isolated sign-up fixtures: fake data, no email, no account-service calls and no account creation."),
    SignupFixtureLink => ("Fixture: mở link xác minh", "Fixture: open verification link"),
    SignupFixtureAccepted => ("Fixture: yêu cầu được tiếp nhận", "Fixture: request accepted"),
    SignupFixtureSetup => ("Fixture: dịch vụ nhận email thiết lập", "Fixture: setup email accepted"),
    SignupFixtureUnavailable => ("Fixture: link không khả dụng", "Fixture: link unavailable"),
    SignupFixtureUnknown => ("Fixture: mất phản hồi", "Fixture: response lost"),
    SignupFixtureInvalid => ("Fixture: email không hợp lệ", "Fixture: invalid email"),
    SignupServiceUnavailable => ("Chưa kiểm tra được khả năng tạo tài khoản. Vui lòng thử lại; không có tài khoản nào được tạo ở bước này.", "Account creation availability could not be checked. Please retry; no account was created by this step."),
    SignupInvalidResponse => ("Chưa xác nhận được khả năng tạo tài khoản từ phản hồi dịch vụ. Vui lòng thử lại.", "Account creation availability could not be confirmed from the service response. Please retry."),
    SignupRetry => ("Kiểm tra lại", "Check again"),
    RegistrationUnconfirmed => ("Chưa có xác nhận đăng ký tài khoản. Bạn có thể trở lại đăng nhập hoặc Trang chủ.", "Account registration has not been confirmed. You can return to sign-in or Home."),
    ReturnHome => ("Về Trang chủ", "Back to Home"),
    ReturnAccount => ("Về Tài khoản", "Back to Account"),
    OpenAccount => ("Mở Tài khoản", "Open Account"),
    ReturnMembership => ("Về Membership", "Back to Membership"),
    ReturnRegistration => ("Về đăng ký tài khoản", "Back to account registration"),
    ReturnLogin => ("Về Đăng nhập", "Back to sign-in"),
    AccountEyebrow => ("MỘT GÓC NGHE CỦA RIÊNG BẠN", "A LISTENING SPACE OF YOUR OWN"),
    AccountHeading => ("Góc của bạn.", "Your space."),
    GuestHeading => ("Chào người nghe.", "Welcome, listener."),
    GuestHelp => ("Bạn vẫn có thể khám phá nội dung công khai. Đăng nhập khi muốn mở dữ liệu thư viện của mình.", "You can still explore public stories. Sign in when you want to open your library data."),
    SignedHeading => ("Chào mừng bạn trở lại.", "Welcome back."),
    SignedHelp => ("Giữ những câu chuyện đã chọn, tìm lại những đoạn kể còn ngân.", "Return to the stories you saved and the passages that stay with you."),
    OpenLibrary => ("Mở thư viện", "Open my library"),
    MembershipName => ("Cantos Membership", "Cantos Membership"),
    MembershipHeading => ("Một người nghe. Một người đồng hành.", "A listener. A companion."),
    MembershipHelp => ("Tìm hiểu chương trình hội viên khi bạn sẵn sàng.", "Learn about membership whenever you are ready."),
    MembershipUnknown => ("Thông tin hội viên hiện chưa khả dụng. Đăng nhập không tự kích hoạt Membership.", "Membership details are currently unavailable. Signing in does not activate Membership."),
    MembershipLearn => ("Tìm hiểu Membership", "Explore Membership"),
    PreferencesHeading => ("Theo cách bạn muốn nghe", "The way you like to listen"),
    ThemeHelp => ("Chọn giao diện cho Cantos; thư viện và phiên nghe vẫn được giữ.", "Choose the Cantos appearance; your library and listener session are retained."),
    SettingsHelp => ("Cài đặt nghe và ngôn ngữ", "Listening settings and language"),
    SignedInActionHelp => ("Phiên nghe đã được xác nhận. Mở Tài khoản để xem thư viện hoặc đăng xuất.", "Your listener session is confirmed. Open Account to view your library or sign out."),
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantos_ui::{AccessibleName, DisabledReason};

    #[test]
    fn every_auth_resource_is_nonblank_in_both_languages_and_valid_for_named_controls() {
        for key in ALL {
            for locale in [Locale::Vietnamese, Locale::English] {
                let copy = text(*key, locale);
                assert!(AccessibleName::new(copy).is_ok(), "{key:?}");
                assert!(DisabledReason::new(copy).is_ok(), "{key:?}");
                assert!(!copy.contains("Cantos Theatre"));
            }
        }
    }
}
