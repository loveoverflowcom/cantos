//! Typed resources for live and mock listener surfaces; story fixtures retain their language.
use crate::{
    app::routes::Route,
    core::{
        catalog::Genre,
        playback::{PauseCause, Status},
    },
};

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
    Library => ("Của tôi", "My library"),
    History => ("Lịch sử", "History"),
    LibraryDeck => ("Những câu chuyện bạn muốn trở lại.", "Stories you want to return to."),
    Player => ("Đang nghe", "Now listening"),
    Queue => ("Tiếp theo", "Up next"),
    Settings => ("Cài đặt nghe", "Listening settings"),
    Account => ("Tài khoản", "Account"),
    MembershipPreview => ("Membership · bản xem thử", "Membership · preview"),
    MembershipRegister => ("Đăng ký Membership", "Register Membership interest"),
    MembershipUnavailable => ("Đăng ký Membership chưa khả dụng. Chưa có dịch vụ tiếp nhận được chấp nhận; thông tin không được gửi.", "Membership registration is unavailable. No accepted intake service exists; information is not sent."),
    NotFound => ("Trang chưa có", "Page not found"),
    HomeHeading => ("Mở một miền tưởng tượng.", "Open a world of imagination."),
    HomeDeck => ("Những câu chuyện để lắng nghe, chậm lại và đi xa hơn.", "Stories to listen to, slow down with and journey further."),
    Quote => ("Nhắm mắt lại. Mở một thế giới.", "Close your eyes. Open a world."),
    DemoBadge => ("Bản mô phỏng · dữ liệu minh họa", "Mock demo · sample data"),
    DemoNotice => ("Bản mô phỏng: không có âm thanh, tài khoản hay đồng bộ. Dữ liệu chỉ lưu trong phiên này; tải lại trang sẽ đặt lại.", "Mock demo: no audio, account or sync. Data lasts only for this session; reloading resets it."),
    Featured => ("TUYỂN CHỌN CANTOS", "CANTOS SELECTION"),
    FeaturedSubtitle => ("Một tiếng chuông lay động sơn hà.", "A bell echoes across mountains and rivers."),
    Begin => ("Mở tập nghe mẫu", "Open a sample episode"),
    AboutWork => ("Về tác phẩm", "About the story"),
    BackDiscover => ("Về khám phá", "Back to discovery"),
    StoryShelf => ("Mỗi câu chuyện, một thế giới", "Every story, a world"),
    Saved => ("Đã lưu", "Saved"),
    Save => ("Lưu", "Save"),
    RemoveSave => ("Bỏ lưu", "Unsave"),
    SaveName => ("Lưu tác phẩm {title}", "Save story {title}"),
    RemoveSaveName => ("Bỏ lưu tác phẩm {title}", "Unsave story {title}"),
    Continue => ("Nghe tiếp", "Continue listening"),
    ContinueHeading => ("Nghe tiếp", "Continue listening"),
    ResumeSample => ("Mở vị trí nghe", "Open listening position"),
    ResumeSampleName => ("Mở vị trí nghe {title} tại {time}", "Open listening position for {title} at {time}"),
    ResumeSampleHelp => ("Mở lại đúng vị trí và giữ tạm dừng. Chọn Phát mô phỏng để thử tiếp.", "Reopens the position paused. Choose Simulate play to try it."),
    NoContinue => ("Chưa có gì để nghe tiếp", "Nothing to pick up yet"),
    NoContinueHelp => ("Mở một tập, rồi quay lại đây để mở lại đúng vị trí trong phiên này.", "Open an episode, then return here to reopen the same position during this session."),
    NoSaved => ("Chưa lưu tác phẩm nào", "No saved stories yet"),
    NoSavedHelp => ("Chọn Lưu trên tác phẩm bạn thích để thấy chúng ở đây.", "Choose Save on a story you like and it will appear here."),
    NoHistory => ("Chưa có lịch sử nghe", "No listening history yet"),
    NoHistoryHelp => ("Lịch sử ghi nhận các tập đã Phát mô phỏng trong phiên này; chỉ mở trang hoặc lưu truyện chưa tạo lịch sử.", "History records episodes after Simulate play in this session; opening a page or saving a story does not add history."),
    HistoryHelp => ("Các câu chuyện đã phát mô phỏng, gần nhất ở trên. Mở lại sẽ khôi phục vị trí và giữ tạm dừng.", "Stories played in the simulation, most recent first. Reopening restores the position and stays paused."),
    BrowseStories => ("Khám phá tác phẩm", "Discover stories"),
    ViewAll => ("Xem tất cả", "View all"),
    ViewAllHistory => ("Xem tất cả lịch sử nghe", "View all listening history"),
    BackLibrary => ("Về Của tôi", "Back to My library"),
    ClearHistory => ("Xóa lịch sử nghe mẫu", "Clear sample listening history"),
    ClearHistoryConfirmTitle => ("Xóa lịch sử trong phiên này?", "Clear history for this session?"),
    ClearHistoryHelp => ("Chỉ xóa danh sách lịch sử. Vị trí nghe, truyện đã lưu, dấu trang và trình phát vẫn được giữ lại.", "Only the history list is cleared. Listening positions, saved stories, bookmarks and the player are retained."),
    ConfirmClearHistory => ("Xác nhận xóa lịch sử", "Confirm clear history"),
    Cancel => ("Hủy", "Cancel"),
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
    PlayerLoading => ("Mô phỏng · đang chờ bộ máy giả xác nhận", "Mock · waiting for fake-engine confirmation"),
    Buffering => ("Mô phỏng · đang đệm, không có âm thanh", "Mock · buffering, no audio"),
    Interrupted => ("Mô phỏng · dừng sau gián đoạn", "Mock · paused after interruption"),
    ResumeOffered => ("Mô phỏng · có vị trí để nghe tiếp, chưa phát", "Mock · resume position offered, playback has not started"),
    MockEngineTools => ("Thử bộ máy giả", "Fake-engine demo controls"),
    MockBuffer => ("Mô phỏng đệm", "Simulate buffering"),
    MockReady => ("Xác nhận phát mô phỏng", "Confirm simulated play"),
    MockInterrupt => ("Mô phỏng gián đoạn", "Simulate interruption"),
    MockEndInterrupt => ("Kết thúc gián đoạn, giữ tạm dừng", "End interruption and stay paused"),
    MockEnd => ("Mô phỏng kết thúc tập", "Simulate episode end"),
    MockRestore => ("Khôi phục vị trí trong phiên", "Restore position within this session"),
    MockRejected => ("Thao tác mẫu chưa được chấp nhận. Kiểm tra trạng thái mô phỏng rồi thử lại.", "The sample action was not accepted. Check the mock state and try again."),
    MockEngineHelp => ("Chỉ thử sự kiện giả. Phát mô phỏng trước khi thử đệm; kết thúc gián đoạn trước khi phát lại. Không có âm thanh, cuộc gọi thật, lưu trữ hay tự nghe tiếp.", "Fake events only. Simulate play before buffering; end an interruption before playing again. No sound, actual phone interruption, storage or automatic resume."),
    Timeline => ("Vị trí mô phỏng", "Mock position"),
    TimelineHelp => ("Kéo để xem vị trí, thả để xác nhận. Escape hủy kéo. Dùng phím mũi tên, Home hoặc End để đổi vị trí. Không có âm thanh hay đồng hồ phát.", "Drag to preview, release to commit. Escape cancels a drag. Use arrow keys, Home or End to seek. No sound or playback clock."),
    ScrubPreview => ("Đang xem trước vị trí mô phỏng", "Previewing mock position"),
    SeekPending => ("Đang xác nhận vị trí mô phỏng", "Confirming mock position"),
    TransportEpisode => ("{action}: {episode}", "{action}: {episode}"),
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
    ResetHelp => ("Đặt lại vị trí, tác phẩm lưu, lịch sử, dấu trang, tiếp theo và tốc độ. Xóa tủ sách sẽ xóa tác phẩm lưu, lịch sử, dấu trang và vị trí; vẫn giữ trình phát và tiếp theo. Chỉ ảnh hưởng phiên demo này.", "Reset position, saved stories, history, bookmarks, queue and speed. Clearing the library removes saved stories, history, bookmarks and progress; the player and queue are retained. Only this demo session is affected."),
    ClearLibrary => ("Xóa nội dung tủ sách mẫu", "Clear sample library contents"),
    GuestListener => ("Người nghe khách", "Guest listener"),
    GuestStatus => ("Chưa đăng nhập. Bạn đang nghe với tư cách khách.", "Not signed in. You are listening as a guest."),
    AccountStatusHeading => ("Trạng thái", "Status"),
    AccountShortcutsHeading => ("Đi tới", "Go to"),
    NotIntegrated => ("Chưa tích hợp", "Not integrated"),
    SignInHelp => ("Bản mô phỏng chưa có đăng nhập, nên chưa có tài khoản để gắn dữ liệu.", "This mock has no sign-in, so there is no account to attach data to."),
    SyncHelp => ("Chưa đồng bộ giữa thiết bị. Tiến trình, tác phẩm đã lưu và dấu trang chỉ nằm trong phiên này.", "Nothing syncs between devices. Progress, saved stories and bookmarks stay in this session only."),
    StorageLabel => ("Nơi lưu dữ liệu", "Where data lives"),
    StorageValue => ("Chỉ trong phiên này", "This session only"),
    StorageHelp => ("Tải lại trang sẽ đặt lại dữ liệu mẫu.", "Reloading the page resets the sample data."),
    SettingsLinkHelp => ("Giao diện, ngôn ngữ và tốc độ nghe.", "Appearance, language and listening speed."),
    LibraryLinkHelp => ("Nghe tiếp, đã lưu và lịch sử.", "Continue, saved and history."),
    ProgressName => ("Đã nghe {time} trên {total}", "Listened {time} of {total}"),
    CoverArtPlaceholder => ("Tranh minh họa · bản mô phỏng, chưa xác minh quyền sử dụng", "Illustration · mock, usage rights not verified"),
    SignIn => ("Đăng nhập", "Sign in"),
    SignOut => ("Đăng xuất", "Sign out"),
    AccountRefresh => ("Cập nhật phiên và dữ liệu", "Refresh session and data"),
    AccountChecking => ("Đang kiểm tra phiên người nghe…", "Checking your listener session…"),
    SignedInListener => ("Người nghe đã đăng nhập", "Signed-in listener"),
    LiveGuestStatus => ("Chưa đăng nhập vào Cantos.", "Not signed in to Cantos."),
    AccountReference => ("Mã tài khoản: {actor}", "Account ID: {actor}"),
    AccountLogoutPending => ("Đang đăng xuất phiên nghe…", "Signing out the listener session…"),
    AccountLogoutRetryHelp => ("Chưa xác nhận đăng xuất. Dữ liệu riêng đã được ẩn; hãy thử hoàn tất đăng xuất trước khi đăng nhập lại.", "Sign-out is not confirmed. Private data is hidden; finish signing out before signing in again."),
    AccountLogoutRetry => ("Hoàn tất đăng xuất", "Finish signing out"),
    AccountUnavailable => ("Chưa kiểm tra được phiên nghe. Hãy thử cập nhật lại.", "Your listener session could not be checked. Try refreshing again."),
    AccountProtocol => ("Phản hồi phiên nghe không hợp lệ. Dữ liệu riêng được giữ ẩn.", "The listener response was invalid. Private data stays hidden."),
    AccountNavigation => ("Chưa mở được trang đăng nhập. Hãy cập nhật phiên rồi thử lại.", "The sign-in page could not be opened. Refresh the session and try again."),
    AccountSessionChanged => ("Phiên nghe đã thay đổi ở cửa sổ khác. Hãy cập nhật để xem dữ liệu của tài khoản hiện tại.", "The listener session changed in another window. Refresh to view the current account's data."),
    LiveNotice => ("Tài khoản và dữ liệu người nghe được đọc từ Cantos. Đăng nhập không tự phát âm thanh hoặc chuyển dữ liệu minh họa vào tài khoản.", "Listener accounts and data are read from Cantos. Signing in does not start audio or import sample data."),
    LiveLibrarySignedOut => ("Đăng nhập để xem vị trí nghe tiếp, tác phẩm đã lưu và lịch sử của bạn.", "Sign in to view your listening positions, saved stories and history."),
    LiveLibraryLoading => ("Đang tải dữ liệu của phiên nghe hiện tại…", "Loading data for the current listener session…"),
    LiveLibraryEmpty => ("Tài khoản này chưa có dữ liệu nghe tiếp, tác phẩm đã lưu hoặc lịch sử.", "This account has no listening positions, saved stories or history yet."),
    LiveLibraryPageHelp => ("Mỗi phần hiển thị tối đa 20 mục trong trang đầu. Các vị trí đã lưu không tự phát âm thanh.", "Each section shows up to 20 items from its first page. Saved positions never start audio automatically."),
    LiveLibraryMore => ("Còn dữ liệu ngoài trang đang hiển thị.", "More data exists beyond the displayed page."),
    LivePosition => ("Vị trí {position} trên {total}", "Position {position} of {total}"),
    LivePositionNumber => ("Vị trí nghe tiếp {number}", "Listening position {number}"),
    LiveResumeExact => ("Vị trí thuộc đúng bản nghe đã lưu.", "The position belongs to the saved listening edition."),
    LiveResumeReplaced => ("Bản nghe đã được thay thế; vị trí cũ được giữ nguyên.", "The listening edition was replaced; the original position is retained."),
    LiveResumeUnavailable => ("Bản nghe hiện chưa khả dụng; vị trí của bạn vẫn được giữ.", "The listening edition is currently unavailable; your position is retained."),
    OpenMock => ("Mở bản xem thử minh họa riêng", "Open the isolated mock preview"),
    Sync => ("Đồng bộ", "Sync"),
    Download => ("Tải để nghe ngoại tuyến", "Download for offline listening"),
    Timer => ("Hẹn giờ dừng", "Sleep timer"),
    Unsupported => ("Chưa được tích hợp trong bản mô phỏng", "Not integrated in this mock"),
    NotFoundHelp => ("Đường dẫn này không có trong demo. Dữ liệu phiên của bạn vẫn được giữ.", "This route is not in the demo. Your session data is unchanged."),
    Preferences => ("Thiết lập cho lần nghe tiếp theo", "Set the mood for your next story"),
    Footer => ("Cantos · khung giao diện nghe thử, chưa phải trải nghiệm nghe thực", "Cantos · listening UI mock, not a real listening experience"),
}

