//! Login-only Studio resources; independent of Theatre authentication and membership copy.
use crate::entry::Entry;

#[derive(Clone, Copy)]
pub enum Key {
    LoginTitle,
    LoginHelp,
    Welcome,
    WelcomeHelp,
    CantosId,
    OtherAccount,
    Checking,
    Denied,
    DeniedHelp,
    Unavailable,
    UnavailableHelp,
    Retained,
    RetainedHelp,
    Dashboard,
    Import,
    Back,
    Cancel,
    Retry,
    Session,
    BearerUnavailable,
    MemoryHelp,
    Reauthenticate,
    CleanupPending,
    AuthorizationComplete,
}

pub fn title(entry: Entry) -> Key {
    match entry {
        Entry::Checking => Key::Checking,
        Entry::Denied => Key::Denied,
        Entry::Unavailable => Key::Unavailable,
        Entry::RetainedOwner => Key::Retained,
        Entry::Login => Key::LoginTitle,
        Entry::Workspace => Key::Dashboard,
    }
}

pub fn help(entry: Entry) -> Key {
    match entry {
        Entry::Checking | Entry::Login => Key::LoginHelp,
        Entry::Denied => Key::DeniedHelp,
        Entry::Unavailable => Key::UnavailableHelp,
        Entry::RetainedOwner => Key::RetainedHelp,
        Entry::Workspace => Key::WelcomeHelp,
    }
}

pub fn text(key: Key, english: bool) -> &'static str {
    match (key, english) {
        (Key::CleanupPending,false)=>"Đang thu hồi phiên trước khi tiếp tục đăng nhập…",
        (Key::CleanupPending,true)=>"Revoking the previous session before sign-in continues…",
        (Key::Reauthenticate,false)=>"Xác thực lại cho thao tác nhạy cảm",
        (Key::Reauthenticate,true)=>"Authenticate again for sensitive actions",
        (Key::BearerUnavailable,false)=>"Đăng nhập cần dịch vụ Cantos ID được cấu hình. Chế độ token phát triển cũ không cấp phiên Web.",
        (Key::BearerUnavailable,true)=>"Sign-in requires configured Cantos ID. Legacy development tokens do not issue Web sessions.",
        (Key::MemoryHelp,false)=>"Cho phép cửa sổ đăng nhập để tiếp tục. Phiên chỉ giữ trong tab này; sau khi tải lại trang hoặc mở tab mới, hãy đăng nhập lại. Bản nháp đang sửa vẫn ở tab gốc khi xác thực.",
        (Key::MemoryHelp,true)=>"Allow the sign-in window to continue. Your session stays in this tab only; sign in again after reload or in a new tab. Drafts stay in the original tab during authentication.",
        (Key::AuthorizationComplete,false)=>"Quay lại tab Cantos Studio ban đầu. Nếu tab đã đóng hoặc tải lại, hãy đóng cửa sổ này và bắt đầu đăng nhập lại.",
        (Key::AuthorizationComplete,true)=>"Return to the original Cantos Studio tab. If it was closed or reloaded, close this window and start sign-in again.",
        (Key::LoginTitle, false) => "Đăng nhập Cantos Studio",
        (Key::LoginTitle, true) => "Sign in to Cantos Studio",
        (Key::LoginHelp, false) => "Dành cho tài khoản đã được cấp quyền biên tập tại Cantos Studio.",
        (Key::LoginHelp, true) => "For accounts with editorial access to Cantos Studio.",
        (Key::Welcome, false) => "Nơi câu chuyện thành tiếng nói.",
        (Key::Welcome, true) => "Where stories find their voice.",
        (Key::WelcomeHelp, false) => "Một không gian yên tĩnh cho bản thảo, tác phẩm và audio bàn giao.",
        (Key::WelcomeHelp, true) => "A quiet space for manuscripts, works and delivered audio.",
        (Key::CantosId, false) => "Đăng nhập với Cantos ID",
        (Key::CantosId, true) => "Sign in with Cantos ID",
        (Key::OtherAccount, false) => "Dùng tài khoản khác",
        (Key::OtherAccount, true) => "Use another account",
        (Key::Checking, false) => "Đang kiểm tra phiên Studio…",
        (Key::Checking, true) => "Checking your Studio session…",
        (Key::Denied, false) => "Tài khoản chưa có quyền Studio",
        (Key::Denied, true) => "This account does not have Studio access",
        (Key::DeniedHelp, false) => "Hãy đăng nhập bằng tài khoản đã được cấp quyền Cantos Studio.",
        (Key::DeniedHelp, true) => "Sign in with an account that has Cantos Studio access.",
        (Key::Unavailable, false) => "Chưa thể xác nhận phiên Studio",
        (Key::Unavailable, true) => "Your Studio session could not be confirmed",
        (Key::UnavailableHelp, false) => "Kiểm tra kết nối rồi thử lại. Bản nháp đang giữ trong phiên này vẫn còn.",
        (Key::UnavailableHelp, true) => "Check your connection and try again. Drafts retained in this session are still here.",
        (Key::Retained, false) => "Bản nháp thuộc tài khoản trước",
        (Key::Retained, true) => "Your draft belongs to the previous account",
        (Key::RetainedHelp, false) => "Đăng nhập lại bằng tài khoản đã mở bản nháp để tiếp tục. Nội dung riêng vẫn được ẩn.",
        (Key::RetainedHelp, true) => "Sign in with the account that opened the draft to continue. Private content stays hidden.",
        (Key::Dashboard, false) => "Bàn viết",
        (Key::Dashboard, true) => "Writing desk",
        (Key::Import, false) => "Nhập bản thảo",
        (Key::Import, true) => "Import manuscript",
        (Key::Back, false) => "Về Bàn viết",
        (Key::Back, true) => "Back to writing desk",
        (Key::Cancel, false) => "Về đăng nhập Studio",
        (Key::Cancel, true) => "Return to Studio sign-in",
        (Key::Retry, false) => "Thử lại",
        (Key::Retry, true) => "Try again",
        (Key::Session, false) => "Tài khoản Studio",
        (Key::Session, true) => "Studio account",
    }
}
