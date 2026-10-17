package com.cantos.theatre.app

import com.cantos.theatre.presentation.model.ShellLocale
import com.cantos.theatre.presentation.model.CopyKey
import com.cantos.theatre.presentation.model.ShellText

/** Exact diagnostic wording supplied only by the sandbox composition. */
val DemoShellCopyOverrides: Map<CopyKey, ShellText> = mapOf(
    CopyKey.EpisodesCount to ShellText("{count} tập minh họa · Tiếng Việt", "{count} sample episodes · Vietnamese", "{count} sample episode · Vietnamese"),
    CopyKey.TimerSimulationNote to ShellText("Chỉ chọn trạng thái minh họa; không chạy đếm ngược hoặc tự dừng audio.", "Select a demo state only; no countdown or automatic audio stop runs."),
    CopyKey.PlayEpisode to ShellText("Mô phỏng phát {title}", "Simulate playing {title}"),
    CopyKey.PauseEpisode to ShellText("Dừng mô phỏng {title}", "Stop simulation for {title}"),
    CopyKey.SavedOnlyHere to ShellText("Chỉ lưu trong phiên xem thử này.", "Saved only in this demo session."),
    CopyKey.LibrarySession to ShellText("Tủ sách mẫu chỉ lưu trong bộ nhớ phiên này. Chưa có tài khoản hoặc đồng bộ; khởi động lại sẽ đặt lại dữ liệu.", "This sample library lives in session memory. Account and sync are unavailable; restarting resets the data."),
    CopyKey.ResumeSample to ShellText("Mở vị trí nghe mẫu", "Open sample position"),
    CopyKey.ResumeSampleName to ShellText("Mở vị trí nghe mẫu {title} tại {time}", "Open sample position for {title} at {time}"),
    CopyKey.ResumeSampleHelp to ShellText("Mở lại đúng vị trí và giữ tạm dừng. Chọn Mô phỏng phát để tiếp tục thử.", "Open the saved position paused. Choose Simulate play to continue the demo."),
    CopyKey.NoContinueHelp to ShellText("Mở một tập mẫu và thử vị trí nghe. Bạn có thể trở lại từ đây trong cùng phiên.", "Open a sample episode and try a listening position. Return here during this session to pick it up again."),
    CopyKey.NoHistoryHelp to ShellText("Lịch sử ghi nhận các tập đã mô phỏng phát trong phiên này; chỉ mở trang hoặc lưu truyện chưa tạo lịch sử.", "History records episodes after simulated play in this session; opening a page or saving a story does not add history."),
    CopyKey.HistoryHelp to ShellText("Các câu chuyện đã phát mô phỏng, gần nhất ở trên. Mở lại sẽ khôi phục vị trí và giữ tạm dừng.", "Stories played in the simulation, most recent first. Reopening restores the position and stays paused."),
    CopyKey.ClearHistory to ShellText("Xóa lịch sử nghe mẫu", "Clear sample listening history"),
    CopyKey.ClearHistoryConfirmTitle to ShellText("Xóa lịch sử trong phiên này?", "Clear history for this session?"),
    CopyKey.MembershipPreview to ShellText("Membership · bản xem thử", "Membership · preview"),
    CopyKey.SearchHint to ShellText("Thử tìm Sơn Hà Ký", "Try Sơn Hà Ký"),
    CopyKey.Results to ShellText("{count} tác phẩm minh họa", "{count} sample works", "{count} sample work"),
    CopyKey.Loading to ShellText("Đang tải bản minh họa…", "Loading the demo…"),
    CopyKey.LoadingNote to ShellText("Trạng thái tải được chọn thủ công; không có yêu cầu mạng.", "Loading is selected manually; no network request is running."),
    CopyKey.Error to ShellText("Chưa mở được danh sách minh họa", "The sample list could not open"),
    CopyKey.ErrorNote to ShellText("Đây là lỗi mô phỏng. Tập và điểm nghe của bạn vẫn được giữ.", "This is a simulated error. Your episode and position are kept."),
    CopyKey.Retry to ShellText("Thử lại bản minh họa", "Retry the demo"),
    CopyKey.ListeningStoreFailure to ShellText("Chưa lưu được thay đổi trong bộ nhớ. Dữ liệu trước đó được giữ lại.", "The change could not be saved in memory. Previous data is retained."),
    CopyKey.SpeedCaption to ShellText("Tốc độ minh họa · {speed}", "Demo speed · {speed}"),
    CopyKey.PlaybackPaused to ShellText("Mô phỏng đang dừng", "Simulation stopped"),
    CopyKey.PlaybackIdle to ShellText("Chưa mở mô phỏng · Không có âm thanh thật", "Simulation not opened · No real audio"),
    CopyKey.PlaybackLoading to ShellText("Đang mở mô phỏng · Không có âm thanh thật", "Opening simulation · No real audio"),
    CopyKey.PlaybackBuffering to ShellText("Mô phỏng đang chờ · Không có âm thanh thật", "Simulation waiting · No real audio"),
    CopyKey.PlaybackEnded to ShellText("Mô phỏng đã kết thúc · Không có âm thanh thật", "Simulation ended · No real audio"),
    CopyKey.SelectionUnavailable to ShellText("Chưa có nội dung minh họa cho lựa chọn này", "No sample content for this selection"),
    CopyKey.Play to ShellText("Mô phỏng phát", "Simulate play"),
    CopyKey.Pause to ShellText("Dừng mô phỏng", "Stop simulation"),
    CopyKey.Position to ShellText("Điểm nghe minh họa", "Demo position"),
    CopyKey.PlayerExplanation to ShellText("Điểm nghe chỉ đổi khi bạn thao tác. Không có bộ phát, audio, tải xuống hay đồng bộ thật.", "Position changes only when you interact. No player engine, audio, downloads or sync are connected."),
    CopyKey.NextExplanation to ShellText("Chọn tập để đổi nội dung minh họa. Không tự phát tập tiếp theo.", "Choose an episode to change the demo. Episodes do not advance automatically."),
    CopyKey.PreferencesNote to ShellText("Ngôn ngữ theo lựa chọn trong ứng dụng. Tốc độ và hẹn giờ chỉ đổi trạng thái minh họa; không điều khiển audio hoặc đếm ngược.", "Language follows your app preference. Speed and timer change demo state only; they do not control audio or run a countdown."),
    CopyKey.Speed to ShellText("Tốc độ minh họa", "Demo speed"),
    CopyKey.Timer to ShellText("Hẹn giờ minh họa", "Demo timer"),
)

