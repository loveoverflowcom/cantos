//! Typed resources for live and mock listener surfaces; story fixtures retain their language.
use cantos_contracts::theatre::WorkKind;

#[cfg(feature = "preview")]
use crate::core::catalog::Genre;
use crate::{
    app::routes::Route,
    core::playback::{Failure, PauseCause, Status},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    Vietnamese,
    English,
}

/// CLDR cardinal plural category; the shipped locales need only `one` and `other`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plural {
    One,
    Other,
}

impl Locale {
    pub const ALL: [Self; 2] = [Self::Vietnamese, Self::English];

    /// Exact supported tag only; an unknown tag is `None`, never a silent default.
    pub fn from_lang(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|locale| locale.lang() == tag)
    }

    /// The language's own name; a language picker shows it untranslated.
    pub const fn autonym(self) -> &'static str {
        match self {
            Self::Vietnamese => "Tiếng Việt",
            Self::English => "English",
        }
    }

    pub const fn lang(self) -> &'static str {
        match self {
            Self::Vietnamese => "vi-VN",
            Self::English => "en",
        }
    }

    /// CLDR cardinal rule for a whole number: `vi` has only `other`; `en` has `one` for exactly 1.
    pub const fn plural(self, count: u64) -> Plural {
        match (self, count) {
            (Self::English, 1) => Plural::One,
            _ => Plural::Other,
        }
    }

    const fn decimal_separator(self) -> char {
        match self {
            Self::Vietnamese => ',',
            Self::English => '.',
        }
    }
}

