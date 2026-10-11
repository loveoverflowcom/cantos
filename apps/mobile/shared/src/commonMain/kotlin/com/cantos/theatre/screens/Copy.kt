package com.cantos.theatre.screens

enum class Key {
    Discover, Search, Library, Work, Player, Back, Demo, Tagline, MissingArt,
    Featured, Synopsis, Episode, Episodes, OpenPlayer, Play, Save, Download,
    Unavailable, LibraryUnavailable, SearchHint, Loading, Empty, Error, NotImplemented,
    Ready, DemoStates, Reset, Light, Dark, Mini, Stories,
}

fun copy(key: Key, english: Boolean): String {
    val pair = when (key) {
        Key.Discover -> "Khám phá" to "Discover"
        Key.Search -> "Tìm kiếm" to "Search"
        Key.Library -> "Tủ sách" to "Library"
        Key.Work -> "Sơn Hà Ký" to "Sơn Hà Ký"
        Key.Player -> "Phòng nghe" to "Listening room"
        Key.Back -> "Quay lại" to "Back"
        Key.Demo -> "Bản khung · dữ liệu minh họa · không phát âm thanh" to "Skeleton · demo content · no audio playback"
        Key.Tagline -> "Lắng nghe một miền khác." to "Listen to another world."
        Key.MissingArt -> "Bìa minh họa chưa được cấp quyền sử dụng" to "Cover artwork awaits usage permission"
        Key.Featured -> "Tuyển chọn Cantos · mẫu" to "Cantos selection · demo"
        Key.Synopsis -> "Một tiếng chuông lay động sơn hà. Một hành trình tìm lại điều đã lãng quên." to "A bell echoes across the mountains. A journey to recover what was forgotten."
        Key.Episode -> "Người giữ tiếng chuông" to "The keeper of the bell"
        Key.Episodes -> "Các tập minh họa" to "Demo episodes"
        Key.OpenPlayer -> "Mở phòng nghe mẫu" to "Open the demo listening room"
        Key.Play -> "Phát" to "Play"
        Key.Save -> "Lưu vào tủ sách" to "Save to library"
        Key.Download -> "Tải xuống" to "Download"
        Key.Unavailable -> "Chưa có bản phát hành hoặc audio adapter. Phát, tua, dấu trang và tải xuống chưa được triển khai." to "No publication or audio adapter is connected. Playback, seeking, bookmarks and downloads are not implemented."
        Key.LibraryUnavailable -> "Tủ sách chưa kết nối tài khoản. Chưa có lịch sử nghe, tác phẩm đã lưu hoặc tệp tải xuống." to "The library has no account connection. Listening history, saved works and downloads are not available."
        Key.SearchHint -> "Tìm theo tên có dấu trong dữ liệu mẫu trên thiết bị" to "Search demo titles with diacritics on this device"
        Key.Loading -> "Đang tải · trạng thái minh họa" to "Loading · demo state"
        Key.Empty -> "Không có kết quả trong dữ liệu mẫu" to "No results in demo content"
        Key.Error -> "Không thể tải · lỗi minh họa, chưa có yêu cầu mạng" to "Could not load · demo error, no network request was made"
        Key.NotImplemented -> "Chưa triển khai kết nối dữ liệu Theatre" to "Theatre data connection is not implemented"
        Key.Ready -> "Nội dung mẫu" to "Demo content"
        Key.DemoStates -> "Trạng thái minh họa" to "Demo states"
        Key.Reset -> "Trở lại dữ liệu mẫu" to "Return to demo content"
        Key.Light -> "Nền sáng" to "Light theme"
        Key.Dark -> "Nền tối" to "Dark theme"
        Key.Mini -> "Trình phát thu gọn · chưa kết nối" to "Mini-player · not connected"
        Key.Stories -> "Dành cho đêm yên" to "For quiet nights"
    }
    return if (english) pair.second else pair.first
}
