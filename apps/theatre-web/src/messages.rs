use crate::navigation::{DemoState, Route};

#[derive(Clone, Copy)]
pub enum Key {
    Skip,
    Navigation,
    Discover,
    Explore,
    Library,
    Work,
    Player,
    NotFound,
    Tagline,
    Demo,
    MissingArt,
    Genre,
    Detail,
    PlayerOpen,
    Featured,
    Stories,
    Synopsis,
    Episodes,
    Episode,
    PlaybackUnavailable,
    Play,
    Save,
    Download,
    Search,
    SearchHint,
    LibraryUnavailable,
    Back,
    Theme,
    System,
    Light,
    Dark,
    DemoStates,
    Ready,
    Loading,
    Empty,
    Error,
    NotImplemented,
    Reset,
    Mini,
}

pub const fn text(key: Key, english: bool) -> &'static str {
    let pair = match key {
        Key::Skip => ("Đi tới nội dung", "Skip to content"),
        Key::Navigation => ("Điều hướng Theatre", "Theatre navigation"),
        Key::Discover => ("Khám phá", "Discover"),
        Key::Explore => ("Tìm tác phẩm", "Find a story"),
        Key::Library => ("Tủ sách", "Library"),
        Key::Work => ("Về tác phẩm", "About the work"),
        Key::Player => ("Phòng nghe", "Listening room"),
        Key::NotFound => ("Không tìm thấy trang", "Page not found"),
        Key::Tagline => ("Mở một miền tưởng tượng.", "Open a world of imagination."),
        Key::Demo => ("Bản khung · dữ liệu minh họa · không phát âm thanh", "Skeleton · demo content · no audio playback"),
        Key::MissingArt => ("Bìa minh họa chưa được cấp quyền sử dụng", "Cover artwork awaits usage permission"),
        Key::Genre => ("Những miền thanh âm", "Worlds of sound"),
        Key::Detail => ("Khám phá câu chuyện mẫu", "Explore the demo story"),
        Key::PlayerOpen => ("Mở phòng nghe mẫu", "Open the demo listening room"),
        Key::Featured => ("Tuyển chọn Cantos · mẫu", "Cantos selection · demo"),
        Key::Stories => ("Mỗi câu chuyện, một thế giới", "Every story, a world"),
        Key::Synopsis => ("Một tiếng chuông lay động sơn hà. Một hành trình tìm lại điều đã lãng quên.", "A bell echoes across the mountains. A journey to recover what was forgotten."),
        Key::Episodes => ("Các tập minh họa", "Demo episodes"),
        Key::Episode => ("Người giữ tiếng chuông", "The keeper of the bell"),
        Key::PlaybackUnavailable => ("Chưa có bản phát hành hoặc audio adapter. Phát, tua, dấu trang và tải xuống chưa được triển khai.", "No publication or audio adapter is connected. Playback, seeking, bookmarks and downloads are not implemented."),
        Key::Play => ("Phát", "Play"),
        Key::Save => ("Lưu vào tủ sách", "Save to library"),
        Key::Download => ("Tải xuống", "Download"),
        Key::Search => ("Tìm trong dữ liệu minh họa", "Search demo content"),
        Key::SearchHint => ("Tìm theo tên có dấu; chỉ lọc dữ liệu mẫu trên thiết bị.", "Search by title with diacritics; filters local demo content only."),
        Key::LibraryUnavailable => ("Tủ sách chưa kết nối tài khoản. Chưa có lịch sử nghe, tác phẩm đã lưu hoặc tệp tải xuống.", "The library has no account connection. Listening history, saved works and downloads are not available."),
        Key::Back => ("Về khám phá", "Back to discovery"),
        Key::Theme => ("Giao diện", "Theme"),
        Key::System => ("Theo hệ thống", "System"),
        Key::Light => ("Sáng", "Light"),
        Key::Dark => ("Tối", "Dark"),
        Key::DemoStates => ("Trạng thái minh họa", "Demo states"),
        Key::Ready => ("Nội dung mẫu", "Demo content"),
        Key::Loading => ("Đang tải · trạng thái minh họa", "Loading · demo state"),
        Key::Empty => ("Không có kết quả trong dữ liệu mẫu", "No results in demo content"),
        Key::Error => ("Không thể tải · lỗi minh họa, chưa có yêu cầu mạng", "Could not load · demo error, no network request was made"),
        Key::NotImplemented => ("Chưa triển khai kết nối dữ liệu Theatre", "Theatre data connection is not implemented"),
        Key::Reset => ("Trở lại dữ liệu mẫu", "Return to demo content"),
        Key::Mini => ("Trình phát thu gọn · chưa kết nối", "Mini-player · not connected"),
    };
    if english {
        pair.1
    } else {
        pair.0
    }
}

pub const fn route_key(route: Route) -> Key {
    match route {
        Route::Discover => Key::Discover,
        Route::Explore => Key::Explore,
        Route::Library => Key::Library,
        Route::Work => Key::Work,
        Route::Player => Key::Player,
        Route::NotFound => Key::NotFound,
    }
}

pub const fn state_key(state: DemoState) -> Key {
    match state {
        DemoState::Ready => Key::Ready,
        DemoState::Loading => Key::Loading,
        DemoState::Empty => Key::Empty,
        DemoState::Error => Key::Error,
        DemoState::NotImplemented => Key::NotImplemented,
    }
}
