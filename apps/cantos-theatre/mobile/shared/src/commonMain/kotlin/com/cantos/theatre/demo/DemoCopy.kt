package com.cantos.theatre.demo

/** Demo-owned mock resources. Content titles remain Vietnamese in either UI locale. */
internal enum class CopyKey(val vi: String, val en: String) {
    Brand("Cantos", "Cantos"),
    DemoBadge("BẢN XEM THỬ", "DEMO"),
    DemoNotice("Dữ liệu minh họa · Không có âm thanh thật", "Sample data · No real audio"),
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
    EpisodesCount("{count} tập minh họa · Tiếng Việt", "{count} sample episodes · Vietnamese"),
    EpisodeLabel("Tập {index} · {title}", "Episode {index} · {title}"),
    Duration("{minutes} phút", "{minutes} minutes"),
    DemoTools("Công cụ bản xem thử", "Demo tools"),
    Done("Xong", "Done"),
    TimerSimulationNote("Chỉ chọn trạng thái minh họa; không chạy đếm ngược hoặc tự dừng audio.", "Select a demo state only; no countdown or automatic audio stop runs."),
    StartDemo("Mở tập đầu", "Open first episode"),
    Episodes("Các tập", "Episodes"),
    PlayEpisode("Mô phỏng phát {title}", "Simulate playing {title}"),
    PauseEpisode("Dừng mô phỏng {title}", "Stop simulation for {title}"),
    OpenPlayer("Mở phòng nghe: {title}", "Open listening room: {title}"),
    Save("Lưu tác phẩm", "Save work"),
    SaveName("Lưu tác phẩm {title}", "Save work {title}"),
    UnsaveName("Bỏ lưu tác phẩm {title}", "Unsave work {title}"),
    Saved("Đã lưu · Bỏ lưu", "Saved · Unsave"),
    SavedOnlyHere("Chỉ lưu trong phiên xem thử này.", "Saved only in this demo session."),
    LibrarySession("Tủ sách mẫu chỉ lưu trong bộ nhớ phiên này. Chưa có tài khoản hoặc đồng bộ; khởi động lại sẽ đặt lại dữ liệu.", "This sample library lives in session memory. Account and sync are unavailable; restarting resets the data."),
    ResumeSample("Mở vị trí nghe mẫu", "Open sample position"),
    ResumeSampleName("Mở vị trí nghe mẫu {title} tại {time}", "Open sample position for {title} at {time}"),
    ResumeSampleHelp("Mở lại đúng vị trí và giữ tạm dừng. Chọn Mô phỏng phát để tiếp tục thử.", "Open the saved position paused. Choose Simulate play to continue the demo."),
    NoContinue("Chưa có câu chuyện để nghe tiếp", "No stories to continue yet"),
    NoContinueHelp("Mở một tập mẫu và thử vị trí nghe. Bạn có thể trở lại từ đây trong cùng phiên.", "Open a sample episode and try a listening position. Return here during this session to pick it up again."),
    NoSaved("Chưa lưu câu chuyện nào", "No saved stories yet"),
    NoSavedHelp("Chọn Lưu trên câu chuyện bạn thích. Tác phẩm sẽ xuất hiện ngay tại đây.", "Choose Save on a story you like. It will appear here straight away."),
    History("Lịch sử", "History"),
    NoHistory("Chưa có lịch sử nghe", "No listening history yet"),
    NoHistoryHelp("Lịch sử ghi nhận các tập đã mô phỏng phát trong phiên này; chỉ mở trang hoặc lưu truyện chưa tạo lịch sử.", "History records episodes after simulated play in this session; opening a page or saving a story does not add history."),
    HistoryHelp("Các câu chuyện đã phát mô phỏng, gần nhất ở trên. Mở lại sẽ khôi phục vị trí và giữ tạm dừng.", "Stories played in the simulation, most recent first. Reopening restores the position and stays paused."),
    ViewAllHistory("Xem tất cả lịch sử nghe", "View all listening history"),
    ClearHistory("Xóa lịch sử nghe mẫu", "Clear sample listening history"),
    ClearHistoryConfirmTitle("Xóa lịch sử trong phiên này?", "Clear history for this session?"),
    ClearHistoryHelp("Chỉ xóa danh sách lịch sử. Vị trí nghe, truyện đã lưu, dấu trang và trình phát vẫn được giữ lại.", "Only the history list is cleared. Listening positions, saved stories, bookmarks and the player are retained."),
    ConfirmClearHistory("Xác nhận xóa lịch sử", "Confirm clear history"),
    Cancel("Hủy", "Cancel"),
    Account("Tài khoản", "Account"),
    MembershipPreview("Membership · bản xem thử", "Membership · preview"),
    AccountSpace("Không gian của bạn", "Your space"),
    // Route titles only: the account/auth screens own their copy under features/auth.
    SignIn("Đăng nhập", "Sign in"),
    CreateAccount("Tạo tài khoản", "Create an account"),
    SearchLabel("Tên tác phẩm, tập hoặc thể loại", "Work, episode or genre"),
    SearchHint("Thử tìm Sơn Hà Ký", "Try Sơn Hà Ký"),
    SearchClear("Xóa tìm kiếm", "Clear search"),
    Results("{count} tác phẩm minh họa", "{count} sample works"),
    Loading("Đang tải bản minh họa…", "Loading the demo…"),
    LoadingNote("Trạng thái tải được chọn thủ công; không có yêu cầu mạng.", "Loading is selected manually; no network request is running."),
    Empty("Chưa tìm thấy câu chuyện", "No stories found"),
    EmptyNote("Đổi từ khóa hoặc trở về khám phá để thử lại.", "Change your search or return to discovery."),
    Error("Chưa mở được danh sách minh họa", "The sample list could not open"),
    ErrorNote("Đây là lỗi mô phỏng. Tập và điểm nghe của bạn vẫn được giữ.", "This is a simulated error. Your episode and position are kept."),
    Retry("Thử lại bản minh họa", "Retry the demo"),
    ListeningStoreFailure("Chưa lưu được thay đổi trong bộ nhớ. Dữ liệu trước đó được giữ lại.", "The change could not be saved in memory. Previous data is retained."),
    RetryListening("Thử lại thao tác thất bại gần nhất", "Retry the last failed action"),
    Ready("Bình thường", "Ready"),
    LoadingMode("Đang tải", "Loading"),
    EmptyMode("Rỗng", "Empty"),
    ErrorMode("Lỗi", "Error"),
    Continuing("Nghe tiếp", "Continue listening"),
    SavedTab("Đã lưu", "Saved"),
    Bookmarks("Dấu trang", "Bookmarks"),
    LibraryEmpty("Tủ sách đang chờ bạn", "Your library is waiting"),
    LibraryEmptyNote("Lưu một tác phẩm hoặc ghi dấu một điểm trong phòng nghe.", "Save a work or bookmark a moment in the listening room."),
    OpenBookmark("Mở dấu trang {title}, {time}", "Open bookmark for {title}, {time}"),
    BookmarkCaption("{title} · {time}", "{title} · {time}"),
    SpeedCaption("Tốc độ minh họa · {speed}", "Demo speed · {speed}"),
    MockPlaying("Đang mô phỏng · Không phát âm thanh", "Simulating · No audio output"),
    MockPaused("Mô phỏng đang dừng", "Simulation stopped"),
    MockIdle("Chưa mở mô phỏng · Không có âm thanh thật", "Simulation not opened · No real audio"),
    MockLoading("Đang mở mô phỏng · Không có âm thanh thật", "Opening simulation · No real audio"),
    MockBuffering("Mô phỏng đang chờ · Không có âm thanh thật", "Simulation waiting · No real audio"),
    MockEnded("Mô phỏng đã kết thúc · Không có âm thanh thật", "Simulation ended · No real audio"),
    SelectionUnavailable("Chưa có nội dung minh họa cho lựa chọn này", "No sample content for this selection"),
    SelectionUnavailableNote("Thông tin hiện tại chưa đủ để mở lựa chọn này. Trở về khám phá để chọn lại.", "The current metadata is insufficient to open this selection. Return to discovery to choose again."),
    Play("Mô phỏng phát", "Simulate play"),
    Pause("Dừng mô phỏng", "Stop simulation"),
    Position("Điểm nghe minh họa", "Demo position"),
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
    PlayerExplanation("Điểm nghe chỉ đổi khi bạn thao tác. Không có bộ phát, audio, tải xuống hay đồng bộ thật.", "Position changes only when you interact. No player engine, audio, downloads or sync are connected."),
    NextExplanation("Chọn tập để đổi nội dung minh họa. Không tự phát tập tiếp theo.", "Choose an episode to change the demo. Episodes do not advance automatically."),
    CurrentEpisode("Tập đang mở", "Current episode"),
    PersonalRhythm("Một nhịp nghe riêng", "Your own listening rhythm"),
    PreferencesNote("Tùy chọn chỉ tồn tại trong bộ nhớ. Tốc độ và hẹn giờ không điều khiển audio hoặc đếm ngược.", "Preferences live in memory. Speed and timer do not control audio or run a countdown."),
    Theme("Giao diện", "Appearance"),
    Light("Giấy ngà · Sáng", "Warm paper · Light"),
    Dark("Mực tím · Tối", "Purple ink · Dark"),
    Language("Ngôn ngữ giao diện", "Interface language"),
    // Language choices use endonyms so either locale can find its own name.
    Vietnamese("Tiếng Việt", "Tiếng Việt"),
    English("English", "English"),
    Speed("Tốc độ minh họa", "Demo speed"),
    Timer("Hẹn giờ minh họa", "Demo timer"),
    TimerOff("Tắt", "Off"),
    Timer15("15 phút", "15 minutes"),
    Timer30("30 phút", "30 minutes"),
    TimerEnd("Hết tập này", "End of episode"),
    CatalogState("Trạng thái khám phá", "Discovery scenario"),
    CatalogStateNote("Chọn một trạng thái, rồi trở về Khám phá hoặc Tìm kiếm.", "Choose a state, then return to Discover or Search."),
    Reset("Đặt lại bản xem thử", "Reset demo"),
    ResetNote("Khôi phục nội dung, điểm nghe, dấu trang và tùy chọn ban đầu.", "Restore initial content, position, bookmarks and preferences."),
    Unsupported("Chưa hỗ trợ trong bản xem thử", "Unavailable in this demo"),
    IntegrationsPending("Đăng nhập, tải xuống, audio nền và đồng bộ sẽ cần adapter native và API riêng.", "Sign-in, downloads, background audio and sync need native adapters and their own API."),
}

internal class DemoCopy(private val locale: DemoLocale) {
    fun text(key: CopyKey, vararg parameters: Pair<String, String>): String {
        val template = if (locale == DemoLocale.Vietnamese) key.vi else key.en
        // Match the original resource once so a supplied title containing braces stays literal.
        return parameterPattern.replace(template) { match ->
            parameters.firstOrNull { (name, _) -> name == match.groupValues[1] }?.second ?: match.value
        }
    }

    fun speed(percent: Int): String {
        val whole = percent / 100
        val fraction = (percent % 100).toString().padStart(2, '0').trimEnd('0')
        val separator = if (locale == DemoLocale.Vietnamese) "," else "."
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

internal fun clockTime(seconds: Int): String = "${seconds / 60}:${(seconds % 60).toString().padStart(2, '0')}"
