package com.cantos.theatre.presentation.model

/** Localized shell text. Content titles retain their source language. */
data class ShellText(val vi: String, val en: String, val enOne: String? = null)

enum class CopyKey(val vi: String, val en: String, val enOne: String? = null) {
    Brand("Cantos", "Cantos"),
    Discover("Khám phá", "Discover"),
    Search("Tìm kiếm", "Search"),
    Library("Của tôi", "My library"),
    Settings("Cài đặt nghe", "Listening settings"),
    Back("Quay lại", "Back"),
    ClosePlayer("Thu gọn phòng nghe", "Close listening room"),
    Work("Tác phẩm", "Work"),
    Player("Phòng nghe", "Listening room"),
    Queue("Tiếp theo", "Up next"),
    Welcome("Câu chuyện vẫn\nđang đợi bạn.", "A story is\nwaiting for you."),
    ContinuingHeading("CHUYỆN CÒN DỞ", "YOUR UNFINISHED STORY"),
    DiscoverHeading("Một thế giới để nghe", "A world to listen to"),
    ExploreHeading("Những miền thanh âm", "More worlds of sound"),
    Continue("Nghe tiếp", "Continue listening"),
    Details("Khám phá tác phẩm {title}", "Explore {title}"),
    EpisodesCount("{count} tập · Tiếng Việt", "{count} episodes · Vietnamese", "{count} episode · Vietnamese"),
    ChaptersCount("{count} chương", "{count} chapters", "{count} chapter"),
    EpisodeLabel("Tập {index} · {title}", "Episode {index} · {title}"),
    Duration("{minutes} phút", "{minutes} minutes", "{minutes} minute"),
    Done("Xong", "Done"),
    TimerSimulationNote("Chọn thời lượng hẹn giờ hoặc thời điểm hết tập.", "Choose a timer duration or the end of the episode."),
    StartEpisode("Mở tập đầu", "Open first episode"),
    Episodes("Các tập", "Episodes"),
    PlayEpisode("Phát {title}", "Play {title}"),
    PauseEpisode("Tạm dừng {title}", "Pause {title}"),
    OpenPlayer("Mở phòng nghe: {title}", "Open listening room: {title}"),
    Save("Lưu tác phẩm", "Save work"),
    SaveName("Lưu tác phẩm {title}", "Save work {title}"),
    UnsaveName("Bỏ lưu tác phẩm {title}", "Unsave work {title}"),
    Saved("Đã lưu · Bỏ lưu", "Saved · Unsave"),
    SavedOnlyHere("Tác phẩm đã lưu xuất hiện trong tủ sách của bạn.", "Saved works appear in your library."),
    LibrarySession("Các tác phẩm đã lưu, điểm nghe và dấu trang của bạn.", "Your saved works, listening positions and bookmarks."),
    ResumeSample("Mở vị trí nghe", "Open listening position"),
    ResumeSampleName("Mở vị trí nghe {title} tại {time}", "Open listening position for {title} at {time}"),
    ResumeSampleHelp("Mở lại đúng vị trí và giữ tạm dừng. Chọn Phát để nghe tiếp.", "Open the saved position paused. Choose Play to continue listening."),
    NoContinue("Chưa có câu chuyện để nghe tiếp", "No stories to continue yet"),
    NoContinueHelp("Mở một tập và bắt đầu nghe. Bạn có thể trở lại từ đây để nghe tiếp.", "Open an episode and start listening. Return here to continue."),
    NoSaved("Chưa lưu câu chuyện nào", "No saved stories yet"),
    NoSavedHelp("Chọn Lưu trên câu chuyện bạn thích. Tác phẩm sẽ xuất hiện ngay tại đây.", "Choose Save on a story you like. It will appear here straight away."),
    History("Lịch sử", "History"),
    NoHistory("Chưa có lịch sử nghe", "No listening history yet"),
    NoHistoryHelp("Lịch sử ghi nhận các tập đã phát; chỉ mở trang hoặc lưu truyện chưa tạo lịch sử.", "History records episodes after playback; opening a page or saving a story does not add history."),
    HistoryHelp("Các câu chuyện đã phát, gần nhất ở trên. Mở lại sẽ khôi phục vị trí và giữ tạm dừng.", "Stories played, most recent first. Reopening restores the position and stays paused."),
    ViewAllHistory("Xem tất cả lịch sử nghe", "View all listening history"),
    ClearHistory("Xóa lịch sử nghe", "Clear listening history"),
    ClearHistoryConfirmTitle("Xóa lịch sử nghe?", "Clear listening history?"),
    ClearHistoryHelp("Chỉ xóa danh sách lịch sử. Vị trí nghe, truyện đã lưu, dấu trang và trình phát vẫn được giữ lại.", "Only the history list is cleared. Listening positions, saved stories, bookmarks and the player are retained."),
    ConfirmClearHistory("Xác nhận xóa lịch sử", "Confirm clear history"),
    Cancel("Hủy", "Cancel"),
    Account("Tài khoản", "Account"),
    MembershipPreview("Membership", "Membership"),
    AccountSpace("Không gian của bạn", "Your space"),
    // Route titles only: the account/auth screens own their copy under features/auth.
    SignIn("Đăng nhập", "Sign in"),
    CreateAccount("Tạo tài khoản", "Create an account"),
    SearchLabel("Tên tác phẩm, tập hoặc thể loại", "Work, episode or genre"),
    SearchHint("Tìm một câu chuyện để nghe", "Find a story to listen to"),
    SearchClear("Xóa tìm kiếm", "Clear search"),
    Results("{count} tác phẩm", "{count} works", "{count} work"),
    Loading("Đang tải danh sách…", "Loading stories…"),
    LoadingNote("Danh sách sẽ xuất hiện khi tải xong.", "Stories will appear when loading finishes."),
    Empty("Chưa tìm thấy câu chuyện", "No stories found"),
    EmptyNote("Đổi từ khóa hoặc trở về khám phá để thử lại.", "Change your search or return to discovery."),
    Error("Chưa mở được danh sách", "The list could not open"),
    ErrorNote("Thử lại để tải danh sách. Tập và điểm nghe của bạn vẫn được giữ.", "Try loading the list again. Your episode and position are kept."),
    Retry("Thử lại", "Retry"),
    ListeningStoreFailure("Chưa lưu được thay đổi. Dữ liệu trước đó được giữ lại.", "The change could not be saved. Previous data is retained."),
    RetryListening("Thử lại thao tác thất bại gần nhất", "Retry the last failed action"),
    Ready("Bình thường", "Ready"),
    Continuing("Nghe tiếp", "Continue listening"),
    SavedTab("Đã lưu", "Saved"),
    Bookmarks("Dấu trang", "Bookmarks"),
    LibraryEmpty("Tủ sách đang chờ bạn", "Your library is waiting"),
    LibraryEmptyNote("Tìm một câu chuyện trong Khám phá. Tủ sách sẽ lớn dần khi bạn nghe và lưu lại.", "Find a story in Discover. Your library grows as you listen and save."),
    ContinueEmptySummary("Trở lại đúng điểm bạn đã dừng nghe.", "Return to where you stopped listening."),
    SavedEmptySummary("Giữ những câu chuyện bạn muốn trở lại.", "Keep the stories you want to return to."),
    BookmarksEmptySummary("Ghi dấu khoảnh khắc trong phòng nghe.", "Bookmark a moment in the listening room."),
    HistoryEmptySummary("Các tập đã phát sẽ xuất hiện ở đây.", "Episodes you have played will appear here."),
    OpenBookmark("Mở dấu trang {title}, {time}", "Open bookmark for {title}, {time}"),
    BookmarksUnavailable("Dấu trang chưa khả dụng trên thiết bị này.", "Bookmarks are unavailable on this device."),
    NoBookmarks("Chưa có dấu trang", "No bookmarks yet"),
    NoBookmarksHelp("Trong phòng nghe, chọn “Ghi dấu điểm này” để giữ đúng một thời điểm của bản thu.", "In the listening room, choose “Bookmark this moment” to keep an exact point of the recording."),
    DeleteBookmark("Xóa", "Delete"),
    DiscardBookmark("Bỏ dấu trang", "Discard bookmark"),
    DeleteBookmarkName("Xóa dấu trang {title}, {time}", "Delete bookmark for {title}, {time}"),
    DiscardBookmarkName("Bỏ dấu trang {title}, {time}", "Discard bookmark for {title}, {time}"),
    BookmarkOffline("Chưa có mạng. Dấu trang sẽ đồng bộ khi kết nối lại.", "Offline. The bookmark will sync when you reconnect."),
    BookmarkFailed("Dấu trang chưa được máy chủ nhận. Bạn có thể bỏ nó.", "The server didn’t accept this bookmark. You can discard it."),
    BookmarkDeleteFailed("Chưa xóa được dấu trang này. Dấu trang vẫn còn; bạn có thể thử xóa lại.", "This bookmark couldn’t be deleted yet. It is still saved; you can try again."),
    RetryDeleteBookmark("Thử xóa lại", "Try deleting again"),
    RetryDeleteBookmarkName("Thử xóa lại dấu trang {title}, {time}", "Try deleting bookmark for {title}, {time} again"),
    BookmarkCaption("{title} · {time}", "{title} · {time}"),
    SpeedCaption("Tốc độ · {speed}", "Speed · {speed}"),
    CachedCatalog("Danh sách đã lưu trên máy. Chọn Làm mới để kiểm tra bản hiện tại.", "Cached catalog. Choose Refresh to check the current catalog."),
    PlaybackPlaying("Đang phát", "Playing"),
    PlaybackFailed("Chưa phát được âm thanh. Thử Phát để xin lại quyền cho đúng bản thu.", "Audio is unavailable. Choose Play to retry this recording."),
    FullPlayer("Mở trình phát", "Open player"),
    SyncPending("Đang chờ đồng bộ", "Pending sync"),
    SyncConfirmed("Đã đồng bộ", "Synced"),
    SyncFailed("Đồng bộ chưa thành công", "Sync failed"),
    LibrarySignIn("Đăng nhập để lưu và đồng bộ tủ sách, lịch sử và điểm nghe.", "Sign in to save and sync your library, history and listening positions."),
    ProgressConflict("Điểm nghe trên thiết bị khác đã thay đổi. Chọn điểm muốn lưu; trình phát vẫn giữ nguyên.", "Another device changed the listening position. Choose which position to save; playback stays where it is."),
    KeepHere("Giữ điểm trên máy này", "Keep this device’s position"),
    UseOther("Giữ điểm trên thiết bị khác", "Keep the other device’s position"),
    ResumeNeedsRefresh("Điểm nghe đã được lưu. Làm mới để kiểm tra quyền nghe bản thu này.", "Your position is saved. Refresh to check access to this recording."),
    LibrarySectionFailed("Chưa tải được mục này. Bạn có thể thử lại.", "Couldn’t load this section. You can try again."),
    LibrarySectionStale("Chưa làm mới được. Đang hiện dữ liệu đã lưu trên máy.", "Couldn’t refresh. Showing data saved on this device."),
    SavedUndescribed("Tác phẩm này hiện chưa được mở công khai. Mục đã lưu của bạn vẫn được giữ.", "This work isn’t publicly available right now. Your saved item is kept."),
    ReleaseUnavailable("Bản thu này đã thay đổi hoặc không còn được phép nghe. Điểm nghe vẫn được giữ.", "This recording has changed or is no longer available. Your position is retained."),
    PlaybackSimulating("Đang mô phỏng · Không phát âm thanh", "Simulating · No audio output"),
    PlaybackPaused("Đang tạm dừng", "Paused"),
    PlaybackIdle("Chọn một tập để nghe", "Choose an episode to listen to"),
    PlaybackLoading("Đang mở tập…", "Opening episode…"),
    PlaybackBuffering("Đang tải âm thanh…", "Buffering…"),
    PlaybackEnded("Đã hết tập", "Episode ended"),
    SelectionUnavailable("Chưa có nội dung cho lựa chọn này", "No content for this selection"),
    SelectionUnavailableNote("Thông tin hiện tại chưa đủ để mở lựa chọn này. Trở về khám phá để chọn lại.", "The current metadata is insufficient to open this selection. Return to discovery to choose again."),
    Play("Phát", "Play"),
    Pause("Tạm dừng", "Pause"),
    Position("Điểm nghe", "Listening position"),
    PositionSpoken("{elapsed} trên {total}", "{elapsed} of {total}"),
    PositionCaption("{elapsed} / {total}", "{elapsed} / {total}"),
    SecondsSpoken("{minutes} {seconds}", "{minutes} {seconds}"),
    MinuteOne("{count} phút", "{count} minute"),
    MinutesOther("{count} phút", "{count} minutes"),
    SecondOne("{count} giây", "{count} second"),
    SecondsOther("{count} giây", "{count} seconds"),
    SeekStep("15 giây", "15 seconds"),
    Rewind("Lùi 15 giây", "Back 15 seconds"),
    Forward("Tiến 15 giây", "Forward 15 seconds"),
    Bookmark("Ghi dấu điểm này", "Bookmark this moment"),
    Bookmarked("Đã ghi dấu điểm này", "Moment bookmarked"),
    PlayerExplanation("Chọn Phát để bắt đầu nghe hoặc điều chỉnh điểm nghe.", "Choose Play to listen or adjust the listening position."),
    NextExplanation("Chọn một tập trong danh sách để mở trong phòng nghe.", "Choose an episode from the list to open in the listening room."),
    CurrentEpisode("Tập đang mở", "Current episode"),
    PersonalRhythm("Một nhịp nghe riêng", "Your own listening rhythm"),
    PreferencesNote("Điều chỉnh giao diện, ngôn ngữ và tùy chọn nghe.", "Adjust appearance, language and listening preferences."),
    Theme("Giao diện", "Appearance"),
    Light("Giấy ngà · Sáng", "Warm paper · Light"),
    Dark("Mực tím · Tối", "Purple ink · Dark"),
    Language("Ngôn ngữ giao diện", "Interface language"),
    // Language choices use endonyms so either locale can find its own name.
    Vietnamese("Tiếng Việt", "Tiếng Việt"),
    English("English", "English"),
    LocaleSaveFailure("Không lưu được ngôn ngữ. Lựa chọn trước đó vẫn được giữ. Hãy thử lại.", "Couldn’t save the language. Your previous choice is unchanged. Try again."),
    Speed("Tốc độ phát", "Playback speed"),
    Timer("Hẹn giờ dừng", "Sleep timer"),
    TimerOff("Tắt", "Off"),
    Timer15("15 phút", "15 minutes"),
    Timer30("30 phút", "30 minutes"),
    TimerEnd("Hết tập này", "End of episode"),
    // Live public catalog (production host). Failures never name a reason the server did not give.
    Refresh("Làm mới", "Refresh"),
    LoadMore("Tải thêm", "Load more"),
    LoadingMore("Đang tải thêm…", "Loading more…"),
    NextPageFailed("Chưa tải thêm được. Các mục đã hiện vẫn được giữ.", "Couldn’t load more. Items already shown are kept."),
    WorkNotFound("Tác phẩm này hiện không mở được", "This work can’t be opened right now"),
    WorkNotFoundNote("Quay lại danh sách để chọn một tác phẩm khác.", "Go back to the list to choose another work."),
    KindNovel("Tiểu thuyết", "Novel"),
    KindStory("Truyện", "Story"),
    KindPodcast("Podcast", "Podcast"),
    Chapters("Các chương", "Chapters"),
    // Standard error window (#298): closed vocabulary, never raw exception, SQL or provider text.
    ErrorDialogTitle("Chưa hoàn tất thao tác", "Couldn’t complete that"),
    CloseError("Đóng", "Close"),
    ErrorNetwork("Chưa kết nối được với máy chủ. Hãy kiểm tra mạng và thử lại.", "Couldn’t reach the server. Check your connection and try again."),
    ErrorTimeout("Phản hồi từ máy chủ quá thời gian chờ. Hãy thử lại.", "The server took too long to respond. Try again."),
    ErrorUnauthenticated("Cần đăng nhập để thực hiện thao tác này.", "Sign in to continue with this action."),
    ErrorForbidden("Tài khoản hiện tại không có quyền thực hiện thao tác này.", "Your account does not have access to this action."),
    ErrorContentUnavailable("Nội dung này đã thay đổi hoặc không còn mở công khai.", "This content has changed or is no longer available."),
    ErrorProtocol("Phản hồi từ máy chủ không đúng định dạng mong đợi. Hãy thử lại.", "The server returned an unexpected response. Try again."),
    ErrorConfiguration("Cấu hình máy chủ của bản dựng này không hợp lệ.", "This build’s server configuration is invalid."),
    ErrorStorage("Chưa truy cập được dữ liệu trên thiết bị này. Các thao tác đang chờ được giữ lại; Cantos chưa xác nhận đã đồng bộ.", "Local data is unavailable. Pending actions are kept; sync has not been confirmed."),
    ErrorServiceUnavailable("Dịch vụ đang tạm thời gián đoạn. Hãy thử lại sau ít phút.", "The service is temporarily unavailable. Try again in a moment."),
}

