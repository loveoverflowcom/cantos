//! Bilingual resources for the app-owned auth/account compositions.
use crate::messages::Locale;

macro_rules! resources {
    ($($(#[$meta:meta])* $key:ident => ($vi:literal, $en:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum CopyKey { $($(#[$meta])* $key),+ }
        pub(crate) fn text(key: CopyKey, locale: Locale) -> &'static str {
            match (key, locale) {
                $($(#[$meta])* (CopyKey::$key, Locale::Vietnamese) => $vi,
                  $(#[$meta])* (CopyKey::$key, Locale::English) => $en),+
            }
        }
        #[cfg(test)]
        const ALL: &[CopyKey] = &[$($(#[$meta])* CopyKey::$key),+];
    };
}

resources! {
    LoginEyebrow => ("CANTOS · MỘT LẦN TRỞ LẠI", "CANTOS · A PLACE TO RETURN"),
    LoginHeading => ("Câu chuyện đang đợi bạn.", "Your story is waiting."),
    LoginHelp => ("Đăng nhập bằng email và mật khẩu để trở lại thư viện của bạn. Nội dung công khai vẫn có thể khám phá từ Trang chủ.", "Sign in with your email and password to return to your library. Public stories are still available to explore from Home."),
    LoginEmail => ("Email", "Email"),
    LoginPassword => ("Mật khẩu", "Password"),
    LoginSubmit => ("Đăng nhập", "Sign in"),
    LoginSubmitting => ("Đang đăng nhập…", "Signing in…"),
    LoginRefusedCredentials => ("Email hoặc mật khẩu chưa đúng. Hãy kiểm tra rồi thử lại.", "The email or password is not right. Check them and try again."),
    LoginRefusedInvalid => ("Chưa gửi được: hãy nhập email hợp lệ và mật khẩu từ 8 đến 128 ký tự.", "Could not send: enter a valid email and a password of 8 to 128 characters."),
    LoginShowPassword => ("Hiện mật khẩu", "Show password"),
    LoginHidePassword => ("Ẩn mật khẩu", "Hide password"),
    AccountCreatedNotice => ("Tài khoản đã được tạo. Hãy đăng nhập bằng email và mật khẩu bạn vừa chọn. Nếu bạn đăng ký từ ứng dụng Cantos, hãy quay lại ứng dụng và đăng nhập ở đó.", "Your account was created. Sign in with the email and password you just chose. If you signed up in the Cantos app, go back to the app and sign in there."),
    CreateAccount => ("Tạo tài khoản Cantos", "Create a Cantos account"),
    BrandHeading => ("Nhắm mắt lại. Mở một thế giới.", "Close your eyes. Open a world."),
    BrandHelp => ("Có những câu chuyện chỉ cần lắng nghe. Có những miền tưởng tượng luôn chờ bạn.", "Some stories only ask you to listen. Worlds of imagination are waiting for you."),
    BrandGenres => ("Truyện · Podcast · Kịch truyền thanh", "Stories · Podcasts · Audio drama"),
    RegisterEyebrow => ("CANTOS · TÀI KHOẢN CỦA BẠN", "CANTOS · YOUR ACCOUNT"),
    RegisterHeading => ("Một nơi để trở lại.", "A place to return to."),
    RegisterHelp => ("Nhập email để nhận liên kết xác minh. Chính bạn chọn mật khẩu sau khi mở liên kết đó.", "Enter your email to receive a verification link. You choose your own password after opening that link."),
    SignupEmail => ("Email của bạn", "Your email"),
    SignupEmailHelp => ("Chúng tôi gửi một liên kết xác minh. Bước này chưa tạo tài khoản và chưa đăng nhập.", "We send one verification link. This step does not create an account or sign you in."),
    SignupSendEmail => ("Gửi email xác minh", "Send verification email"),
    SignupSubmitting => ("Đang gửi yêu cầu…", "Sending request…"),
    SignupEmailAccepted => ("Yêu cầu đã được tiếp nhận. Nếu email đủ điều kiện, bạn sẽ nhận được liên kết xác minh trong ít phút; hãy kiểm tra cả thư rác. Thông báo này giống nhau với mọi địa chỉ và chưa xác nhận tài khoản hay đăng nhập.", "Your request has been accepted. If the email is eligible, you will receive a verification link within minutes; check your spam folder too. This message is the same for every address and does not confirm an account or sign you in."),
    SignupInvalidEmail => ("Nhập một địa chỉ email hợp lệ.", "Enter a valid email address."),
    SignupRequestUnavailable => ("Chưa gửi được yêu cầu lúc này. Bạn có thể thử gửi lại sau.", "The request is unavailable right now. You can try sending it again later."),
    SignupRequestRateLimited => ("Bạn đã gửi quá nhiều yêu cầu. Hãy đợi một lúc rồi thử lại.", "Too many requests. Wait a while, then try again."),
    SignupRequestUnknown => ("Chưa xác nhận được kết quả gửi yêu cầu. Hãy kiểm tra email trước khi yêu cầu liên kết mới; thư có thể đến chậm.", "The request outcome could not be confirmed. Check your email before requesting a new link; delivery may be delayed."),
    SignupNotConfigured => ("Tạo tài khoản chưa mở trên trang này. Bạn vẫn có thể khám phá nội dung công khai.", "Account creation is not open on this site yet. You can still explore public stories."),
    SignupConfirmHelp => ("Liên kết đã được mở an toàn. Hãy chọn mật khẩu rồi xác nhận để tạo tài khoản. Chỉ mở liên kết thì chưa tạo gì và chưa đăng nhập.", "The link has been opened safely. Choose a password, then confirm to create your account. Opening the link alone creates nothing and does not sign you in."),
    SignupPassword => ("Mật khẩu mới", "New password"),
    SignupPasswordRepeat => ("Nhập lại mật khẩu", "Repeat the password"),
    SignupPasswordHelp => ("Từ 8 đến 128 ký tự. Khoảng trắng được giữ nguyên và mật khẩu không bao giờ được lưu trong trình duyệt.", "8 to 128 characters. Spaces are kept as typed and the password is never stored in the browser."),
    SignupPasswordShort => ("Mật khẩu cần ít nhất 8 ký tự.", "The password needs at least 8 characters."),
    SignupPasswordLong => ("Mật khẩu chỉ được tối đa 128 ký tự.", "The password can have at most 128 characters."),
    SignupPasswordMismatch => ("Hai mật khẩu chưa giống nhau.", "The two passwords do not match."),
    SignupConfirmCheckbox => ("Tôi là chủ hộp thư này và xác nhận tạo tài khoản Cantos với mật khẩu vừa nhập.", "I own this mailbox and confirm creating a Cantos account with the password I just entered."),
    SignupCreate => ("Tạo tài khoản", "Create account"),
    SignupCompleting => ("Đang tạo tài khoản…", "Creating your account…"),
    SignupRetryHelp => ("Chưa biết tài khoản đã được tạo hay chưa. Hãy gửi lại đúng yêu cầu cũ: không cần nhập lại và mật khẩu không bị đổi. Nếu rời trang, yêu cầu này sẽ mất; khi đó hãy thử đăng nhập bằng mật khẩu bạn đã chọn.", "We cannot tell whether the account was created. Send the same request again: nothing is retyped and the password is unchanged. If you leave this page the request is lost; then try signing in with the password you chose."),
    SignupRetryRateLimited => ("Có quá nhiều yêu cầu. Hãy đợi một lúc rồi gửi lại đúng yêu cầu cũ; mật khẩu không bị đổi.", "Too many requests. Wait a while, then send the same request again; the password is unchanged."),
    SignupRetrySame => ("Gửi lại đúng yêu cầu cũ", "Send the same request again"),
    SignupInvalidPassword => ("Mật khẩu chưa được chấp nhận. Chưa có gì được tạo; hãy chọn mật khẩu khác rồi xác nhận lại.", "The password was not accepted. Nothing was created; choose another password and confirm again."),
    SignupChoosePassword => ("Chọn mật khẩu khác", "Choose another password"),
    SignupLinkRefused => ("Liên kết này không còn dùng được: có thể đã hết hạn hoặc đã được dùng. Nếu bạn đã tạo tài khoản, hãy đăng nhập; nếu chưa, hãy yêu cầu email mới.", "This link can no longer be used: it may have expired or already been used. If you already created an account, sign in; otherwise request another email."),
    SignupForbidden => ("Trình duyệt này chưa được phép tạo tài khoản từ địa chỉ hiện tại. Hãy mở Cantos từ địa chỉ chính thức.", "This browser is not allowed to create an account from the current address. Open Cantos from its official address."),
    SignupCaptureFailed => ("Chưa đọc được liên kết xác minh an toàn. Hãy mở lại liên kết từ email; nếu vẫn lỗi, bạn có thể yêu cầu email mới.", "The verification link could not be read safely. Reopen it from your email; if the problem continues, you can request another email."),
    SignupHistoryBlocked => ("Trình duyệt chưa xóa được liên kết xác minh khỏi thanh địa chỉ. Đăng ký đang tạm dừng để bảo vệ liên kết. Thử xóa liên kết an toàn trước khi tiếp tục; nếu vẫn lỗi, hãy đóng thẻ này.", "The browser could not remove the verification link from the address bar. Sign-up is paused to protect the link. Try clearing it safely before continuing; if that fails, close this tab."),
    SignupClearLink => ("Xóa liên kết an toàn và bắt đầu lại", "Clear link safely and start again"),
    SignupNewRequest => ("Yêu cầu email mới", "Request another email"),
    #[cfg(feature = "preview")]
    SignupFixtureNotice => ("Kiểm thử đăng ký cô lập: dữ liệu giả, không gửi email, không gọi dịch vụ tài khoản, không tạo tài khoản.", "Isolated sign-up fixtures: fake data, no email, no account-service calls and no account creation."),
    #[cfg(feature = "preview")]
    SignupFixtureLink => ("Fixture: mở liên kết xác minh", "Fixture: open verification link"),
    #[cfg(feature = "preview")]
    SignupFixtureAccepted => ("Fixture: yêu cầu được tiếp nhận", "Fixture: request accepted"),
    #[cfg(feature = "preview")]
    SignupFixtureCompleted => ("Fixture: tài khoản đã tạo", "Fixture: account created"),
    #[cfg(feature = "preview")]
    SignupFixtureRefused => ("Fixture: liên kết bị từ chối", "Fixture: link refused"),
    #[cfg(feature = "preview")]
    SignupFixtureUnknown => ("Fixture: mất phản hồi", "Fixture: response lost"),
    #[cfg(feature = "preview")]
    SignupFixtureInvalid => ("Fixture: email không hợp lệ", "Fixture: invalid email"),
    ReturnHome => ("Về Trang chủ", "Back to Home"),
    ReturnAccount => ("Về Tài khoản", "Back to Account"),
    OpenAccount => ("Mở Tài khoản", "Open Account"),
    ReturnMembership => ("Về Membership", "Back to Membership"),
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

    /// ADR 0048 stage 0: a signup started in the app finishes by returning to it, never by
    /// treating this Web page as the app's session. The copy names that path in both locales.
    #[test]
    fn account_created_notice_names_the_return_to_the_app() {
        let vi = text(CopyKey::AccountCreatedNotice, Locale::Vietnamese);
        let en = text(CopyKey::AccountCreatedNotice, Locale::English);
        assert!(
            vi.contains("ứng dụng Cantos") && vi.contains("quay lại ứng dụng"),
            "{vi}"
        );
        assert!(
            en.contains("Cantos app") && en.contains("go back to the app"),
            "{en}"
        );
    }
}