internal enum class MembershipFixtureText(val vi: String, val en: String) {
    FixtureTools("Công cụ fixture · Chọn kết quả thủ công", "Fixture tools · Choose an outcome manually"),
    ConfirmFixture("Fixture: xác nhận đã nhận đăng ký", "Fixture: confirm receipt"),
    FailFixture("Fixture: lỗi chưa nhận đăng ký", "Fixture: request failed"),
    TimeoutFixture("Fixture: timeout, kết quả chưa biết", "Fixture: timeout, outcome unknown"),
    AccountFixtureAction("Fixture: đăng ký tài khoản hoàn tất", "Fixture: account registration complete"),
    ClearSession("Fixture: đăng xuất / đổi tài khoản / hết phiên", "Fixture: logout / switch account / session expired");

    fun localized(locale: ShellLocale): String = if (locale == ShellLocale.English) en else vi
}

internal enum class DemoShellText(val vi: String, val en: String) {
    DiagnosticsTitle("Công cụ bản xem thử", "Demo tools"),
    DiagnosticsSummary("Trạng thái khám phá", "Discovery scenario"),
    Notice("Dữ liệu minh họa · Không có âm thanh thật", "Sample data · No real audio"),
    CatalogStateNote("Chọn một trạng thái, rồi trở về Khám phá hoặc Tìm kiếm.", "Choose a state, then return to Discover or Search."),
    Ready("Bình thường", "Ready"),
    Loading("Đang tải", "Loading"),
    Empty("Rỗng", "Empty"),
    Error("Lỗi", "Error"),
    IntegrationsPending("Đăng nhập, tải xuống, audio nền và đồng bộ sẽ cần adapter native và API riêng.", "Sign-in, downloads, background audio and sync need native adapters and their own API."),
    Reset("Đặt lại bản xem thử", "Reset demo"),
    ResetNote("Khôi phục nội dung, điểm nghe, dấu trang và tùy chọn ban đầu.", "Restore initial content, position, bookmarks and preferences."),
    ;

    val resource: ShellText get() = ShellText(vi, en)

    fun localized(locale: ShellLocale): String = if (locale == ShellLocale.Vietnamese) vi else en
}