class ShellCopy(
    private val locale: ShellLocale,
    overrides: Map<CopyKey, ShellText> = emptyMap(),
) {
    private val overridesSnapshot = overrides.toMap()

    fun text(key: CopyKey, vararg parameters: Pair<String, String>): String {
        val resource = overridesSnapshot[key]
        val quantityName = when (key) {
            CopyKey.EpisodesCount, CopyKey.ChaptersCount, CopyKey.Results -> "count"
            CopyKey.Duration -> "minutes"
            else -> null
        }
        // Current callers supply nonnegative integer quantities. CLDR English uses one for
        // integer 1 (not visible fractions such as 1.0); Vietnamese has only other.
        val englishOne = quantityName != null && parameters.firstOrNull { it.first == quantityName }?.second == "1"
        val template = when {
            locale == ShellLocale.Vietnamese -> resource?.vi ?: key.vi
            englishOne && resource != null -> resource.enOne ?: resource.en
            englishOne -> key.enOne ?: key.en
            else -> resource?.en ?: key.en
        }
        // Match the original resource once so a supplied title containing braces stays literal.
        return parameterPattern.replace(template) { match ->
            parameters.firstOrNull { (name, _) -> name == match.groupValues[1] }?.second ?: match.value
        }
    }

    fun speed(percent: Int): String {
        val whole = percent / 100
        val fraction = (percent % 100).toString().padStart(2, '0').trimEnd('0')
        val separator = if (locale == ShellLocale.Vietnamese) "," else "."
        return if (fraction.isEmpty()) "$whole×" else "$whole$separator$fraction×"
    }

    fun spokenTime(seconds: Int): String {
        val minutes = seconds / 60
        val remainder = seconds % 60
        return text(
            CopyKey.SecondsSpoken,
            "minutes" to text(if (minutes == 1) CopyKey.MinuteOne else CopyKey.MinutesOther, "count" to minutes.toString()),
            "seconds" to text(if (remainder == 1) CopyKey.SecondOne else CopyKey.SecondsOther, "count" to remainder.toString()),
        )
    }
}

private val parameterPattern = Regex("\\{([^{}]+)\\}")

fun clockTime(seconds: Int): String = "${seconds / 60}:${(seconds % 60).toString().padStart(2, '0')}"
