//! Typed resources for the mock only; Vietnamese content fixtures never change with UI locale.
use crate::model::{Genre, Route, Transport};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    Vietnamese,
    English,
}

impl Locale {
    pub const fn lang(self) -> &'static str {
        match self {
            Self::Vietnamese => "vi-VN",
            Self::English => "en",
        }
    }
}

macro_rules! resources {
    ($($key:ident => ($vi:literal, $en:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Key { $($key),+ }
        impl Key {
            pub const ALL: &'static [Self] = &[$(Self::$key),+];
        }
        pub fn text(key: Key, locale: Locale) -> &'static str {
            match (key, locale) {
                $((Key::$key, Locale::Vietnamese) => $vi,
                  (Key::$key, Locale::English) => $en),+
            }
        }
    };
}

resources! {
    PageTitle => ("{page} · Cantos", "{page} · Cantos"),
    SkipContent => ("Đến nội dung chính", "Skip to main content"),
    MainNavigation => ("Điều hướng chính", "Main navigation"),
    BrandCaption => ("VỌNG ĐÀI · MỘT THẾ GIỚI ĐỂ NGHE", "VỌNG ĐÀI · A WORLD TO LISTEN TO"),
    Discover => ("Khám phá", "Discover"),
    Explore => ("Tìm tác phẩm", "Find a story"),
    Library => ("Tủ sách của tôi", "My library"),
    Player => ("Đang nghe", "Now listening"),
    Queue => ("Tiếp theo", "Up next"),
    Settings => ("Cài đặt nghe", "Listening settings"),
    Account => ("Không gian của bạn", "Your space"),
    NotFound => ("Trang chưa có", "Page not found"),
    HomeHeading => ("Mở một miền tưởng tượng.", "Open a world of imagination."),
    HomeDeck => ("Những câu chuyện để lắng nghe, chậm lại và đi xa hơn.", "Stories to listen to, slow down with and journey further."),
    Quote => ("Nhắm mắt lại. Mở một thế giới.", "Close your eyes. Open a world."),
    DemoBadge => ("Bản mô phỏng · dữ liệu minh họa", "Mock demo · sample data"),
    DemoNotice => ("Mọi thao tác chỉ ở trong phiên này. Không có âm thanh, API, tài khoản hay đồng bộ. Tải lại trang sẽ đặt lại dữ liệu.", "Everything stays in this session. No audio, API, account or sync. Reloading resets the demo."),
    Featured => ("TUYỂN CHỌN CANTOS", "CANTOS SELECTION"),
    FeaturedSubtitle => ("Một tiếng chuông lay động sơn hà.", "A bell echoes across mountains and rivers."),
    Begin => ("Mở tập nghe mẫu", "Open a sample episode"),
    AboutWork => ("Về tác phẩm", "About the story"),
    BackDiscover => ("Về khám phá", "Back to discovery"),
    StoryShelf => ("Mỗi câu chuyện, một thế giới", "Every story, a world"),
    Saved => ("Đã lưu", "Saved"),
    Save => ("Lưu tác phẩm", "Save story"),
    SaveName => ("Lưu tác phẩm {title}", "Save story {title}"),
    RemoveSaveName => ("Bỏ lưu tác phẩm {title}", "Unsave story {title}"),
    Continue => ("Nghe tiếp", "Continue listening"),
    ContinueHeading => ("Câu chuyện còn dở", "Your unfinished story"),
    Bookmarks => ("Dấu trang", "Bookmarks"),
    Episodes => ("Các tập nghe mẫu", "Sample episodes"),
    Episode => ("Tập {number}", "Episode {number}"),
    EpisodeCountOne => ("{count} tập", "{count} episode"),
    EpisodeCountOther => ("{count} tập", "{count} episodes"),
    ContentLanguage => ("Tiếng Việt · nội dung minh họa", "Vietnamese · sample content"),
    Fantasy => ("Tiên hiệp", "Cultivation fantasy"),
    Memory => ("Dã sử", "Historical fiction"),
    Dream => ("Huyền ảo", "Fantasy"),
    All => ("Tất cả", "All stories"),
    CoverPlaceholder => ("Bìa minh họa bằng chữ · chưa có ảnh được cấp quyền", "Typographic cover · licensed artwork pending"),
    SearchLabel => ("Tìm theo tên hoặc mô tả", "Search titles or descriptions"),
    SearchHelp => ("Tìm cục bộ, không gửi dữ liệu. Phân biệt dấu tiếng Việt.", "Local search, no data sent. Vietnamese diacritics are significant."),
    Filters => ("Lọc thể loại", "Genre filters"),
    NoResults => ("Chưa tìm thấy câu chuyện", "No stories found"),
    NoResultsHelp => ("Thử tên có dấu hoặc bỏ bộ lọc để xem lại các tác phẩm mẫu.", "Try an accented title or clear filters to see the sample stories."),
    ClearSearch => ("Xóa tìm kiếm và bộ lọc", "Clear search and filters"),
    Loading => ("Đang tải danh mục mẫu…", "Loading the sample catalog…"),
    LoadingHelp => ("Trạng thái thử nghiệm giữ nguyên cho đến khi bạn chọn tiếp tục.", "This demo state stays visible until you choose to continue."),
    LoadDemo => ("Hiện dữ liệu mẫu", "Show sample data"),
    EmptyCatalog => ("Danh mục đang trống", "The catalog is empty"),
    EmptyCatalogHelp => ("Đây là trạng thái mẫu. Có thể trở lại dữ liệu minh họa bất kỳ lúc nào.", "This is a sample empty state. You can return to the demo data any time."),
    CatalogError => ("Chưa mở được danh mục mẫu", "The sample catalog could not be opened"),
    CatalogErrorHelp => ("Lỗi được mô phỏng, không có yêu cầu mạng. Vị trí nghe và tủ sách của bạn vẫn giữ nguyên.", "This is a simulated error, with no network request. Your position and library are unchanged."),
    Retry => ("Thử lại dữ liệu mẫu", "Retry sample data"),
    NoLibrary => ("Tủ sách chưa có nội dung", "Your library is empty"),
    NoLibraryHelp => ("Lưu tác phẩm, mở một tập mẫu hoặc thêm dấu trang để bắt đầu.", "Save a story, open a sample episode or add a bookmark to begin."),
    NoPlayer => ("Chưa chọn tập nghe", "No episode selected"),
    NoPlayerHelp => ("Chọn một tập mẫu từ tác phẩm. Demo không phát âm thanh.", "Choose a sample episode from a story. The demo has no audio."),
    MiniPlayer => ("Trình phát mô phỏng thu gọn", "Compact mock player"),
    FullPlayer => ("Mở trình phát mô phỏng", "Open mock player"),
    Play => ("Phát mô phỏng", "Simulate play"),
    Pause => ("Tạm dừng mô phỏng", "Pause simulation"),
    Replay => ("Mô phỏng từ đầu", "Simulate from the beginning"),
    PlayName => ("Mở tập nghe mẫu {title}", "Open a sample episode: {title}"),
    Paused => ("Mô phỏng · đang tạm dừng", "Mock · paused"),
    Simulating => ("Mô phỏng đang phát · không có âm thanh", "Simulating play · no audio"),
    Ended => ("Mô phỏng · đã hết tập", "Mock · episode ended"),
    Timeline => ("Vị trí mô phỏng", "Mock position"),
    TimelineHelp => ("Kéo hoặc dùng phím mũi tên. Không có đồng hồ phát; vị trí chỉ đổi khi bạn thao tác.", "Drag or use arrow keys. There is no playback clock; position changes only with your input."),
    SpokenTimeOf => ("{elapsed} trên tổng số {total}", "{elapsed} of {total}"),
    SpokenTime => ("{minutes}, {seconds}", "{minutes}, {seconds}"),
    MinuteOne => ("{count} phút", "{count} minute"),
    MinuteOther => ("{count} phút", "{count} minutes"),
    SecondOne => ("{count} giây", "{count} second"),
    SecondOther => ("{count} giây", "{count} seconds"),
    Back15 => ("Lùi 15 giây mô phỏng", "Move mock position back 15 seconds"),
    Forward15 => ("Tiến 15 giây mô phỏng", "Move mock position forward 15 seconds"),
    AddBookmark => ("Ghi dấu tại vị trí này", "Bookmark this position"),
    BookmarkAdded => ("Đã ghi dấu tại vị trí này", "This position is bookmarked"),
    RemoveBookmark => ("Xóa dấu trang", "Remove bookmark"),
    BookmarkName => ("Nghe tiếp {title} tại {time}", "Continue listening to {title} at {time}"),
    RemoveBookmarkName => ("Xóa dấu trang của {title} tại {time}", "Remove bookmark for {title} at {time}"),
    QueueAdd => ("Thêm vào tiếp theo", "Add to queue"),
    QueueAdded => ("Đã ở trong tiếp theo", "Already queued"),
    QueueName => ("Thêm vào tiếp theo: {title}", "Add to queue: {title}"),
    QueueEmpty => ("Chưa có tập tiếp theo", "Your queue is empty"),
    QueueEmptyHelp => ("Thêm tập mẫu từ danh sách tập. Demo không tự phát tập tiếp theo.", "Add a sample episode from a story. The demo never advances automatically."),
    QueueRemove => ("Bỏ khỏi tiếp theo", "Remove from queue"),
    QueueRemoveName => ("Bỏ khỏi tiếp theo: {title}", "Remove from queue: {title}"),
    Next => ("Mở tập tiếp theo", "Open next episode"),
    NoNext => ("Chưa có tập trong danh sách tiếp theo", "There are no episodes in your queue"),
    CloseQueue => ("Đóng tiếp theo, về trình phát", "Close queue and return to player"),
    CurrentEpisode => ("Tập mẫu đang chọn", "Selected sample episode"),
    Unavailable => ("Chưa có bản nghe mẫu", "No sample listening edition"),
    UnavailableHelp => ("Tập minh họa chưa có bản nghe; không thể mở hoặc thêm vào tiếp theo.", "This sample episode has no listening edition and cannot be opened or queued."),
    Speed => ("Tốc độ mô phỏng", "Mock speed"),
    SpeedHelp => ("Chỉ đổi giá trị giao diện; không điều khiển âm thanh.", "Changes the display value only; it does not control audio."),
    Theme => ("Giao diện", "Appearance"),
    System => ("Theo hệ thống", "System"),
    Light => ("Sáng", "Light"),
    Dark => ("Tối", "Dark"),
    Locale => ("Ngôn ngữ giao diện", "Interface language"),
    Scenario => ("Trạng thái danh mục thử nghiệm", "Catalog demo state"),
    Ready => ("Có dữ liệu", "Ready"),
    Empty => ("Trống", "Empty"),
    Error => ("Lỗi", "Error"),
    DemoTools => ("Điều khiển bản mô phỏng", "Demo controls"),
    Reset => ("Đặt lại toàn bộ dữ liệu mẫu", "Reset all sample data"),
    ResetHelp => ("Đặt lại vị trí, tác phẩm lưu, dấu trang, tiếp theo, tìm kiếm và tốc độ. Chỉ ảnh hưởng phiên demo này.", "Reset position, saved stories, bookmarks, queue, search and speed. Only this demo session is affected."),
    ClearLibrary => ("Thử tủ sách trống", "Try an empty library"),
    AccountHelp => ("Bạn đang dùng bản mô phỏng không có tài khoản. Lưu tác phẩm và dấu trang chỉ tồn tại trong bộ nhớ phiên này.", "You are using an account-free mock. Saved stories and bookmarks exist only in this session's memory."),
    SignIn => ("Đăng nhập", "Sign in"),
    Sync => ("Đồng bộ", "Sync"),
    Download => ("Tải để nghe ngoại tuyến", "Download for offline listening"),
    Timer => ("Hẹn giờ dừng", "Sleep timer"),
    Unsupported => ("Chưa được tích hợp trong bản mô phỏng", "Not integrated in this mock"),
    NotFoundHelp => ("Đường dẫn này không có trong demo. Dữ liệu phiên của bạn vẫn được giữ.", "This route is not in the demo. Your session data is unchanged."),
    Preferences => ("Thiết lập cho lần nghe tiếp theo", "Set the mood for your next story"),
    Footer => ("Cantos · khung giao diện nghe thử, chưa phải trải nghiệm nghe thực", "Cantos · listening UI mock, not a real listening experience"),
}

pub fn format(key: Key, locale: Locale, values: &[(&str, String)]) -> String {
    values.iter().fold(text(key, locale).to_owned(), |message, (name, value)| {
        message.replace(&format!("{{{name}}}"), value)
    })
}

pub fn episode_count(locale: Locale, count: usize) -> String {
    let key = match (locale, count) {
        (Locale::English, 1) => Key::EpisodeCountOne,
        _ => Key::EpisodeCountOther,
    };
    format(key, locale, &[("count", count.to_string())])
}

pub fn spoken_time(locale: Locale, position: u32) -> String {
    let minutes = position / 60;
    let seconds = position % 60;
    let minute_key = if locale == Locale::English && minutes == 1 { Key::MinuteOne } else { Key::MinuteOther };
    let second_key = if locale == Locale::English && seconds == 1 { Key::SecondOne } else { Key::SecondOther };
    format(Key::SpokenTime, locale, &[
        ("minutes", format(minute_key, locale, &[("count", minutes.to_string())])),
        ("seconds", format(second_key, locale, &[("count", seconds.to_string())])),
    ])
}

pub fn position_description(locale: Locale, position: u32, duration: u32) -> String {
    format(Key::SpokenTimeOf, locale, &[
        ("elapsed", spoken_time(locale, position)),
        ("total", spoken_time(locale, duration)),
    ])
}

pub fn page_title(locale: Locale, route: Route) -> String {
    let page = match route {
        Route::Work(id) => crate::model::work(id).title,
        route => text(route_key(route), locale),
    };
    format(Key::PageTitle, locale, &[("page", page.to_owned())])
}

pub fn speed_label(locale: Locale, speed: u16) -> String {
    let separator = if locale == Locale::Vietnamese { ',' } else { '.' };
    format!("{}{separator}{:02}×", speed / 100, speed % 100)
}

pub fn genre_key(genre: Genre) -> Key {
    match genre {
        Genre::All => Key::All,
        Genre::Fantasy => Key::Fantasy,
        Genre::Memory => Key::Memory,
        Genre::Dream => Key::Dream,
    }
}

pub fn route_key(route: Route) -> Key {
    match route {
        Route::Discover => Key::Discover,
        Route::Explore => Key::Explore,
        Route::Work(_) => Key::AboutWork,
        Route::Library => Key::Library,
        Route::Player => Key::Player,
        Route::Queue => Key::Queue,
        Route::Settings => Key::Settings,
        Route::Account => Key::Account,
        Route::NotFound => Key::NotFound,
    }
}

pub fn transport_key(transport: Transport) -> Key {
    match transport {
        Transport::Paused => Key::Paused,
        Transport::Simulating => Key::Simulating,
        Transport::Ended => Key::Ended,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn placeholders(value: &str) -> BTreeSet<&str> {
        value.split('{').skip(1).filter_map(|part| part.split_once('}').map(|pair| pair.0)).collect()
    }

    #[test]
    fn every_resource_has_both_locales_and_matching_parameters() {
        for key in Key::ALL {
            let vi = text(*key, Locale::Vietnamese);
            let en = text(*key, Locale::English);
            assert!(!vi.trim().is_empty() && !en.trim().is_empty(), "{key:?}");
            assert_eq!(placeholders(vi), placeholders(en), "{key:?}");
        }
        assert_eq!(episode_count(Locale::English, 1), "1 episode");
        assert_eq!(episode_count(Locale::English, 3), "3 episodes");
        assert_eq!(spoken_time(Locale::English, 61), "1 minute, 1 second");
        assert_eq!(speed_label(Locale::Vietnamese, 125), "1,25×");
    }
}
