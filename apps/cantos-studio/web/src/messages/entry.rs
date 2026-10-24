//! Login-only Studio resources; independent of Theatre authentication and membership copy.
use crate::entry::Entry;

#[derive(Clone, Copy)]
pub enum Key {
    LoginTitle,
    LoginHelp,
    Welcome,
    WelcomeHelp,
    Checking,
    Denied,
    DeniedHelp,
    Unavailable,
    UnavailableHelp,
    SignInUnavailable,
    SignInUnavailableHelp,
    Retained,
    RetainedHelp,
    Dashboard,
    Import,
    Back,
    Cancel,
    Retry,
    Session,
    MemoryHelp,
    Reauthenticate,
    CleanupPending,
}

pub fn title(entry: Entry) -> Key {
    match entry {
        Entry::Checking => Key::Checking,
        Entry::Denied => Key::Denied,
        Entry::Unavailable => Key::Unavailable,
        Entry::SignInUnavailable => Key::SignInUnavailable,
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
        Entry::SignInUnavailable => Key::SignInUnavailableHelp,
        Entry::RetainedOwner => Key::RetainedHelp,
        Entry::Workspace => Key::WelcomeHelp,
    }
}

pub fn text(key: Key, english: bool) -> &'static str {
    match (key, english) {
        (Key::CleanupPending,false)=>"Đang thu hồi phiên trước khi tiếp tục đăng nhập…",
        (Key::CleanupPending,true)=>"Revoking the previous session before sign-in continues…",
        (Key::MemoryHelp,false)=>"Bản nháp chưa lưu chỉ còn trong tab này.",
        (Key::MemoryHelp,true)=>"Unsaved drafts live only in this tab.",
        (Key::SignInUnavailable,false)=>"Đăng nhập Studio chưa khả dụng",
        (Key::SignInUnavailable,true)=>"Studio sign-in is not available",
        (Key::SignInUnavailableHelp,false)=>"Máy chủ chưa cấu hình đăng nhập Studio hoặc chưa có danh sách tài khoản được phép, và không có cách đăng nhập thay thế. Hãy liên hệ quản trị viên. Bản nháp đang giữ trong phiên này vẫn còn.",
        (Key::SignInUnavailableHelp,true)=>"The server has no Studio sign-in or no list of allowed accounts configured, and there is no alternative sign-in. Contact an administrator. Drafts retained in this session are still here.",
        (Key::Reauthenticate,false)=>"Xác thực lại cho thao tác nhạy cảm",
        (Key::Reauthenticate,true)=>"Authenticate again for sensitive actions",
        (Key::LoginTitle, false) => "Đăng nhập Cantos Studio",
        (Key::LoginTitle, true) => "Sign in to Cantos Studio",
        (Key::LoginHelp, false) => "Dành cho tài khoản đã được cấp quyền biên tập tại Cantos Studio.",
        (Key::LoginHelp, true) => "For accounts with editorial access to Cantos Studio.",
        (Key::Welcome, false) => "Nơi câu chuyện thành tiếng nói.",
        (Key::Welcome, true) => "Where stories find their voice.",
        (Key::WelcomeHelp, false) => "Một không gian yên tĩnh cho bản thảo, tác phẩm và audio bàn giao.",
        (Key::WelcomeHelp, true) => "A quiet space for manuscripts, works and delivered audio.",
        (Key::Checking, false) => "Đang kiểm tra phiên Studio…",
        (Key::Checking, true) => "Checking your Studio session…",
        (Key::Denied, false) => "Không vào được Studio bằng tài khoản này",
        (Key::Denied, true) => "This account cannot enter Studio",
        (Key::DeniedHelp, false) => "Yêu cầu bị từ chối. Hãy đăng nhập bằng tài khoản đã được cấp quyền Cantos Studio và mở Studio từ địa chỉ chính thức.",
        (Key::DeniedHelp, true) => "The request was refused. Sign in with an account that has Cantos Studio access, and open Studio from its official address.",
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
