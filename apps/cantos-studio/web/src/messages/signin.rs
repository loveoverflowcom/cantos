//! Studio email/password sign-in resources. vi-VN and en are exhaustive. No message names the
//! failing factor of a login: the API never says, and the UI does not guess.
use crate::signin::SignInFailure;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Email,
    Password,
    Submit,
    Submitting,
    Help,
    ReauthenticateHelp,
    Incomplete,
    InvalidInput,
    InvalidCredentials,
    NotPermitted,
    NotAvailable,
    RateLimited,
    Unavailable,
    AccountMismatch,
    CleanupPending,
    Superseded,
    SessionNotStored,
}

pub const ALL: [Key; 17] = [
    Key::Email,
    Key::Password,
    Key::Submit,
    Key::Submitting,
    Key::Help,
    Key::ReauthenticateHelp,
    Key::Incomplete,
    Key::InvalidInput,
    Key::InvalidCredentials,
    Key::NotPermitted,
    Key::NotAvailable,
    Key::RateLimited,
    Key::Unavailable,
    Key::AccountMismatch,
    Key::CleanupPending,
    Key::Superseded,
    Key::SessionNotStored,
];

pub fn text(key: Key, english: bool) -> &'static str {
    match (key, english) {
        (Key::Email, false) => "Email",
        (Key::Email, true) => "Email",
        (Key::Password, false) => "Mật khẩu",
        (Key::Password, true) => "Password",
        (Key::Submit, false) => "Đăng nhập",
        (Key::Submit, true) => "Sign in",
        (Key::Submitting, false) => "Đang đăng nhập…",
        (Key::Submitting, true) => "Signing in…",
        (Key::Help, false) => "Đăng nhập bằng email và mật khẩu của tài khoản Cantos. Studio không có đăng ký công khai; quyền vào Studio do quản trị viên cấp.",
        (Key::Help, true) => "Sign in with your Cantos account email and password. Studio has no public sign-up; Studio access is granted by an administrator.",
        (Key::ReauthenticateHelp, false) => "Nhập lại email và mật khẩu của chính tài khoản đang mở bản nháp. Tài khoản khác sẽ bị từ chối và phiên mới bị thu hồi.",
        (Key::ReauthenticateHelp, true) => "Enter the email and password of the account that opened the current drafts. Another account is refused and its new session is revoked.",
        (Key::Incomplete, false) => "Nhập email và mật khẩu.",
        (Key::Incomplete, true) => "Enter your email and password.",
        (Key::InvalidInput, false) => "Email hoặc mật khẩu không đúng định dạng. Kiểm tra rồi thử lại.",
        (Key::InvalidInput, true) => "The email or password is not in a valid format. Check it and try again.",
        (Key::InvalidCredentials, false) => "Không đăng nhập được. Kiểm tra email và mật khẩu rồi thử lại.",
        (Key::InvalidCredentials, true) => "Could not sign in. Check your email and password and try again.",
        (Key::NotPermitted, false) => "Mật khẩu đúng nhưng tài khoản này chưa được phép vào Cantos Studio. Hãy liên hệ quản trị viên hoặc dùng tài khoản khác.",
        (Key::NotPermitted, true) => "Your password is correct, but this account is not allowed into Cantos Studio. Contact an administrator or use another account.",
        (Key::NotAvailable, false) => "Đăng nhập Studio chưa khả dụng trên máy chủ này và không có cách đăng nhập thay thế. Hãy liên hệ quản trị viên.",
        (Key::NotAvailable, true) => "Studio sign-in is not available on this server and there is no alternative sign-in. Contact an administrator.",
        (Key::RateLimited, false) => "Quá nhiều lần thử. Hãy chờ {seconds} giây rồi thử lại.",
        (Key::RateLimited, true) => "Too many attempts. Wait {seconds} seconds and try again.",
        (Key::Unavailable, false) => "Dịch vụ tài khoản chưa phản hồi nên chưa biết đăng nhập đã thành công hay chưa. Hãy kiểm tra phiên hiện tại hoặc thử lại sau. Bản nháp trong tab này vẫn được giữ.",
        (Key::Unavailable, true) => "The account service did not answer, so it is unknown whether sign-in completed. Check the current session or try again later. Drafts in this tab are kept.",
        (Key::AccountMismatch, false) => "Đây không phải tài khoản đã mở bản nháp đang giữ. Phiên vừa tạo đã bị thu hồi; hãy đăng nhập đúng tài khoản ban đầu.",
        (Key::AccountMismatch, true) => "This is not the account that opened the retained draft. The new session was revoked; sign in as the original account.",
        (Key::CleanupPending, false) => "Phiên trước vẫn đang được thu hồi. Hãy thử lại sau giây lát.",
        (Key::CleanupPending, true) => "The previous session is still being revoked. Try again in a moment.",
        (Key::Superseded, false) => "Phiên đã thay đổi trong lúc đăng nhập (có thể ở tab khác). Hãy kiểm tra phiên rồi thử lại.",
        (Key::Superseded, true) => "The session changed while signing in (possibly in another tab). Check the session and try again.",
        (Key::SessionNotStored, false) => "Đã xác thực nhưng trình duyệt chưa giữ phiên Studio. Kiểm tra cài đặt cookie của trang rồi thử lại.",
        (Key::SessionNotStored, true) => "You were authenticated, but the browser did not keep the Studio session. Check the site's cookie settings and try again.",
    }
}