pub fn format(key: Key, locale: Locale, values: &[(&str, String)]) -> String {
    values
        .iter()
        .fold(text(key, locale).to_owned(), |message, (name, value)| {
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
    let minute_key = if locale == Locale::English && minutes == 1 {
        Key::MinuteOne
    } else {
        Key::MinuteOther
    };
    let second_key = if locale == Locale::English && seconds == 1 {
        Key::SecondOne
    } else {
        Key::SecondOther
    };
    format(
        Key::SpokenTime,
        locale,
        &[
            (
                "minutes",
                format(minute_key, locale, &[("count", minutes.to_string())]),
            ),
            (
                "seconds",
                format(second_key, locale, &[("count", seconds.to_string())]),
            ),
        ],
    )
}

pub fn position_description(locale: Locale, position: u32, duration: u32) -> String {
    format(
        Key::SpokenTimeOf,
        locale,
        &[
            ("elapsed", spoken_time(locale, position)),
            ("total", spoken_time(locale, duration)),
        ],
    )
}

pub fn page_title(locale: Locale, route: Route) -> String {
    let page = match route {
        Route::Work(_) => text(Key::AboutWork, locale),
        route => text(route_key(route), locale),
    };
    format(Key::PageTitle, locale, &[("page", page.to_owned())])
}

pub fn speed_label(locale: Locale, speed: u16) -> String {
    let separator = if locale == Locale::Vietnamese {
        ','
    } else {
        '.'
    };
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
        Route::History => Key::History,
        Route::Player => Key::Player,
        Route::Queue => Key::Queue,
        Route::Settings => Key::Settings,
        Route::Account => Key::Account,
        Route::Auth(_) => Key::SignIn,
        Route::Membership(_) => Key::MembershipRegister,
        Route::NotFound => Key::NotFound,
    }
}

pub fn status_key(status: Status) -> Key {
    match status {
        Status::Idle => Key::NoPlayer,
        Status::Loading => Key::PlayerLoading,
        Status::Paused(PauseCause::User) => Key::Paused,
        Status::Paused(PauseCause::Interruption) => Key::Interrupted,
        Status::Paused(PauseCause::ColdRestore) => Key::ResumeOffered,
        Status::Simulating => Key::Simulating,
        Status::Buffering => Key::Buffering,
        Status::Ended => Key::Ended,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn placeholders(value: &str) -> BTreeSet<&str> {
        value
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|pair| pair.0))
            .collect()
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