macro_rules! resources {
    ($($(#[$meta:meta])* $key:ident => ($vi:literal, $en:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Key { $($(#[$meta])* $key),+ }
        impl Key {
            pub const ALL: &'static [Self] = &[$($(#[$meta])* Self::$key),+];
        }
        pub fn text(key: Key, locale: Locale) -> &'static str {
            match (key, locale) {
                $($(#[$meta])* (Key::$key, Locale::Vietnamese) => $vi,
                  $(#[$meta])* (Key::$key, Locale::English) => $en),+
            }
        }
    };
}

resources! {
    ErrorDialogTitle => ("Chưa hoàn tất thao tác", "Couldn’t complete that"),
    CloseError => ("Đóng", "Close"),
    ErrorNetwork => ("Chưa kết nối được với máy chủ. Hãy kiểm tra mạng và thử lại.", "Couldn’t reach the server. Check your connection and try again."),
    ErrorTimeout => ("Phản hồi từ máy chủ quá thời gian chờ. Hãy thử lại.", "The server took too long to respond. Try again."),
    ErrorUnauthenticated => ("Cần đăng nhập để thực hiện thao tác này.", "Sign in to continue with this action."),
    ErrorForbidden => ("Tài khoản hiện tại không có quyền thực hiện thao tác này.", "Your account does not have access to this action."),
    ErrorContentUnavailable => ("Nội dung này đã thay đổi hoặc không còn mở công khai.", "This content has changed or is no longer available."),
    ErrorProtocol => ("Phản hồi từ máy chủ không đúng định dạng mong đợi. Hãy thử lại.", "The server returned an unexpected response. Try again."),
    ErrorConfiguration => ("Cấu hình máy chủ của bản dựng này không hợp lệ.", "This build’s server configuration is invalid."),
    ErrorStorage => ("Chưa truy cập được dữ liệu trên thiết bị này. Các thao tác đang chờ được giữ lại; Cantos chưa xác nhận đã đồng bộ.", "Local data is unavailable. Pending actions are kept; sync has not been confirmed."),
    ErrorStorageRecovery => ("Kiểm tra lại bộ nhớ", "Check storage again"),
    ErrorServiceUnavailable => ("Dịch vụ đang tạm thời gián đoạn. Hãy thử lại sau ít phút.", "The service is temporarily unavailable. Try again in a moment."),
    PublicHomeEyebrow => ("VỌNG ĐÀI · NHỮNG MIỀN THANH ÂM", "VỌNG ĐÀI · WORLDS OF SOUND"),
    PublicHomeIntroduction => ("Một câu chuyện hay không cần đợi bạn đăng nhập. Khám phá nội dung công khai ngay tại đây.", "A good story does not need to wait for sign-in. Explore public stories here."),
    PublicHomeLoading => ("Đang mở những câu chuyện đã công bố…", "Opening published stories…"),
    PublicHomeEmpty => ("Những câu chuyện đang được chuẩn bị.", "Stories are being prepared."),
    PublicHomeEmptyHelp => ("Danh mục công khai chưa có tác phẩm. Bạn có thể quay lại hoặc cập nhật sau.", "The public catalog has no stories yet. You can return or refresh later."),
    PublicHomeUnavailable => ("Chưa mở được danh mục công khai.", "The public catalog could not be opened."),
    PublicConnectionUnavailableHelp => ("Kết nối hiện chưa sẵn sàng. Hãy thử lại; bạn không cần đăng nhập để xem nội dung công khai.", "The connection is unavailable. Try again; public stories do not require sign-in."),
    PublicHomeProtocol => ("Chưa đọc được danh mục công khai.", "The public catalog response could not be read."),
    PublicHomeProtocolHelp => ("Phản hồi chưa hợp lệ. Hãy thử lại để tải danh mục từ Cantos.", "The response was invalid. Try loading the Cantos catalog again."),
    PublicRetry => ("Thử lại", "Try again"),
    PublicHomeRefresh => ("Cập nhật danh mục", "Refresh catalog"),
    PublicWorkKindNovel => ("Tiểu thuyết", "Novel"),
    PublicWorkKindStory => ("Truyện", "Story"),
    PublicWorkKindPodcast => ("Podcast", "Podcast"),
    PublicWorkLoading => ("Đang mở tác phẩm…", "Opening the story…"),
    PublicWorkNotFound => ("Tác phẩm này hiện không mở công khai.", "This story is not publicly available right now."),
    PublicWorkNotFoundHelp => ("Cantos không thể hiển thị tác phẩm này cho người nghe lúc này. Bạn có thể quay về Trang chủ để khám phá những câu chuyện khác.", "Cantos cannot show this story to listeners right now. You can return Home to explore other stories."),
    PublicWorkUnavailable => ("Chưa mở được tác phẩm.", "The story could not be opened."),
    PublicWorkProtocol => ("Chưa đọc được thông tin tác phẩm.", "The story details could not be read."),
    PublicWorkProtocolHelp => ("Phản hồi chưa hợp lệ. Hãy thử lại để tải tác phẩm từ Cantos.", "The response was invalid. Try loading the story from Cantos again."),
    PublicWorkChapter => ("Chương", "Chapter"),
    PublicWorkEpisode => ("Tập", "Episode"),
    PublicWorkChapters => ("Các chương", "Chapters"),
    PublicWorkEpisodes => ("Các tập", "Episodes"),
    PublicWorkListen => ("Nghe", "Listen"),
    PublicWorkListenUnavailable => ("Chưa có chương hoặc tập đã công bố để nghe. Danh sách có thể đang tải.", "There is no published chapter or episode to play yet. The list may still be loading."),
    #[cfg(feature = "preview")]
    ListenUnit => ("Nghe {title}", "Listen to {title}"),
    #[cfg(feature = "preview")]
    ListenUnitPause => ("Tạm dừng {title}", "Pause {title}"),
    ListenUnitPreparing => ("Đang chuẩn bị {title}…", "Preparing {title}…"),
    #[cfg(feature = "preview")]
    ListenUnitRetry => ("Thử lại {title}", "Try {title} again"),
    ListenResolveNotFound => ("Bản phát hành này không còn khả dụng.", "This release is no longer available."),
    ListenResolveUnavailable => ("Chưa kết nối được để chuẩn bị bản nghe. Hãy thử lại.", "Could not prepare the audio. Try again."),
    ListenResolveProtocol => ("Cantos chưa đọc được thông tin của bản nghe này.", "Cantos could not read this release's details."),
    ListenNothingSelected => ("Chưa chọn nội dung để nghe", "Nothing selected to listen to"),
    ListenNothingSelectedHelp => ("Mở một tác phẩm đã công bố rồi chọn một chương hoặc tập.", "Open a published story and choose a chapter or episode."),
    ListenMiniPlayer => ("Trình phát thu gọn", "Compact player"),
    ListenPlay => ("Phát", "Play"),
    ListenPause => ("Tạm dừng", "Pause"),
    ListenReplay => ("Nghe lại từ đầu", "Play again from the start"),
    ListenTransportName => ("{action}: {title}", "{action}: {title}"),
    ListenPlaying => ("Đang phát", "Playing"),
    ListenPaused => ("Đang tạm dừng", "Paused"),
    ListenPreparing => ("Đang chuẩn bị âm thanh…", "Preparing audio…"),
    ListenBuffering => ("Đang đệm âm thanh…", "Buffering audio…"),
    ListenEnded => ("Đã nghe hết", "Finished"),
    ListenInterrupted => ("Tạm dừng sau gián đoạn", "Paused after an interruption"),
    ListenResumeOffered => ("Có vị trí để nghe tiếp, chưa phát", "A resume position is available; playback has not started"),
    ListenTimeline => ("Vị trí nghe", "Listening position"),
    ListenTimelineHelp => ("Kéo để xem vị trí, thả để chuyển tới đó. Escape hủy kéo. Dùng phím mũi tên, Home hoặc End để đổi vị trí.", "Drag to preview, release to move there. Escape cancels a drag. Use arrow keys, Home or End to change the position."),
    ListenScrubPreview => ("Đang xem trước vị trí", "Previewing position"),
    ListenSeekPending => ("Đang chuyển tới vị trí đã chọn…", "Moving to the chosen position…"),
    ListenBack15 => ("Lùi 15 giây", "Back 15 seconds"),
    ListenForward15 => ("Tiến 15 giây", "Forward 15 seconds"),
    ListenSpeed => ("Tốc độ nghe", "Listening speed"),
    #[cfg(feature = "preview")]
    ListenOpenWork => ("Mở tác phẩm {title}", "Open the story {title}"),
    ListenFailedPlayRefused => ("Trình duyệt chưa cho phát. Nhấn Phát để thử lại.", "The browser did not start playback. Press Play to try again."),
    ListenFailedUnavailable => ("Bản phát hành này không còn khả dụng. Cantos dừng lại thay vì chuyển sang bản khác.", "This release is no longer available. Cantos stopped instead of switching to another."),
    ListenFailedNetwork => ("Mất kết nối khi tải âm thanh. Nhấn Phát để thử lại.", "The connection failed while loading audio. Press Play to try again."),
    ListenFailedDecode => ("Trình duyệt không giải mã được âm thanh này.", "The browser could not decode this audio."),
    ListenFailedProtocol => ("Máy chủ trả về dữ liệu phát không hợp lệ. Nhấn Phát để thử lại.", "The server returned invalid playback data. Press Play to try again."),
    ListenFailedIdentity => ("Bản nghe đã đổi. Cantos dừng lại thay vì phát bản khác; hãy mở lại từ danh sách.", "The release changed. Cantos stopped instead of playing another; open it again from the list."),
    PublicWorkUnitsLoading => ("Đang tải danh sách…", "Loading the list…"),
    PublicWorkUnitsRestarting => ("Đang tải lại danh sách từ đầu…", "Reloading the list from the start…"),
    PublicWorkChaptersEmpty => ("Tác phẩm này chưa có chương nào mở công khai.", "This story has no publicly available chapters yet."),
    PublicWorkEpisodesEmpty => ("Podcast này chưa có tập nào mở công khai.", "This podcast has no publicly available episodes yet."),
    PublicWorkUnitsComplete => ("Đã hiển thị hết danh sách.", "You have reached the end of the list."),
    PublicWorkUnitsMoreAvailable => ("Danh sách vẫn còn phần tiếp theo.", "More of the list is available."),
    PublicWorkUnitsFailed => ("Chưa tải được danh sách.", "The list could not be loaded."),
    PublicWorkUnitsUnavailableHelp => ("Kết nối hiện chưa sẵn sàng. Hãy thử lại.", "The connection is unavailable. Try again."),
    PublicWorkUnitsProtocolHelp => ("Phản hồi chưa hợp lệ. Hãy thử lại.", "The response was invalid. Try again."),
    PublicWorkPageUnavailable => ("Chưa tải được phần tiếp theo. Những mục đã hiển thị vẫn được giữ nguyên.", "The next part could not be loaded. Items already shown are kept."),
    PublicWorkPageProtocol => ("Phần tiếp theo trả về dữ liệu chưa hợp lệ. Những mục đã hiển thị vẫn được giữ nguyên.", "The next part returned invalid data. Items already shown are kept."),
    PublicWorkRestarted => ("Phiên duyệt danh sách đã hết hạn nên Cantos tải lại từ đầu; các mục đã hiển thị được giữ và không lặp lại.", "The list session expired, so Cantos reloaded it from the start; items already shown are kept without duplicates."),
    PublicWorkMoreChapters => ("Tải thêm chương", "Load more chapters"),
    PublicWorkMoreEpisodes => ("Tải thêm tập", "Load more episodes"),
    PublicWorkLoadingMore => ("Đang tải phần tiếp theo…", "Loading the next part…"),
    PublicWorkRetryPage => ("Thử lại phần này", "Retry this part"),
    PublicWorkDurationDescription => ("Thời lượng: {duration}", "Duration: {duration}"),
    PublicWorkDurationUnknown => ("Chưa rõ thời lượng", "Duration not known yet"),
    PublicWorkRowUnavailable => ("Mục này không còn trong danh sách công khai.", "This item is no longer in the public list."),
    PublicWorkUnitCountChapters => ("Chương đã công bố: {count}", "Published chapters: {count}"),
    PublicWorkUnitCountEpisodes => ("Tập đã công bố: {count}", "Published episodes: {count}"),
    PublicWorkUnitLabel => ("{kind} · Vị trí {ordinal}", "{kind} · Position {ordinal}"),
    DiscoverySearchLabel => ("Tìm trong danh mục", "Search the catalog"),
    DiscoverySearchSubmit => ("Tìm", "Search"),
    DiscoveryKindLabel => ("Loại", "Type"),
    DiscoveryKindAll => ("Tất cả loại", "All types"),
    DiscoveryLanguageLabel => ("Ngôn ngữ nội dung", "Content language"),
    DiscoveryLanguageAny => ("Mọi ngôn ngữ", "All languages"),
    PublicHomeFeaturedEyebrow => ("Tác phẩm đầu danh mục", "First in the catalog"),
    PublicHomeFirstMatchEyebrow => ("Kết quả khớp đầu tiên", "First match"),
    PublicHomeFeatureMeta => ("{kind} · {language} · {units}", "{kind} · {language} · {units}"),
    PublicHomeOpenWork => ("Mở tác phẩm", "Open story"),
    PublicHomeOpenWorkNamed => ("Mở tác phẩm: {title}", "Open story: {title}"),
    DiscoveryLanguageInvalid => ("Mã ngôn ngữ chỉ gồm chữ cái, chữ số và dấu gạch ngang, tối đa 63 ký tự.", "A language tag has only letters, digits and hyphens, at most 63 characters."),
    DiscoverySearchTooLong => ("Từ khóa quá dài: tối đa 512 byte.", "The search is too long: at most 512 bytes."),
    DiscoverySearchInvalid => ("Từ khóa chứa ký tự điều khiển không được hỗ trợ.", "The search contains an unsupported control character."),
    DiscoveryClear => ("Xóa tìm kiếm và bộ lọc", "Clear search and filters"),
    DiscoveryNoMatches => ("Không có tác phẩm nào khớp.", "No stories match."),
    DiscoveryNoMatchesHelp => ("Hãy thử từ khóa khác hoặc bỏ bớt bộ lọc.", "Try another search or remove a filter."),
    DiscoveryIncomplete => ("Chưa thấy kết quả trong phần đã duyệt.", "No match yet in the part searched so far."),
    DiscoveryIncompleteHelp => ("Danh mục còn phần chưa duyệt, nên đây chưa phải là hết kết quả. Hãy tìm tiếp.", "Part of the catalog has not been searched, so this is not the end of the results. Keep searching."),
    DiscoveryKeepSearching => ("Tìm tiếp", "Keep searching"),
    DiscoveryLoadMore => ("Tải thêm tác phẩm", "Load more stories"),
    DiscoveryLoadingMore => ("Đang tải thêm tác phẩm…", "Loading more stories…"),
    DiscoveryShowing => ("Đang hiển thị {count} tác phẩm; danh mục còn nội dung tiếp theo.", "Showing {count} stories; the catalog has more."),
    DiscoveryComplete => ("Đã hiển thị hết các tác phẩm: {count}.", "You have reached the end: {count} stories."),
    DiscoveryPageUnavailable => ("Chưa tải được phần tiếp theo. Các tác phẩm đã hiển thị vẫn được giữ nguyên.", "The next part could not be loaded. Stories already shown are kept."),
    DiscoveryPageProtocol => ("Phần tiếp theo trả về dữ liệu chưa hợp lệ. Các tác phẩm đã hiển thị vẫn được giữ nguyên.", "The next part returned invalid data. Stories already shown are kept."),
    DiscoveryExpired => ("Phiên duyệt danh mục đã hết hạn. Các tác phẩm đã hiển thị vẫn được giữ; hãy tải lại từ đầu để xem phần còn lại.", "The browsing session expired. Stories already shown are kept; reload from the start to see the rest."),
    DiscoveryRestart => ("Tải lại từ đầu", "Reload from the start"),
    LiveNotFound => ("Không tìm thấy trang này", "This page could not be found"),
    LiveNotFoundHelp => ("Đường dẫn không đúng hoặc không còn tồn tại. Bạn có thể quay về Khám phá để tìm câu chuyện khác.", "The link is wrong or no longer exists. You can return to Explore to find another story."),
    LiveBackToDiscover => ("Về Khám phá", "Back to Explore"),
    LiveRouteUnavailable => ("Tính năng này chưa mở", "This feature is not available yet"),
    LiveRouteUnavailableHelp => ("Phần này chưa được nối với dữ liệu thật nên Cantos không hiển thị nội dung mẫu. Bạn vẫn có thể khám phá và nghe các tác phẩm đã công bố.", "This part is not connected to real data yet, so Cantos shows no sample content. You can still explore and listen to published stories."),
    #[cfg(feature = "preview")]
    AuthPreviewHeading => ("Auth preview · Chỉ dữ liệu mẫu", "Auth preview · Fixtures only"),
    #[cfg(feature = "preview")]
    AuthPreviewNotice => ("Công cụ kiểm tra riêng: không gọi mạng, timer hoặc thông tin đăng nhập thật. A/B là tài khoản mẫu; không chứng nhận đăng nhập hoặc đăng ký thực.", "Isolated diagnostics: no network, timers or real credentials. A/B are fixture accounts; this does not verify real sign-in or registration."),
    #[cfg(feature = "preview")]
    AuthPreviewGuest => ("Mẫu khách · cleanup đã xác nhận", "Guest fixture · cleanup acknowledged"),
    #[cfg(feature = "preview")]
    AuthPreviewSignedA => ("Mẫu đăng nhập A", "Signed-in fixture A"),
    #[cfg(feature = "preview")]
    AuthPreviewSignedB => ("Mẫu đăng nhập B", "Signed-in fixture B"),
    #[cfg(feature = "preview")]
    AuthPreviewLogoutPending => ("Bắt đầu mẫu đăng xuất", "Begin logout fixture"),
    #[cfg(feature = "preview")]
    AuthPreviewCleanupFail => ("Mẫu cleanup thất bại", "Fail cleanup fixture"),
    #[cfg(feature = "preview")]
    AuthPreviewCleanupAck => ("Xác nhận cleanup mẫu", "Acknowledge cleanup fixture"),
    #[cfg(feature = "preview")]
    AuthPreviewBeginDelayed => ("Giữ phản hồi phiên A", "Hold session A response"),
    #[cfg(feature = "preview")]
    AuthPreviewCompleteDelayed => ("Trả phản hồi A đã giữ", "Deliver held A response"),
    #[cfg(feature = "preview")]
    AuthPreviewCancel => ("Hủy yêu cầu mẫu và rời auth", "Cancel fixture request and leave auth"),
    #[cfg(feature = "preview")]
    AuthPreviewCleanupHelp => ("Bắt đầu mẫu đăng xuất trước khi thử phản hồi cleanup; xác nhận thành công mới mở lại Login.", "Begin the logout fixture before delivering cleanup responses; only acknowledgement re-enables Login."),
    #[cfg(feature = "preview")]
    AuthPreviewDelayHelp => ("Giữ phản hồi A trước khi trả phản hồi. Sau khi hủy hoặc chuyển khách → B, phản hồi A cũ phải được bỏ qua.", "Hold A before delivering its response. After cancellation or a Guest → B switch, the old A response must be ignored."),
    #[cfg(feature = "preview")]
    AuthPreviewReady => ("Sẵn sàng kiểm tra trạng thái mẫu.", "Ready to inspect fixture states."),
    #[cfg(feature = "preview")]
    AuthPreviewApplied => ("Đã áp dụng chuyển trạng thái mẫu qua AccountViewState.", "Fixture transition applied through AccountViewState."),
    #[cfg(feature = "preview")]
    AuthPreviewNeedsCleanup => ("Core chặn thao tác: phải xác nhận cleanup trước.", "Core refused the action: acknowledge cleanup first."),
    #[cfg(feature = "preview")]
    AuthPreviewRequestPending => ("Core chặn yêu cầu trùng trong khi phản hồi cũ còn chờ.", "Core refused a duplicate request while a response is pending."),
    #[cfg(feature = "preview")]
    AuthPreviewRejected => ("Core từ chối chuyển trạng thái mẫu; phiên hiện tại được giữ.", "Core refused the fixture transition; the current session is retained."),
    #[cfg(feature = "preview")]
    AuthPreviewDelayed => ("Đã giữ generation của yêu cầu A. Hủy hoặc chuyển khách → B rồi trả A để kiểm tra phản hồi trễ.", "Captured request A's generation. Cancel or switch Guest → B, then deliver A to inspect a stale response."),
    #[cfg(feature = "preview")]
    AuthPreviewIgnored => ("Core đã bỏ qua phản hồi A cũ; trạng thái hiện tại được giữ.", "Core ignored the old A response; the current state is retained."),
    #[cfg(feature = "preview")]
    AuthPreviewCleanupFailed => ("Cleanup mẫu chưa xác nhận; Login vẫn bị khóa, có thể thử hoàn tất đăng xuất.", "Fixture cleanup remains unacknowledged; Login stays blocked and sign-out can be retried."),
    #[cfg(feature = "preview")]
    AuthPreviewCancelled => ("Đã hủy generation mẫu và rời auth. Phản hồi A đã giữ có thể được trả để kiểm tra việc bỏ qua.", "Fixture generation cancelled and auth closed. Deliver the held A response to inspect rejection."),
    #[cfg(feature = "preview")]
    CoverArtName => ("{title}: {description}", "{title}: {description}"),
    MembershipHeading => ("Membership", "Membership"),
    PageTitle => ("{page} · Cantos", "{page} · Cantos"),
    SkipContent => ("Đến nội dung chính", "Skip to main content"),
    MainNavigation => ("Điều hướng chính", "Main navigation"),
    BrandCaption => ("VỌNG ĐÀI · MỘT THẾ GIỚI ĐỂ NGHE", "VỌNG ĐÀI · A WORLD TO LISTEN TO"),
    Discover => ("Khám phá", "Discover"),
    Explore => ("Tìm tác phẩm", "Find a story"),
    Library => ("Của tôi", "My library"),
    History => ("Lịch sử", "History"),
    #[cfg(feature = "preview")]
    LibraryDeck => ("Những câu chuyện bạn muốn trở lại.", "Stories you want to return to."),
    Player => ("Đang nghe", "Now listening"),
    Queue => ("Tiếp theo", "Up next"),
    Settings => ("Cài đặt nghe", "Listening settings"),
    Account => ("Tài khoản", "Account"),
    #[cfg(feature = "preview")]
    MembershipPreview => ("Membership · bản xem thử", "Membership · preview"),
    MembershipRegister => ("Đăng ký Membership", "Register Membership interest"),
    MembershipUnavailable => ("Đăng ký Membership chưa khả dụng. Chưa có dịch vụ tiếp nhận được chấp nhận; thông tin không được gửi.", "Membership registration is unavailable. No accepted intake service exists; information is not sent."),
    NotFound => ("Trang chưa có", "Page not found"),
    HomeHeading => ("Mở một miền tưởng tượng.", "Open a world of imagination."),
    #[cfg(feature = "preview")]
    HomeDeck => ("Những câu chuyện để lắng nghe, chậm lại và đi xa hơn.", "Stories to listen to, slow down with and journey further."),
    #[cfg(feature = "preview")]
    Quote => ("Nhắm mắt lại. Mở một thế giới.", "Close your eyes. Open a world."),
    #[cfg(feature = "preview")]
    DemoBadge => ("Bản mô phỏng · dữ liệu minh họa", "Mock demo · sample data"),
    #[cfg(feature = "preview")]
    DemoNotice => ("Bản mô phỏng: không có âm thanh, tài khoản hay đồng bộ. Dữ liệu chỉ lưu trong phiên này; tải lại trang sẽ đặt lại.", "Mock demo: no audio, account or sync. Data lasts only for this session; reloading resets it."),
    #[cfg(feature = "preview")]
    Featured => ("TUYỂN CHỌN CANTOS", "CANTOS SELECTION"),
    #[cfg(feature = "preview")]
    FeaturedSubtitle => ("Một tiếng chuông lay động sơn hà.", "A bell echoes across mountains and rivers."),
    #[cfg(feature = "preview")]
    Begin => ("Mở tập nghe mẫu", "Open a sample episode"),
    AboutWork => ("Về tác phẩm", "About the story"),
    #[cfg(feature = "preview")]
    BackDiscover => ("Về khám phá", "Back to discovery"),
    StoryShelf => ("Mỗi câu chuyện, một thế giới", "Every story, a world"),
    Saved => ("Đã lưu", "Saved"),
    Save => ("Lưu", "Save"),
    RemoveSave => ("Bỏ lưu", "Unsave"),
    SaveName => ("Lưu tác phẩm {title}", "Save story {title}"),
    RemoveSaveName => ("Bỏ lưu tác phẩm {title}", "Unsave story {title}"),
    Continue => ("Nghe tiếp", "Continue listening"),
    #[cfg(feature = "preview")]
    ContinueHeading => ("Nghe tiếp", "Continue listening"),
    #[cfg(feature = "preview")]
    ResumeSample => ("Mở vị trí nghe", "Open listening position"),
    #[cfg(feature = "preview")]
    ResumeSampleName => ("Mở vị trí nghe {title} tại {time}", "Open listening position for {title} at {time}"),
    #[cfg(feature = "preview")]
    ResumeSampleHelp => ("Mở lại đúng vị trí và giữ tạm dừng. Chọn Phát mô phỏng để thử tiếp.", "Reopens the position paused. Choose Simulate play to try it."),
    #[cfg(feature = "preview")]
    NoContinue => ("Chưa có gì để nghe tiếp", "Nothing to pick up yet"),
    #[cfg(feature = "preview")]
    NoContinueHelp => ("Mở một tập, rồi quay lại đây để mở lại đúng vị trí trong phiên này.", "Open an episode, then return here to reopen the same position during this session."),
    #[cfg(feature = "preview")]
    NoSaved => ("Chưa lưu tác phẩm nào", "No saved stories yet"),
    #[cfg(feature = "preview")]
    NoSavedHelp => ("Chọn Lưu trên tác phẩm bạn thích để thấy chúng ở đây.", "Choose Save on a story you like and it will appear here."),
    #[cfg(feature = "preview")]
    NoHistory => ("Chưa có lịch sử nghe", "No listening history yet"),
    #[cfg(feature = "preview")]
    NoHistoryHelp => ("Lịch sử ghi nhận các tập đã Phát mô phỏng trong phiên này; chỉ mở trang hoặc lưu truyện chưa tạo lịch sử.", "History records episodes after Simulate play in this session; opening a page or saving a story does not add history."),
    #[cfg(feature = "preview")]
    HistoryHelp => ("Các câu chuyện đã phát mô phỏng, gần nhất ở trên. Mở lại sẽ khôi phục vị trí và giữ tạm dừng.", "Stories played in the simulation, most recent first. Reopening restores the position and stays paused."),
    #[cfg(feature = "preview")]
    BrowseStories => ("Khám phá tác phẩm", "Discover stories"),
    #[cfg(feature = "preview")]
    ViewAll => ("Xem tất cả", "View all"),
    #[cfg(feature = "preview")]
    ViewAllHistory => ("Xem tất cả lịch sử nghe", "View all listening history"),
    #[cfg(feature = "preview")]
    BackLibrary => ("Về Của tôi", "Back to My library"),
    #[cfg(feature = "preview")]
    ClearHistory => ("Xóa lịch sử nghe mẫu", "Clear sample listening history"),
    #[cfg(feature = "preview")]
    ClearHistoryConfirmTitle => ("Xóa lịch sử trong phiên này?", "Clear history for this session?"),
    #[cfg(feature = "preview")]
    ClearHistoryHelp => ("Chỉ xóa danh sách lịch sử. Vị trí nghe, truyện đã lưu, dấu trang và trình phát vẫn được giữ lại.", "Only the history list is cleared. Listening positions, saved stories, bookmarks and the player are retained."),
    #[cfg(feature = "preview")]
    ConfirmClearHistory => ("Xác nhận xóa lịch sử", "Confirm clear history"),
    Cancel => ("Hủy", "Cancel"),
    #[cfg(feature = "preview")]
    Bookmarks => ("Dấu trang", "Bookmarks"),
    #[cfg(feature = "preview")]
    Episodes => ("Các tập nghe mẫu", "Sample episodes"),
    #[cfg(feature = "preview")]
    Episode => ("Tập {number}", "Episode {number}"),
    EpisodeCountOne => ("{count} tập", "{count} episode"),
    EpisodeCountOther => ("{count} tập", "{count} episodes"),
    #[cfg(feature = "preview")]
    ContentLanguage => ("Tiếng Việt · nội dung minh họa", "Vietnamese · sample content"),
    #[cfg(feature = "preview")]
    Fantasy => ("Tiên hiệp", "Cultivation fantasy"),
    #[cfg(feature = "preview")]
    Memory => ("Dã sử", "Historical fiction"),
    #[cfg(feature = "preview")]
    Dream => ("Huyền ảo", "Fantasy"),
    #[cfg(feature = "preview")]
    All => ("Tất cả", "All stories"),
    #[cfg(feature = "preview")]
    CoverPlaceholder => ("Bìa minh họa bằng chữ · chưa có ảnh được cấp quyền", "Typographic cover · licensed artwork pending"),
    #[cfg(feature = "preview")]
    SearchLabel => ("Tìm theo tên hoặc mô tả", "Search titles or descriptions"),
    #[cfg(feature = "preview")]
    SearchHelp => ("Tìm cục bộ, không gửi dữ liệu. Phân biệt dấu tiếng Việt.", "Local search, no data sent. Vietnamese diacritics are significant."),
    #[cfg(feature = "preview")]
    Filters => ("Lọc thể loại", "Genre filters"),
    #[cfg(feature = "preview")]
    NoResults => ("Chưa tìm thấy câu chuyện", "No stories found"),
    #[cfg(feature = "preview")]
    NoResultsHelp => ("Thử tên có dấu hoặc bỏ bộ lọc để xem lại các tác phẩm mẫu.", "Try an accented title or clear filters to see the sample stories."),
    #[cfg(feature = "preview")]
    ClearSearch => ("Xóa tìm kiếm và bộ lọc", "Clear search and filters"),
    #[cfg(feature = "preview")]
    Loading => ("Đang tải danh mục mẫu…", "Loading the sample catalog…"),
    #[cfg(feature = "preview")]
    LoadingHelp => ("Trạng thái thử nghiệm giữ nguyên cho đến khi bạn chọn tiếp tục.", "This demo state stays visible until you choose to continue."),
    #[cfg(feature = "preview")]
    LoadDemo => ("Hiện dữ liệu mẫu", "Show sample data"),
    #[cfg(feature = "preview")]
    EmptyCatalog => ("Danh mục đang trống", "The catalog is empty"),
    #[cfg(feature = "preview")]
    EmptyCatalogHelp => ("Đây là trạng thái mẫu. Có thể trở lại dữ liệu minh họa bất kỳ lúc nào.", "This is a sample empty state. You can return to the demo data any time."),
    #[cfg(feature = "preview")]
    CatalogError => ("Chưa mở được danh mục mẫu", "The sample catalog could not be opened"),
    #[cfg(feature = "preview")]
    CatalogErrorHelp => ("Lỗi được mô phỏng, không có yêu cầu mạng. Vị trí nghe và tủ sách của bạn vẫn giữ nguyên.", "This is a simulated error, with no network request. Your position and library are unchanged."),
    #[cfg(feature = "preview")]
    Retry => ("Thử lại dữ liệu mẫu", "Retry sample data"),
    #[cfg(feature = "preview")]
    NoLibrary => ("Tủ sách chưa có nội dung", "Your library is empty"),
    #[cfg(feature = "preview")]
    NoLibraryHelp => ("Lưu tác phẩm, mở một tập mẫu hoặc thêm dấu trang để bắt đầu.", "Save a story, open a sample episode or add a bookmark to begin."),
    #[cfg(feature = "preview")]
    NoPlayer => ("Chưa chọn tập nghe", "No episode selected"),
    #[cfg(feature = "preview")]
    NoPlayerHelp => ("Chọn một tập mẫu từ tác phẩm. Demo không phát âm thanh.", "Choose a sample episode from a story. The demo has no audio."),
    #[cfg(feature = "preview")]
    MiniPlayer => ("Trình phát mô phỏng thu gọn", "Compact mock player"),
    #[cfg(feature = "preview")]
    FullPlayer => ("Mở trình phát mô phỏng", "Open mock player"),
    #[cfg(feature = "preview")]
    Play => ("Phát mô phỏng", "Simulate play"),
    #[cfg(feature = "preview")]
    Pause => ("Tạm dừng mô phỏng", "Pause simulation"),
    #[cfg(feature = "preview")]
    Replay => ("Mô phỏng từ đầu", "Simulate from the beginning"),
    #[cfg(feature = "preview")]
    PlayName => ("Mở tập nghe mẫu {title}", "Open a sample episode: {title}"),
    #[cfg(feature = "preview")]
    Paused => ("Mô phỏng · đang tạm dừng", "Mock · paused"),
    #[cfg(feature = "preview")]
    Simulating => ("Mô phỏng đang phát · không có âm thanh", "Simulating play · no audio"),
    #[cfg(feature = "preview")]
    Ended => ("Mô phỏng · đã hết tập", "Mock · episode ended"),
    #[cfg(feature = "preview")]
    PlayerLoading => ("Mô phỏng · đang chờ bộ máy giả xác nhận", "Mock · waiting for fake-engine confirmation"),
    #[cfg(feature = "preview")]
    Buffering => ("Mô phỏng · đang đệm, không có âm thanh", "Mock · buffering, no audio"),
    #[cfg(feature = "preview")]
    Interrupted => ("Mô phỏng · dừng sau gián đoạn", "Mock · paused after interruption"),
    #[cfg(feature = "preview")]
    ResumeOffered => ("Mô phỏng · có vị trí để nghe tiếp, chưa phát", "Mock · resume position offered, playback has not started"),
    #[cfg(feature = "preview")]
    MockEngineTools => ("Thử bộ máy giả", "Fake-engine demo controls"),
    #[cfg(feature = "preview")]
    MockBuffer => ("Mô phỏng đệm", "Simulate buffering"),
    #[cfg(feature = "preview")]
    MockReady => ("Xác nhận phát mô phỏng", "Confirm simulated play"),
    #[cfg(feature = "preview")]
    MockInterrupt => ("Mô phỏng gián đoạn", "Simulate interruption"),
    #[cfg(feature = "preview")]
    MockEndInterrupt => ("Kết thúc gián đoạn, giữ tạm dừng", "End interruption and stay paused"),
    #[cfg(feature = "preview")]
    MockEnd => ("Mô phỏng kết thúc tập", "Simulate episode end"),
    #[cfg(feature = "preview")]
    MockRestore => ("Khôi phục vị trí trong phiên", "Restore position within this session"),
    #[cfg(feature = "preview")]
    MockRejected => ("Thao tác mẫu chưa được chấp nhận. Kiểm tra trạng thái mô phỏng rồi thử lại.", "The sample action was not accepted. Check the mock state and try again."),
    #[cfg(feature = "preview")]
    MockEngineHelp => ("Chỉ thử sự kiện giả. Phát mô phỏng trước khi thử đệm; kết thúc gián đoạn trước khi phát lại. Không có âm thanh, cuộc gọi thật, lưu trữ hay tự nghe tiếp.", "Fake events only. Simulate play before buffering; end an interruption before playing again. No sound, actual phone interruption, storage or automatic resume."),
    #[cfg(feature = "preview")]
    Timeline => ("Vị trí mô phỏng", "Mock position"),
    #[cfg(feature = "preview")]
    TimelineHelp => ("Kéo để xem vị trí, thả để xác nhận. Escape hủy kéo. Dùng phím mũi tên, Home hoặc End để đổi vị trí. Không có âm thanh hay đồng hồ phát.", "Drag to preview, release to commit. Escape cancels a drag. Use arrow keys, Home or End to seek. No sound or playback clock."),
    #[cfg(feature = "preview")]
    ScrubPreview => ("Đang xem trước vị trí mô phỏng", "Previewing mock position"),
    #[cfg(feature = "preview")]
    SeekPending => ("Đang xác nhận vị trí mô phỏng", "Confirming mock position"),
    #[cfg(feature = "preview")]
    TransportEpisode => ("{action}: {episode}", "{action}: {episode}"),
    SpokenTimeOf => ("{elapsed} trên tổng số {total}", "{elapsed} of {total}"),
    SpokenTime => ("{minutes} {seconds}", "{minutes} {seconds}"),
    MinuteOne => ("{count} phút", "{count} minute"),
    MinuteOther => ("{count} phút", "{count} minutes"),
    SecondOne => ("{count} giây", "{count} second"),
    SecondOther => ("{count} giây", "{count} seconds"),
    #[cfg(feature = "preview")]
    Back15 => ("Lùi 15 giây mô phỏng", "Move mock position back 15 seconds"),
    #[cfg(feature = "preview")]
    Forward15 => ("Tiến 15 giây mô phỏng", "Move mock position forward 15 seconds"),
    #[cfg(feature = "preview")]
    AddBookmark => ("Ghi dấu tại vị trí này", "Bookmark this position"),
    #[cfg(feature = "preview")]
    BookmarkAdded => ("Đã ghi dấu tại vị trí này", "This position is bookmarked"),
    #[cfg(feature = "preview")]
    RemoveBookmark => ("Xóa dấu trang", "Remove bookmark"),
    #[cfg(feature = "preview")]
    BookmarkName => ("Nghe tiếp {title} tại {time}", "Continue listening to {title} at {time}"),
    #[cfg(feature = "preview")]
    RemoveBookmarkName => ("Xóa dấu trang của {title} tại {time}", "Remove bookmark for {title} at {time}"),
    #[cfg(feature = "preview")]
    QueueAdd => ("Thêm vào tiếp theo", "Add to queue"),
    #[cfg(feature = "preview")]
    QueueAdded => ("Đã ở trong tiếp theo", "Already queued"),
    #[cfg(feature = "preview")]
    QueueName => ("Thêm vào tiếp theo: {title}", "Add to queue: {title}"),
    #[cfg(feature = "preview")]
    QueueEmpty => ("Chưa có tập tiếp theo", "Your queue is empty"),
    #[cfg(feature = "preview")]
    QueueEmptyHelp => ("Thêm tập mẫu từ danh sách tập. Demo không tự phát tập tiếp theo.", "Add a sample episode from a story. The demo never advances automatically."),
    #[cfg(feature = "preview")]
    QueueRemove => ("Bỏ khỏi tiếp theo", "Remove from queue"),
    #[cfg(feature = "preview")]
    QueueRemoveName => ("Bỏ khỏi tiếp theo: {title}", "Remove from queue: {title}"),
    #[cfg(feature = "preview")]
    Next => ("Mở tập tiếp theo", "Open next episode"),
    #[cfg(feature = "preview")]
    NoNext => ("Chưa có tập trong danh sách tiếp theo", "There are no episodes in your queue"),
    #[cfg(feature = "preview")]
    CloseQueue => ("Đóng tiếp theo, về trình phát", "Close queue and return to player"),
    #[cfg(feature = "preview")]
    CurrentEpisode => ("Tập mẫu đang chọn", "Selected sample episode"),
    #[cfg(feature = "preview")]
    Unavailable => ("Chưa có bản nghe mẫu", "No sample listening edition"),
    #[cfg(feature = "preview")]
    UnavailableHelp => ("Tập minh họa chưa có bản nghe; không thể mở hoặc thêm vào tiếp theo.", "This sample episode has no listening edition and cannot be opened or queued."),
    #[cfg(feature = "preview")]
    Speed => ("Tốc độ mô phỏng", "Mock speed"),
    #[cfg(feature = "preview")]
    SpeedHelp => ("Chỉ đổi giá trị giao diện; không điều khiển âm thanh.", "Changes the display value only; it does not control audio."),
    Theme => ("Giao diện", "Appearance"),
    System => ("Theo hệ thống", "System"),
    Light => ("Sáng", "Light"),
    Dark => ("Tối", "Dark"),
    Locale => ("Ngôn ngữ giao diện", "Interface language"),
    #[cfg(feature = "preview")]
    Scenario => ("Trạng thái danh mục thử nghiệm", "Catalog demo state"),
    #[cfg(feature = "preview")]
    Ready => ("Có dữ liệu", "Ready"),
    #[cfg(feature = "preview")]
    Empty => ("Trống", "Empty"),
    #[cfg(feature = "preview")]
    Error => ("Lỗi", "Error"),
    #[cfg(feature = "preview")]
    DemoTools => ("Điều khiển bản mô phỏng", "Demo controls"),
    #[cfg(feature = "preview")]
    Reset => ("Đặt lại toàn bộ dữ liệu mẫu", "Reset all sample data"),
    #[cfg(feature = "preview")]
    ResetHelp => ("Đặt lại vị trí, tác phẩm lưu, lịch sử, dấu trang, tiếp theo và tốc độ. Xóa tủ sách sẽ xóa tác phẩm lưu, lịch sử, dấu trang và vị trí; vẫn giữ trình phát và tiếp theo. Chỉ ảnh hưởng phiên demo này.", "Reset position, saved stories, history, bookmarks, queue and speed. Clearing the library removes saved stories, history, bookmarks and progress; the player and queue are retained. Only this demo session is affected."),
    #[cfg(feature = "preview")]
    ClearLibrary => ("Xóa nội dung tủ sách mẫu", "Clear sample library contents"),
    #[cfg(feature = "preview")]
    GuestListener => ("Người nghe khách", "Guest listener"),
    #[cfg(feature = "preview")]
    GuestStatus => ("Chưa đăng nhập. Bạn đang nghe với tư cách khách.", "Not signed in. You are listening as a guest."),
    #[cfg(feature = "preview")]
    AccountStatusHeading => ("Trạng thái", "Status"),
    #[cfg(feature = "preview")]
    AccountShortcutsHeading => ("Đi tới", "Go to"),
    #[cfg(feature = "preview")]
    NotIntegrated => ("Chưa tích hợp", "Not integrated"),
    #[cfg(feature = "preview")]
    SignInHelp => ("Bản mô phỏng chưa có đăng nhập, nên chưa có tài khoản để gắn dữ liệu.", "This mock has no sign-in, so there is no account to attach data to."),
    #[cfg(feature = "preview")]
    SyncHelp => ("Chưa đồng bộ giữa thiết bị. Tiến trình, tác phẩm đã lưu và dấu trang chỉ nằm trong phiên này.", "Nothing syncs between devices. Progress, saved stories and bookmarks stay in this session only."),
    #[cfg(feature = "preview")]
    StorageLabel => ("Nơi lưu dữ liệu", "Where data lives"),
    #[cfg(feature = "preview")]
    StorageValue => ("Chỉ trong phiên này", "This session only"),
    #[cfg(feature = "preview")]
    StorageHelp => ("Tải lại trang sẽ đặt lại dữ liệu mẫu.", "Reloading the page resets the sample data."),
    #[cfg(feature = "preview")]
    SettingsLinkHelp => ("Giao diện, ngôn ngữ và tốc độ nghe.", "Appearance, language and listening speed."),
    #[cfg(feature = "preview")]
    LibraryLinkHelp => ("Nghe tiếp, đã lưu và lịch sử.", "Continue, saved and history."),
    #[cfg(feature = "preview")]
    ProgressName => ("Đã nghe {time} trên {total}", "Listened {time} of {total}"),
    #[cfg(feature = "preview")]
    CoverArtPlaceholder => ("Tranh minh họa · bản mô phỏng, chưa xác minh quyền sử dụng", "Illustration · mock, usage rights not verified"),
    SignIn => ("Đăng nhập", "Sign in"),
    SignOut => ("Đăng xuất", "Sign out"),
    AccountChecking => ("Đang kiểm tra phiên người nghe…", "Checking your listener session…"),
    SignedInListener => ("Người nghe đã đăng nhập", "Signed-in listener"),
    LiveGuestStatus => ("Chưa đăng nhập vào Cantos.", "Not signed in to Cantos."),
    #[cfg(feature = "preview")]
    AccountReference => ("Mã tài khoản: {actor}", "Account ID: {actor}"),
    AccountLogoutPending => ("Đang đăng xuất phiên nghe…", "Signing out the listener session…"),
    AccountLogoutRetryHelp => ("Chưa xác nhận đăng xuất. Dữ liệu riêng đã được ẩn; hãy thử hoàn tất đăng xuất trước khi đăng nhập lại.", "Sign-out is not confirmed. Private data is hidden; finish signing out before signing in again."),
    AccountLogoutRetry => ("Hoàn tất đăng xuất", "Finish signing out"),
    AccountUnavailable => ("Chưa kiểm tra được phiên nghe. Hãy thử lại.", "Your listener session could not be checked. Try again."),
    AccountProtocol => ("Phản hồi phiên nghe không hợp lệ. Dữ liệu riêng được giữ ẩn.", "The listener response was invalid. Private data stays hidden."),
    AccountRateLimited => ("Có quá nhiều yêu cầu cùng lúc. Hãy đợi một lúc rồi thử lại.", "Too many requests. Wait a while, then try again."),
    AccountNotFound => ("Dịch vụ tài khoản chưa được cấu hình cho trang này. Bạn vẫn có thể khám phá nội dung công khai.", "The account service is not configured for this site. You can still explore public stories."),
    AccountSessionChanged => ("Phiên nghe đã thay đổi ở cửa sổ khác. Hãy thử lại để xem dữ liệu của tài khoản hiện tại.", "The listener session changed in another window. Try again to view the current account's data."),
    #[cfg(feature = "preview")]
    LiveNotice => ("Tài khoản và dữ liệu người nghe được đọc từ Cantos. Đăng nhập không tự phát âm thanh hoặc chuyển dữ liệu minh họa vào tài khoản.", "Listener accounts and data are read from Cantos. Signing in does not start audio or import sample data."),
    LiveLibrarySignedOut => ("Đăng nhập để xem vị trí nghe tiếp, tác phẩm đã lưu và lịch sử của bạn.", "Sign in to view your listening positions, saved stories and history."),
    LiveLibraryLoading => ("Đang tải dữ liệu của phiên nghe hiện tại…", "Loading data for the current listener session…"),
    LiveLibraryEmpty => ("Tài khoản này chưa có dữ liệu nghe tiếp, tác phẩm đã lưu hoặc lịch sử.", "This account has no listening positions, saved stories or history yet."),
    LiveLibraryPageHelp => ("Tải thêm trong từng phần để xem các mục cũ hơn. Mở tác phẩm không tự phát âm thanh.", "Load more in each section to see older items. Opening a work never starts audio automatically."),
    LiveLibraryCursorExpired => ("Trang đã hết hạn hoặc lịch sử đã thay đổi. Tải lại riêng danh sách này để tiếp tục.", "This page expired or history changed. Reload this list to continue."),
    LiveLibraryEmptyContinue => ("Chưa có vị trí nghe tiếp trong tài khoản này.", "No listening positions in this account yet."),
    LiveLibraryEmptySaved => ("Bạn chưa lưu tác phẩm nào.", "You have no saved stories yet."),
    LiveLibraryEmptyHistory => ("Chưa có lịch sử nghe trong tài khoản này.", "No listening history in this account yet."),
    LiveLibraryLoaded => ("Đang hiển thị {count} mục đã tải.", "Showing {count} loaded items."),
    LiveLibraryRetry => ("Thử tải lại", "Retry page"),
    LiveLibraryRestart => ("Tải lại danh sách", "Reload list"),
    LiveLibraryLoadMore => ("Tải thêm {section}", "Load more {section}"),
    LiveLibraryMetadataUnavailable => ("Thông tin tác phẩm hiện không khả dụng", "Story details are currently unavailable"),
    LiveLibraryPageError => ("Chưa tải được trang này. Các mục đã tải vẫn được giữ; bạn có thể thử lại.", "This page could not be loaded. Previously loaded items are retained; you can retry."),
    LiveLibraryInvalidPage => ("Phản hồi trang không hợp lệ. Hãy tải lại danh sách.", "The page response was invalid. Reload the list."),
    LiveLibraryNoPosition => ("Chưa có vị trí nghe được ghi nhận.", "No listening position recorded yet."),
    LiveLibraryOpenWork => ("Mở tác phẩm", "Open story"),
    LiveLibraryMore => ("Còn dữ liệu ngoài trang đang hiển thị.", "More data exists beyond the displayed page."),
    LivePosition => ("Vị trí {position} trên {total}", "Position {position} of {total}"),
    LivePositionNumber => ("Vị trí nghe tiếp {number}", "Listening position {number}"),
    LiveResumeExact => ("Vị trí thuộc đúng bản nghe đã lưu.", "The position belongs to the saved listening edition."),
    LiveResumeReplaced => ("Bản nghe đã được thay thế; vị trí cũ được giữ nguyên.", "The listening edition was replaced; the original position is retained."),
    LiveResumeUnavailable => ("Bản nghe hiện chưa khả dụng; vị trí của bạn vẫn được giữ.", "The listening edition is currently unavailable; your position is retained."),
    LiveSaveSignIn => ("Đăng nhập để lưu", "Sign in to save"),
    LiveSaveSignInName => ("Đăng nhập để lưu tác phẩm {title}", "Sign in to save story {title}"),
    LiveSaveSaving => ("Đang lưu…", "Saving…"),
    LiveSaveRemoving => ("Đang bỏ lưu…", "Removing from saved…"),
    LiveSaveUnknown => ("Chưa xác nhận được kết quả lưu. Hãy thử kiểm tra lại.", "Could not confirm the save outcome yet. Try checking again."),
    LiveSaveConflict => ("Trạng thái lưu đã thay đổi trên thiết bị khác.", "Saved state changed on another device."),
    LiveSaveError => ("Chưa cập nhật được trạng thái lưu. Hãy thử lại.", "Could not update saved state. Try again."),
    LiveResumeOpen => ("Mở vị trí nghe", "Open listening position"),
    LiveResumeOpenName => ("Mở vị trí nghe {title} tại {time}", "Open listening position for {title} at {time}"),
    LiveResumeRestored => ("Đã khôi phục vị trí nghe tại {time} (đang tạm dừng).", "Restored listening position at {time} (paused)."),
    LiveResumeLoadFailed => ("Chưa tải được thông tin bản phát hành để khôi phục vị trí nghe. Hãy thử lại.", "Could not load the release manifest to restore your position. Try again."),
    LiveResumeCompleted => ("Đã nghe xong", "Completed"),
    LiveMarkCompleted => ("Đánh dấu đã nghe xong", "Mark as completed"),
    LiveRestartFromBeginning => ("Nghe lại từ đầu (0:00)", "Start over from 0:00"),
    LiveProgressPending => ("Đang lưu vị trí nghe…", "Saving listening position…"),
    LiveProgressUnknown => ("Chưa xác nhận được kết quả lưu vị trí nghe.", "Could not confirm listening position save yet."),
    LiveProgressError => ("Chưa lưu được vị trí nghe. Hãy thử lại.", "Could not save listening position. Try again."),
    LiveProgressKeepServer => ("Đã giữ vị trí từ thiết bị khác ({time}).", "Kept the position from another device ({time})."),
    LiveProgressConflictPrompt => ("Vị trí nghe khác nhau giữa thiết bị này ({local}) và máy chủ ({server}). Bạn muốn giữ vị trí nào?", "Listening position differs between this device ({local}) and the server ({server}). Which position do you want to keep?"),
    LiveProgressResolveHere => ("Giữ vị trí tại đây ({time})", "Keep position on this device ({time})"),
    LiveProgressResolveOther => ("Theo vị trí trên máy chủ ({time})", "Use server position ({time})"),
    LiveClearHistory => ("Xóa lịch sử nghe", "Clear listening history"),
    LiveClearHistoryConfirmTitle => ("Xóa toàn bộ lịch sử nghe?", "Clear all listening history?"),
    LiveClearHistoryHelp => ("Chỉ xóa danh sách lịch sử nghe. Truyện đã lưu, tiến độ nghe tiếp và trình phát hiện tại vẫn được giữ nguyên.", "Only the listening history list is cleared. Saved stories, continue-listening progress and the current player are kept."),
    LiveConfirmClearHistory => ("Xác nhận xóa lịch sử", "Confirm clear history"),
    LiveClearHistoryPending => ("Đang xóa lịch sử nghe…", "Clearing listening history…"),
    LiveClearHistoryConflict => ("Lịch sử nghe vừa thay đổi. Hãy kiểm tra và xác nhận lại nếu bạn vẫn muốn xóa.", "Listening history changed. Review and confirm again if you still want to clear it."),
    LiveHistoryCommandUnknown => ("Chưa xác nhận được thao tác lịch sử. Hãy thử lại.", "Could not confirm the history action yet. Try again."),
    LiveHistoryCommandError => ("Chưa thực hiện được thao tác lịch sử. Hãy thử lại.", "Could not complete the history action. Try again."),
    #[cfg(feature = "preview")]
    OpenMock => ("Mở bản xem thử minh họa riêng", "Open the isolated mock preview"),
    #[cfg(feature = "preview")]
    Sync => ("Đồng bộ", "Sync"),
    #[cfg(feature = "preview")]
    Download => ("Tải để nghe ngoại tuyến", "Download for offline listening"),
    #[cfg(feature = "preview")]
    Timer => ("Hẹn giờ dừng", "Sleep timer"),
    #[cfg(feature = "preview")]
    Unsupported => ("Chưa được tích hợp trong bản mô phỏng", "Not integrated in this mock"),
    #[cfg(feature = "preview")]
    NotFoundHelp => ("Đường dẫn này không có trong demo. Dữ liệu phiên của bạn vẫn được giữ.", "This route is not in the demo. Your session data is unchanged."),
    #[cfg(feature = "preview")]
    Preferences => ("Thiết lập cho lần nghe tiếp theo", "Set the mood for your next story"),
    #[cfg(feature = "preview")]
    Footer => ("Cantos · khung giao diện nghe thử, chưa phải trải nghiệm nghe thực", "Cantos · listening UI mock, not a real listening experience"),
}

/// Renders `key` with named values in one pass over the original template. Inserted values are
/// never re-scanned, so a story title such as `Lời hẹn {time}` keeps its braces; the first value
/// wins for a repeated name; a placeholder without a value stays visible instead of vanishing.
pub fn format(key: Key, locale: Locale, values: &[(&str, String)]) -> String {
    interpolate(text(key, locale), values)
}

fn interpolate(template: &str, values: &[(&str, String)]) -> String {
    let mut message = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        message.push_str(&rest[..open]);
        rest = &rest[open..];
        let Some(close) = rest.find('}') else {
            break;
        };
        let name = &rest[1..close];
        match values.iter().find(|(candidate, _)| *candidate == name) {
            Some((_, value)) => message.push_str(value),
            None => message.push_str(&rest[..=close]),
        }
        rest = &rest[close + 1..];
    }
    message.push_str(rest);
    message
}

fn counted(locale: Locale, count: u64, one: Key, other: Key) -> String {
    let key = match locale.plural(count) {
        Plural::One => one,
        Plural::Other => other,
    };
    format(key, locale, &[("count", count.to_string())])
}

pub fn episode_count(locale: Locale, count: usize) -> String {
    let count = u64::try_from(count).unwrap_or(u64::MAX);
    counted(locale, count, Key::EpisodeCountOne, Key::EpisodeCountOther)
}

pub fn spoken_time(locale: Locale, position: u32) -> String {
    format(
        Key::SpokenTime,
        locale,
        &[
            (
                "minutes",
                counted(
                    locale,
                    u64::from(position / 60),
                    Key::MinuteOne,
                    Key::MinuteOther,
                ),
            ),
            (
                "seconds",
                counted(
                    locale,
                    u64::from(position % 60),
                    Key::SecondOne,
                    Key::SecondOther,
                ),
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
        Route::Work(_) | Route::PublicWork(_) => text(Key::AboutWork, locale),
        route => text(route_key(route), locale),
    };
    format(Key::PageTitle, locale, &[("page", page.to_owned())])
}

/// Speed in hundredths with the locale decimal mark and no trailing zeros, the same contract as
/// CMP `ShellCopy.speed`: 75 → `0,75×` / `0.75×`, 100 → `1×`, 150 → `1,5×` / `1.5×`.
pub fn speed_label(locale: Locale, speed: u16) -> String {
    let fraction = format!("{:02}", speed % 100);
    match fraction.trim_end_matches('0') {
        "" => format!("{}×", speed / 100),
        digits => format!("{}{}{digits}×", speed / 100, locale.decimal_separator()),
    }
}

/// Public catalog kinds have one app-owned mapping, independent of either feature.
pub fn work_kind_key(kind: WorkKind) -> Key {
    match kind {
        WorkKind::Novel => Key::PublicWorkKindNovel,
        WorkKind::Story => Key::PublicWorkKindStory,
        WorkKind::Podcast => Key::PublicWorkKindPodcast,
    }
}

#[cfg(feature = "preview")]
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
        Route::Work(_) | Route::PublicWork(_) => Key::AboutWork,
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

#[cfg(feature = "preview")]
pub fn status_key(status: Status) -> Key {
    match status {
        Status::Idle => Key::NoPlayer,
        Status::Loading => Key::PlayerLoading,
        Status::Paused(PauseCause::User) => Key::Paused,
        Status::Paused(PauseCause::Interruption) => Key::Interrupted,
        Status::Paused(PauseCause::ColdRestore) => Key::ResumeOffered,
        Status::Playing => Key::Simulating,
        Status::Buffering => Key::Buffering,
        Status::Ended => Key::Ended,
        Status::Failed(failure) => failure_key(failure),
    }
}

/// The status of a real engine: nothing here is simulated.
pub fn listening_status_key(status: Status) -> Key {
    match status {
        Status::Idle => Key::ListenNothingSelected,
        Status::Loading => Key::ListenPreparing,
        Status::Paused(PauseCause::User) => Key::ListenPaused,
        Status::Paused(PauseCause::Interruption) => Key::ListenInterrupted,
        Status::Paused(PauseCause::ColdRestore) => Key::ListenResumeOffered,
        Status::Playing => Key::ListenPlaying,
        Status::Buffering => Key::ListenBuffering,
        Status::Ended => Key::ListenEnded,
        Status::Failed(failure) => failure_key(failure),
    }
}

pub fn failure_key(failure: Failure) -> Key {
    match failure {
        Failure::Timeout => Key::ErrorTimeout,
        Failure::Unauthenticated => Key::ErrorUnauthenticated,
        Failure::Forbidden => Key::ErrorForbidden,
        Failure::Configuration => Key::ErrorConfiguration,
        Failure::PlayRefused => Key::ListenFailedPlayRefused,
        Failure::Unavailable => Key::ListenFailedUnavailable,
        Failure::Network => Key::ListenFailedNetwork,
        Failure::Decode => Key::ListenFailedDecode,
        Failure::Protocol => Key::ListenFailedProtocol,
        Failure::IdentityChanged => Key::ListenFailedIdentity,
    }
}

pub const fn error_key(kind: crate::application::errors::ErrorKind) -> Key {
    match kind {
        crate::application::errors::ErrorKind::Network => Key::ErrorNetwork,
        crate::application::errors::ErrorKind::Timeout => Key::ErrorTimeout,
        crate::application::errors::ErrorKind::Unauthenticated => Key::ErrorUnauthenticated,
        crate::application::errors::ErrorKind::Forbidden => Key::ErrorForbidden,
        crate::application::errors::ErrorKind::ContentUnavailable => Key::ErrorContentUnavailable,
        crate::application::errors::ErrorKind::Protocol => Key::ErrorProtocol,
        crate::application::errors::ErrorKind::Configuration => Key::ErrorConfiguration,
        crate::application::errors::ErrorKind::Storage => Key::ErrorStorage,
        crate::application::errors::ErrorKind::ServiceUnavailable => Key::ErrorServiceUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn placeholders(value: &str) -> BTreeMap<&str, usize> {
        let mut names = BTreeMap::new();
        for name in value
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|pair| pair.0))
        {
            *names.entry(name).or_default() += 1;
        }
        names
    }

    #[test]
    fn every_resource_has_both_locales_and_matching_parameters() {
        for key in Key::ALL {
            let vi = text(*key, Locale::Vietnamese);
            let en = text(*key, Locale::English);
            assert!(!vi.trim().is_empty() && !en.trim().is_empty(), "{key:?}");
            assert_eq!(placeholders(vi), placeholders(en), "{key:?}");
        }
    }

    #[test]
    fn locale_tags_parse_exactly_and_name_themselves() {
        for (locale, lang, autonym) in [
            (Locale::Vietnamese, "vi-VN", "Tiếng Việt"),
            (Locale::English, "en", "English"),
        ] {
            assert_eq!(locale.lang(), lang);
            assert_eq!(locale.autonym(), autonym);
            assert_eq!(Locale::from_lang(lang), Some(locale));
        }
        for unsupported in ["", "vi", "EN", "en-US", "fr"] {
            assert_eq!(Locale::from_lang(unsupported), None, "{unsupported}");
        }
        assert_eq!(Locale::default(), Locale::Vietnamese);
    }

    fn named(pairs: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
        pairs
            .iter()
            .map(|(name, value)| (*name, (*value).to_owned()))
            .collect()
    }

    #[test]
    fn inserted_titles_keep_their_braces_whatever_the_argument_order() {
        let title = "Lời hẹn {time}";
        for values in [
            named(&[("title", title), ("time", "12:05")]),
            named(&[("time", "12:05"), ("title", title)]),
        ] {
            assert_eq!(
                format(Key::BookmarkName, Locale::Vietnamese, &values),
                "Nghe tiếp Lời hẹn {time} tại 12:05"
            );
            assert_eq!(
                format(Key::BookmarkName, Locale::English, &values),
                "Continue listening to Lời hẹn {time} at 12:05"
            );
        }
        let nested = named(&[("page", "{page} · {title}")]);
        assert_eq!(
            format(Key::PageTitle, Locale::English, &nested),
            "{page} · {title} · Cantos"
        );
    }

    #[test]
    fn repeated_missing_and_unclosed_placeholders_are_deterministic() {
        let values = named(&[("title", "Đầu"), ("title", "Sau"), ("time", "1:00")]);
        assert_eq!(interpolate("{title} · {title}", &values), "Đầu · Đầu");
        assert_eq!(
            format(
                Key::BookmarkName,
                Locale::English,
                &named(&[("title", "A")])
            ),
            "Continue listening to A at {time}"
        );
        assert_eq!(interpolate("{title} {", &values), "Đầu {");
        assert_eq!(interpolate("{} {unknown}", &values), "{} {unknown}");
        assert_eq!(interpolate("", &values), "");
    }

    #[test]
    fn markup_in_values_stays_text_when_rendered() {
        use leptos::prelude::*;
        let label = format(
            Key::SaveName,
            Locale::English,
            &named(&[("title", "<b onclick=\"x()\">Sấm</b> {title}")]),
        );
        assert_eq!(label, "Save story <b onclick=\"x()\">Sấm</b> {title}");
        let html = view! { <button aria-label=label.clone()>{label.clone()}</button> }.to_html();
        assert!(!html.contains("<b "), "{html}");
        assert!(html.contains("&lt;b"), "{html}");
    }

    #[test]
    fn plural_categories_follow_cldr_for_each_locale() {
        for count in [0, 1, 2, 21, 101] {
            assert_eq!(Locale::Vietnamese.plural(count), Plural::Other);
        }
        assert_eq!(Locale::English.plural(0), Plural::Other);
        assert_eq!(Locale::English.plural(1), Plural::One);
        assert_eq!(Locale::English.plural(2), Plural::Other);
        assert_eq!(Locale::English.plural(21), Plural::Other);
        for (count, vi, en) in [
            (0, "0 tập", "0 episodes"),
            (1, "1 tập", "1 episode"),
            (2, "2 tập", "2 episodes"),
        ] {
            assert_eq!(episode_count(Locale::Vietnamese, count), vi);
            assert_eq!(episode_count(Locale::English, count), en);
        }
    }

    /// Same vectors as CMP `DemoCopyTest`; a change here is a cross-renderer product change.
    #[test]
    fn speed_and_spoken_time_match_the_cmp_shell_copy_contract() {
        for (speed, vi, en) in [
            (75, "0,75×", "0.75×"),
            (100, "1×", "1×"),
            (125, "1,25×", "1.25×"),
            (150, "1,5×", "1.5×"),
            (200, "2×", "2×"),
        ] {
            assert_eq!(speed_label(Locale::Vietnamese, speed), vi);
            assert_eq!(speed_label(Locale::English, speed), en);
        }
        for (seconds, vi, en) in [
            (0, "0 phút 0 giây", "0 minutes 0 seconds"),
            (1, "0 phút 1 giây", "0 minutes 1 second"),
            (59, "0 phút 59 giây", "0 minutes 59 seconds"),
            (60, "1 phút 0 giây", "1 minute 0 seconds"),
            (61, "1 phút 1 giây", "1 minute 1 second"),
            (768, "12 phút 48 giây", "12 minutes 48 seconds"),
            (1722, "28 phút 42 giây", "28 minutes 42 seconds"),
        ] {
            assert_eq!(spoken_time(Locale::Vietnamese, seconds), vi);
            assert_eq!(spoken_time(Locale::English, seconds), en);
        }
    }

    #[test]
    fn public_kind_mapping_and_named_templates_have_both_locales() {
        for (kind, key, vi, en) in [
            (
                WorkKind::Novel,
                Key::PublicWorkKindNovel,
                "Tiểu thuyết",
                "Novel",
            ),
            (WorkKind::Story, Key::PublicWorkKindStory, "Truyện", "Story"),
            (
                WorkKind::Podcast,
                Key::PublicWorkKindPodcast,
                "Podcast",
                "Podcast",
            ),
        ] {
            assert_eq!(work_kind_key(kind), key);
            assert_eq!(text(key, Locale::Vietnamese), vi);
            assert_eq!(text(key, Locale::English), en);
        }
        for locale in Locale::ALL {
            for (key, name) in [
                (Key::DiscoveryShowing, "count"),
                (Key::DiscoveryComplete, "count"),
                (Key::PublicWorkUnitCountChapters, "count"),
                (Key::PublicWorkUnitCountEpisodes, "count"),
            ] {
                assert_eq!(placeholders(text(key, locale)), BTreeMap::from([(name, 1)]));
            }
            assert_eq!(
                placeholders(text(Key::PublicWorkUnitLabel, locale)),
                BTreeMap::from([("kind", 1), ("ordinal", 1)])
            );
            assert_eq!(
                placeholders(text(Key::PublicHomeFeatureMeta, locale)),
                BTreeMap::from([("kind", 1), ("language", 1), ("units", 1)])
            );
            assert_eq!(
                placeholders(text(Key::PublicHomeOpenWorkNamed, locale)),
                BTreeMap::from([("title", 1)])
            );
            let label = format(
                Key::CoverArtName,
                locale,
                &named(&[("title", "Lời hẹn {description}"), ("description", "Bìa")]),
            );
            assert_eq!(label, "Lời hẹn {description}: Bìa");
        }
    }
}