pub fn failure_key(failure: SignInFailure) -> Key {
    match failure {
        SignInFailure::Incomplete => Key::Incomplete,
        SignInFailure::InvalidInput => Key::InvalidInput,
        SignInFailure::InvalidCredentials => Key::InvalidCredentials,
        SignInFailure::NotPermitted => Key::NotPermitted,
        SignInFailure::NotAvailable => Key::NotAvailable,
        SignInFailure::RateLimited(_) => Key::RateLimited,
        SignInFailure::Unavailable => Key::Unavailable,
        SignInFailure::AccountMismatch => Key::AccountMismatch,
        SignInFailure::CleanupPending => Key::CleanupPending,
        SignInFailure::Superseded => Key::Superseded,
        SignInFailure::SessionNotStored => Key::SessionNotStored,
    }
}

/// The message for a failed attempt, with the bounded Retry-After seconds where there is one.
pub fn failure_text(failure: SignInFailure, english: bool) -> String {
    let message = text(failure_key(failure), english);
    match failure {
        SignInFailure::RateLimited(seconds) => message.replace("{seconds}", &seconds.to_string()),
        _ => message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_resource_is_nonblank_and_distinct_failures_read_differently() {
        for key in ALL {
            for english in [false, true] {
                assert!(!text(key, english).trim().is_empty(), "{key:?}");
            }
        }
        let failures = [
            SignInFailure::Incomplete,
            SignInFailure::InvalidInput,
            SignInFailure::InvalidCredentials,
            SignInFailure::NotPermitted,
            SignInFailure::NotAvailable,
            SignInFailure::RateLimited(30),
            SignInFailure::Unavailable,
            SignInFailure::AccountMismatch,
            SignInFailure::CleanupPending,
            SignInFailure::Superseded,
            SignInFailure::SessionNotStored,
        ];
        for english in [false, true] {
            let messages: Vec<_> = failures.iter().map(|f| failure_text(*f, english)).collect();
            for (index, message) in messages.iter().enumerate() {
                assert!(!messages[..index].contains(message), "{message}");
            }
        }
    }

    #[test]
    fn the_roster_denial_is_the_only_message_that_says_the_password_was_right() {
        for failure in [
            SignInFailure::InvalidCredentials,
            SignInFailure::InvalidInput,
            SignInFailure::Unavailable,
            SignInFailure::NotAvailable,
        ] {
            for english in [false, true] {
                let message = failure_text(failure, english).to_lowercase();
                assert!(!message.contains("mật khẩu đúng"), "{failure:?}");
                assert!(!message.contains("password is correct"), "{failure:?}");
                // Never names which of password or account failed.
                assert!(!message.contains("không tồn tại"), "{failure:?}");
                assert!(!message.contains("wrong password"), "{failure:?}");
                assert!(!message.contains("no such account"), "{failure:?}");
            }
        }
        assert!(failure_text(SignInFailure::NotPermitted, false).contains("Mật khẩu đúng"));
        assert!(failure_text(SignInFailure::NotPermitted, true).contains("password is correct"));
    }

    #[test]
    fn retry_after_seconds_are_shown_and_the_placeholder_never_leaks() {
        assert_eq!(
            failure_text(SignInFailure::RateLimited(42), true),
            "Too many attempts. Wait 42 seconds and try again."
        );
        assert!(failure_text(SignInFailure::RateLimited(42), false).contains("42 giây"));
        for key in ALL {
            if key != Key::RateLimited {
                assert!(!text(key, true).contains("{seconds}"));
                assert!(!text(key, false).contains("{seconds}"));
            }
        }
    }
}
