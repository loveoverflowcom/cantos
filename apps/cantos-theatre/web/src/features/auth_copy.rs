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
    RegisterHelp => ("Tài khoản Cantos dùng Cantos ID. Thông tin đăng ký được nhập tại bước xác thực khi dịch vụ hỗ trợ.", "Your Cantos account uses Cantos ID. Registration details belong in the identity service when it supports sign-up."),
    SignupUnavailable => ("Tạo tài khoản chưa khả dụng. Cantos ID chưa có đường dẫn đăng ký được xác nhận; Cantos không thu email hoặc mật khẩu tại đây.", "Account creation is unavailable. No supported Cantos ID registration entry has been confirmed; Cantos does not collect email or passwords here."),
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
